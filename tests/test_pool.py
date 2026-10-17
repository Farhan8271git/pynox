"""Run the existing security contract against a persistent pool, plus lifecycle tests."""

import json
import os
import selectors
import signal
import subprocess
import textwrap

import test_runtime as runtime


class PoolRuntimeTests(runtime.RuntimeTests):
    def setUp(self):
        super().setUp()
        self.pool = None
        self.counter = 0
        self.addCleanup(self.close_pool)
        self.start_pool()

    def start_pool(self, pool_flags=(), **kwargs):
        self.close_pool()
        self.pool = subprocess.Popen(
            [str(runtime.BINARY), "pool", "--workers=1", *pool_flags],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            **kwargs,
        )
        self.assertEqual(self.read_response()["event"], "ready")

    def close_pool(self):
        if self.pool is not None:
            self.pool.stdin.close()
            try:
                self.pool.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.pool.kill()
                self.pool.wait()
            self.pool.stdout.close()
            self.pool.stderr.close()
            self.pool = None

    def read_response(self):
        with selectors.DefaultSelector() as selector:
            selector.register(self.pool.stdout, selectors.EVENT_READ)
            self.assertTrue(selector.select(timeout=10), "Pool response timed out")
        line = self.pool.stdout.readline()
        self.assertTrue(line, "Pool exited unexpectedly")
        return json.loads(line)

    def request(self, request):
        self.pool.stdin.write(json.dumps(request) + "\n")
        self.pool.stdin.flush()
        return self.read_response()

    def run_code(self, code, *flags, args=(), env=None, pass_fds=(), preexec_fn=None):
        if pass_fds or preexec_fn:
            self.start_pool(pass_fds=pass_fds, preexec_fn=preexec_fn)
        self.script.write_text(textwrap.dedent(code))
        self.counter += 1
        request = {"id": self.counter, "script": str(self.script), "args": list(args)}
        environment = os.environ if env is None else env
        for flag in flags:
            name, value = str(flag).removeprefix("--").split("=", 1)
            name = name.replace("-", "_")
            if name == "allow_env":
                request.setdefault("env", {})[value] = environment.get(value, "")
            elif name in ("allow_read", "allow_write", "package_dir"):
                if name == "package_dir":
                    name = "package_dirs"
                request.setdefault(name, []).append(value)
            elif name in ("allow_network", "optimize_json"):
                request[name] = value == "true"
            else:
                request[name] = int(value) if name == "max_output" else value
        result = self.request(request)
        self.assertEqual(result["id"], self.counter)
        return result

    def test_each_task_gets_fresh_process_from_same_template(self):
        results = [
            self.run_code("import os, json; print(json.dumps([os.getpid(), os.getppid()]))")
            for _ in range(4)
        ]
        identities = [json.loads(result["stdout"]) for result in results]
        self.assertEqual(len({pid for pid, _ in identities}), 4)
        self.assertEqual(len({parent for _, parent in identities}), 1)

    def test_modules_environment_and_cwd_do_not_leak(self):
        original_cwd = os.getcwd()
        result = self.run_code("""
            import builtins, os, sys, types
            builtins.PYNOX_LEAK = "old state"
            sys.modules["leaked_module"] = types.ModuleType("leaked_module")
            os.environ["OLD_SECRET"] = "should disappear"
            os.chdir("/tmp")
            print("changed")
        """)
        self.assertEqual(result["exit"], 0, result)
        result = self.run_code("""
            import builtins, ctypes, json, os, sys
            libc = ctypes.CDLL(None)
            libc.getenv.restype = ctypes.c_char_p
            print(json.dumps([
                hasattr(builtins, "PYNOX_LEAK"), "leaked_module" in sys.modules,
                os.environ.get("OLD_SECRET"), libc.getenv(b"OLD_SECRET"), os.getcwd()
            ]))
        """)
        self.assertEqual(json.loads(result["stdout"]), [False, False, None, None, original_cwd])

    def test_per_request_permissions_do_not_accumulate(self):
        code = f"print(open({str(self.secret)!r}).read())"
        allowed = self.run_code(code, f"--allow-read={self.secret}")
        self.assertEqual(allowed["exit"], 0, allowed)
        self.assert_denied(self.run_code(code))
        self.assertEqual(self.run_code(code, f"--allow-read={self.secret}")["exit"], 0)

    def test_secret_is_delivered_only_to_selected_task(self):
        code = "import os; print(os.environ.get('TASK_SECRET', 'absent'))"
        first = self.run_code(code, "--allow-env=TASK_SECRET", env={"TASK_SECRET": "first-only"})
        second = self.run_code(code)
        self.assertEqual(first["stdout"], "first-only\n")
        self.assertEqual(second["stdout"], "absent\n")

    def test_pool_survives_signal_timeout_and_memory_error(self):
        for code, flags, kind in [
            ("import ctypes; ctypes.string_at(0)", (), "SignalError"),
            ("while True: pass", ("--timeout=50ms",), "TimeoutError"),
            ("x = bytearray(1024**3)", ("--memory=64m",), "MemoryError"),
        ]:
            failed = self.run_code(code, *flags)
            self.assertEqual(failed["error"]["type"], kind, failed)
            healthy = self.run_code("print('still healthy')")
            self.assertEqual(healthy["stdout"], "still healthy\n", healthy)
            self.assertEqual(healthy["exit"], 0, healthy)

    def test_infinite_output_is_bounded_and_times_out(self):
        result = self.run_code(
            "import os\nwhile True: os.write(1, b'x' * 8192)",
            "--timeout=100ms",
            "--max-output=1024",
        )
        self.assertEqual(result["error"]["type"], "TimeoutError", result)
        self.assertTrue(result["stdout_truncated"])
        self.assertEqual(len(result["stdout"]), 1024)
        self.assertEqual(self.run_code("print('next')")["stdout"], "next\n")

    def test_no_control_descriptors_reach_the_script(self):
        result = self.run_code("""
            import os
            opened = []
            for fd in range(128):
                try:
                    os.fstat(fd)
                    opened.append(fd)
                except OSError:
                    pass
            print(opened)
        """)
        self.assertEqual(result["stdout"], "[0, 1, 2, 3]\n", result)

    def test_atexit_is_preserved(self):
        result = self.run_code("import atexit; atexit.register(print, 'finalized')")
        self.assertEqual(result["stdout"], "finalized\n", result)

    def test_bad_requests_do_not_poison_the_pool(self):
        self.script.write_text("print('ok')")
        for request in [
            {"id": "bad", "script": str(self.script), "allow_net": ["example.com"]},
            {"id": "bad", "script": str(self.script), "timeout": "zero"},
            {"id": "bad", "script": str(self.script), "max_output": 0},
            {"id": "bad", "script": str(self.script), "env": {"LD_PRELOAD": "bad.so"}},
        ]:
            result = self.request(request)
            self.assertEqual(result["id"], "bad", result)
            self.assertEqual(result["error"]["type"], "RequestError", result)
        self.pool.stdin.write("not json\n")
        self.pool.stdin.flush()
        self.assertEqual(self.read_response()["error"]["type"], "RequestError")
        self.assertEqual(self.run_code("print('ok')")["stdout"], "ok\n")

    def test_oversized_line_is_drained_before_next_request(self):
        self.pool.stdin.write("x" * (1024 * 1024 + 1) + "\n")
        self.pool.stdin.flush()
        self.assertEqual(self.read_response()["error"]["type"], "RequestError")
        self.assertEqual(self.run_code("print('ok')")["stdout"], "ok\n")

    def test_failed_template_is_replaced_without_retrying_task(self):
        result = self.run_code("import os; print(os.getppid())")
        os.kill(int(result["stdout"]), signal.SIGKILL)
        result = self.run_code("print('must not retry')")
        self.assertEqual(result["error"]["type"], "PoolWorkerError", result)
        self.assertEqual(self.run_code("print('replacement')")["stdout"], "replacement\n")

    def test_parallel_jobs_and_eof_drain(self):
        slow = self.root / "slow.py"
        fast = self.root / "fast.py"
        slow.write_text("import time; time.sleep(0.3); print('slow')")
        fast.write_text("print('fast')")
        requests = [
            {"id": "slow", "script": str(slow)},
            {"id": "fast", "script": str(fast)},
            *[{"id": n, "script": str(fast)} for n in range(6)],
        ]
        completed = subprocess.run(
            [str(runtime.BINARY), "pool", "--workers=2"],
            input="".join(json.dumps(request) + "\n" for request in requests),
            text=True,
            capture_output=True,
            timeout=10,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        events = [json.loads(line) for line in completed.stdout.splitlines()]
        self.assertEqual(events[0]["event"], "ready")
        responses = events[1:]
        self.assertEqual(len(responses), len(requests))
        self.assertEqual(responses[0]["id"], "fast")
        self.assertEqual({response["id"] for response in responses}, {"fast", "slow", *range(6)})
        self.assertTrue(all(response["exit"] == 0 for response in responses), responses)
