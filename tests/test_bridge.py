"""Host functions: sandboxed code calls functions the application exposes."""

import json
import os
import subprocess
import sys
import threading
import time
import unittest

import test_runtime as runtime

sys.path.insert(0, str(runtime.ROOT / "python"))
os.environ["PYNOX_BIN"] = str(runtime.BINARY)

import pynox  # noqa: E402


class ProtocolTests(unittest.TestCase):
    def setUp(self):
        self.process = subprocess.Popen(
            [str(runtime.BINARY), "pool", "--workers=1"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
        )
        self.addCleanup(self.process.kill)
        self.assertEqual(json.loads(self.process.stdout.readline())["event"], "ready")

    def send(self, message):
        self.process.stdin.write(json.dumps(message) + "\n")
        self.process.stdin.flush()

    def receive(self):
        return json.loads(self.process.stdout.readline())

    def test_undeclared_functions_and_forged_frames_are_refused(self):
        forged = (
            "import os, json\n"
            "os.write(3, json.dumps({'pynox_call': {'function': 'delete_all'}}).encode() + b'\\n')\n"
            "header = b''\n"
            "while not header.endswith(b'\\n'): header += os.read(4, 1)\n"
            "print(json.loads(os.read(4, int(header)))['error'])\n"
        )
        self.send({"id": "forge", "functions": ["search"], "code": forged})
        result = self.receive()
        self.assertEqual(result["id"], "forge", "no call event reached the client")
        self.assertIn("not available", result["stdout"])
        self.send({"id": "plain", "code": "import pynox_host"})
        self.assertEqual(self.receive()["error"]["type"], "ModuleNotFoundError")
        self.send({"id": "bad-name", "functions": ["rm -rf"], "code": "pass"})
        self.assertEqual(self.receive()["error"]["type"], "RequestError")
        self.send({"id": "late", "reply": {"call_id": 999, "result": 1}})
        self.assertEqual(self.receive()["error"]["type"], "RequestError")

    def test_waiting_for_the_host_counts_against_the_timeout(self):
        self.send(
            {
                "id": "slow",
                "functions": ["wait"],
                "timeout": "300ms",
                "code": "from pynox_host import wait\nwait()",
            }
        )
        event = self.receive()
        self.assertEqual((event["event"], event["function"]), ("call", "wait"))
        result = self.receive()
        self.assertEqual((result["id"], result["exit"]), ("slow", 124))
        self.send({"id": "after", "reply": {"call_id": event["call_id"], "result": None}})
        self.assertEqual(
            self.receive()["error"]["type"], "RequestError", "the call died with its task"
        )

    def test_session_steps_keep_their_functions_and_cannot_add_more(self):
        self.send(
            {
                "id": 1,
                "session": "s",
                "functions": ["double"],
                "code": "from pynox_host import double\ntotal = double(21)",
            }
        )
        event = self.receive()
        self.send({"reply": {"call_id": event["call_id"], "result": event["args"][0] * 2}})
        self.assertEqual(self.receive()["exit"], 0)
        self.send({"id": 2, "session": "s", "code": "print(total, double(total))"})
        event = self.receive()
        self.send({"reply": {"call_id": event["call_id"], "result": event["args"][0] * 2}})
        self.assertEqual(self.receive()["stdout"], "42 84\n")
        self.send({"id": 3, "session": "s", "functions": ["other"], "code": "pass"})
        self.assertEqual(self.receive()["error"]["type"], "RequestError")


def search(query: str, limit: int = 3) -> list:
    """Search the product catalog."""
    return [f"{query}-{i}" for i in range(limit)]


class PackageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pool = pynox.Pool(workers=2).start()

    @classmethod
    def tearDownClass(cls):
        cls.pool.close()

    def test_functions_run_in_the_host_and_errors_become_host_errors(self):
        calls = []

        def record(event: str) -> str:
            calls.append((event, threading.current_thread().name))
            return "recorded"

        def broken():
            raise LookupError("catalog offline")

        result = self.pool.run(
            "from pynox_host import search, record, broken, HostError\n"
            "print(search('book', limit=2), record('hello'))\n"
            "try:\n    broken()\nexcept HostError as e:\n    print('host said:', e)\n",
            functions={"search": search, "record": record, "broken": broken},
        )
        self.assertTrue(result.ok, result)
        self.assertEqual(
            result.stdout,
            "['book-0', 'book-1'] recorded\nhost said: LookupError: catalog offline\n",
        )
        self.assertEqual([c[0] for c in calls], ["hello"])

    def test_concurrent_tasks_route_calls_to_their_own_functions(self):
        def tagged(tag):
            return lambda: tag

        results = {}

        def run(tag):
            results[tag] = self.pool.run(
                "import time\nfrom pynox_host import who\ntime.sleep(0.1)\nprint(who())",
                functions={"who": tagged(tag)},
            ).stdout

        threads = [threading.Thread(target=run, args=(t,)) for t in ("a", "b", "c", "d")]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assertEqual(results, {t: f"{t}\n" for t in "abcd"})

    def test_values_travel_as_json(self):
        result = self.pool.run(
            "from pynox_host import echo\nprint(echo({'n': [1, 2.5, None, True]}, label='x'))\n"
            "print(echo(object()))",
            functions={"echo": lambda value, label=None: value, "unused": lambda: None},
        )
        self.assertEqual(result.stdout.splitlines()[0], "{'n': [1, 2.5, None, True]}")
        self.assertTrue(result.stdout.splitlines()[1].startswith("<object object"))

    def test_tools_describe_their_functions_to_the_model(self):
        run_python = self.pool.tool(functions={"search": search})
        self.assertIn("from pynox_host import", run_python.__doc__)
        self.assertIn(
            "search(query: str, limit: int = 3) -> list: Search the product catalog.",
            run_python.__doc__,
        )
        self.assertEqual(
            run_python("from pynox_host import search\nprint(search('x', 1))"), "['x-0']\n"
        )
        notebook = self.pool.tool(session=True, functions={"search": search})
        notebook("from pynox_host import search\nfound = search('y')")
        self.assertEqual(notebook("print(len(found), search('z', 1))"), "3 ['z-0']\n")

    def test_host_function_overhead_is_small(self):
        self.pool.run("from pynox_host import f\nf()", functions={"f": lambda: 1})
        start = time.perf_counter()
        result = self.pool.run(
            "from pynox_host import f\nprint(sum(f() for _ in range(100)))",
            functions={"f": lambda: 1},
        )
        elapsed = time.perf_counter() - start
        self.assertEqual(result.stdout, "100\n")
        self.assertLess(elapsed, 2.0, "100 round trips should take well under two seconds")
