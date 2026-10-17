"""Contracts for the v0.1 integrations, against real processes and kernel rules."""

import json
import os
import platform
import socket
import subprocess
import tempfile
import unittest
from pathlib import Path

import test_runtime as runtime


class ProductTests(unittest.TestCase):
    def python(self, *args):
        return subprocess.run(
            [str(runtime.BINARY), *args],
            capture_output=True,
            text=True,
            timeout=30,
        )

    def test_default_mode_is_the_python_cli(self):
        result = self.python("-c", "import sys; print(sys.argv[1:]); sys.exit(7)", "one")
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertEqual(result.stdout, "['one']\n")
        result = self.python("-m", "json.tool", "--help")
        self.assertEqual(result.returncode, 0, result.stderr)
        # The explicit alias keeps working for existing integrations.
        result = self.python("python", "-c", "print('alias')")
        self.assertEqual((result.returncode, result.stdout), (0, "alias\n"), result.stderr)
        with tempfile.TemporaryDirectory() as temporary:
            script = Path(temporary) / "script.py"
            script.write_text("import sys\nprint('plain', sys.argv[1:])\nsys.exit(3)\n")
            result = self.python(str(script), "--allow-network", "x")
            self.assertEqual(result.returncode, 3, result.stderr)
            self.assertEqual(result.stdout, "plain ['--allow-network', 'x']\n")

    def test_optimizations_are_transparent_and_lazy(self):
        result = self.python(
            "-c",
            "import sys\n"
            "before = '_pynox_json_runtime' in sys.modules, 'json' in sys.modules\n"
            "import json\n"
            "after = '_pynox_json_runtime' in sys.modules\n"
            "print(before, after, json.dump.__module__, type(json.__loader__).__name__,\n"
            "      json.__spec__.loader is json.__loader__, json.__spec__.name)\n",
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "(False, False) True json SourceFileLoader True json\n")

    def test_doctor_and_version(self):
        result = subprocess.run([runtime.BINARY, "doctor"], capture_output=True, check=True)
        doctor = json.loads(result.stdout)
        self.assertEqual(doctor["runtime"], "pynox")
        python = self.python("-c", "import platform; print(platform.python_version())")
        self.assertEqual(python.returncode, 0, python.stderr)
        expected = f"pynox {doctor['version']} (CPython {python.stdout.strip()})\n"
        for flag in ("--version", "-V"):
            with self.subTest(flag=flag):
                result = subprocess.run(
                    [runtime.BINARY, flag], capture_output=True, text=True, check=True
                )
                self.assertEqual(result.stdout, expected)
                self.assertEqual(result.stderr, "")
        result = subprocess.run(
            [runtime.BINARY, "--help"], capture_output=True, text=True, check=True
        )
        self.assertIn("pynox sandbox", result.stdout)

    def stock_results(self):
        source = runtime.ROOT / "tests/fixtures/json_contract.py"
        doctor = json.loads(subprocess.check_output([runtime.BINARY, "doctor"]))
        base = subprocess.run(
            [doctor["python"]["executable"], "-I", "-S", "-B", source],
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
        expected = json.loads(base.stdout)["results"]
        self.assertEqual(len(expected), 158)
        return source, expected

    def test_json_differential(self):
        source, expected = self.stock_results()
        result = self.python("-I", "-S", "-B", str(source))
        self.assertEqual(result.returncode, 0, result.stderr)
        actual = json.loads(result.stdout)
        self.assertEqual(expected, actual["results"])
        self.assertEqual(actual["paths"]["nested"]["fast"], 1)
        self.assertEqual(actual["paths"]["writer_custom"]["fallback"], 1)

    @unittest.skipUnless(platform.system() == "Linux", "Linux sandbox backend")
    def test_json_differential_under_sandbox(self):
        source, expected = self.stock_results()
        with tempfile.TemporaryDirectory() as temporary:
            completed = subprocess.run(
                [
                    runtime.BINARY,
                    "sandbox",
                    "--max-output=1048576",
                    f"--allow-read={temporary}",
                    f"--allow-write={temporary}",
                    "--allow-env=TMPDIR",
                    source,
                ],
                env={**os.environ, "TMPDIR": temporary},
                capture_output=True,
                text=True,
                timeout=30,
            )
        report = json.loads(completed.stdout)
        self.assertEqual(report["exit"], 0, report)
        actual = json.loads(report["stdout"])
        # Partial output before a RecursionError depends on the frames above the
        # script; the sandbox runner adds one. The error itself must match.
        depth_dependent = {"cycle_no_check"}
        for name in depth_dependent:
            self.assertEqual(expected[name]["error"], actual["results"][name]["error"])
        self.assertEqual(
            {k: v for k, v in expected.items() if k not in depth_dependent},
            {k: v for k, v in actual["results"].items() if k not in depth_dependent},
        )
        self.assertEqual(actual["paths"]["nested"]["fast"], 1)

    @unittest.skipUnless(platform.system() == "Linux", "Linux sandbox backend")
    def test_sandbox_uses_utf8_like_stock_python(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            script = root / "test.py"
            script.write_text(
                "import sys\n"
                f"with open({str(root / 'out.txt')!r}, 'w') as f: f.write('ação')\n"
                "print(sys.getfilesystemencoding(), sys.stdout.encoding, 'ação')\n"
            )
            result = subprocess.run(
                [runtime.BINARY, "sandbox", f"--allow-write={root}", script],
                text=True,
                capture_output=True,
                timeout=10,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report["exit"], 0, report)
            self.assertEqual(report["stdout"], "utf-8 utf-8 ação\n")
            self.assertEqual((root / "out.txt").read_bytes(), "ação".encode())

    @unittest.skipUnless(platform.system() == "Linux", "Linux sandbox backend")
    def test_pool_optimization_is_present_in_every_job_and_stays_lazy(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            lazy = root / "lazy.py"
            lazy.write_text("import sys\nprint('_pynox_json_runtime' in sys.modules)\n")
            eager = root / "eager.py"
            eager.write_text("import sys, json\nprint('_pynox_json_runtime' in sys.modules)\n")
            requests = [{"script": str(path)} for path in (lazy, eager, lazy, eager)]
            result = subprocess.run(
                [runtime.BINARY, "pool", "--workers=1"],
                input="".join(json.dumps(r) + "\n" for r in requests),
                text=True,
                capture_output=True,
                timeout=30,
                check=True,
            )
            reports = [json.loads(line) for line in result.stdout.splitlines()][1:]
            self.assertEqual(
                [r["stdout"] for r in reports], ["False\n", "True\n", "False\n", "True\n"]
            )

    def test_pool_runs_inline_code_without_files(self):
        requests = [
            {
                "id": "named",
                "code": "import sys\nprint(sys.argv, __name__, '__file__' in dir())",
                "name": "step.py",
                "args": ["a"],
            },
            {"id": "error", "code": "x = 1\n1/0\n", "name": "calc.py"},
            {"id": "default-name", "code": "raise ValueError('boom')"},
            {
                "id": "no-file-grant",
                "code": "open(__import__('sys').argv[0]).read()",
                "name": "step.py",
            },
            {"id": "both", "script": "a.py", "code": "print(1)"},
            {"id": "neither"},
        ]
        result = subprocess.run(
            [runtime.BINARY, "pool", "--workers=1"],
            input="".join(json.dumps(r) + "\n" for r in requests),
            text=True,
            capture_output=True,
            timeout=30,
            check=True,
        )
        reports = {r["id"]: r for r in map(json.loads, result.stdout.splitlines()[1:])}
        self.assertEqual(reports["named"]["stdout"], "['step.py', 'a'] __main__ False\n")
        self.assertEqual(reports["named"]["exit"], 0, reports["named"])
        error = reports["error"]["error"]
        self.assertEqual(
            (error["type"], error["file"], error["line"]), ("ZeroDivisionError", "calc.py", 2)
        )
        self.assertEqual(reports["default-name"]["error"]["file"], "<code>")
        self.assertEqual(reports["no-file-grant"]["error"]["type"], "FileNotFoundError")
        self.assertEqual(reports["both"]["error"]["type"], "RequestError")
        self.assertEqual(reports["neither"]["error"]["type"], "RequestError")

    @unittest.skipUnless(platform.system() == "Linux", "Linux sandbox backend")
    def test_network_permission_reaches_local_server(self):
        with socket.socket() as listener, tempfile.TemporaryDirectory() as temporary:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            listener.settimeout(2)
            script = Path(temporary) / "client.py"
            script.write_text(
                "import socket\n"
                f"with socket.create_connection({listener.getsockname()!r}) as s:\n"
                "    s.sendall(b'pynox')\n"
            )
            result = subprocess.run(
                [runtime.BINARY, "sandbox", "--allow-network", script],
                text=True,
                capture_output=True,
                timeout=10,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report["exit"], 0, report)
            connection, _ = listener.accept()
            with connection:
                self.assertEqual(connection.recv(32), b"pynox")
