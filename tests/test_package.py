"""The `pynox` Python package against the real pool."""

import concurrent.futures
import os
import sys
import tempfile
import unittest
from pathlib import Path

import test_runtime as runtime

sys.path.insert(0, str(runtime.ROOT / "python"))
os.environ["PYNOX_BIN"] = str(runtime.BINARY)

import pynox  # noqa: E402


class PoolTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pool = pynox.Pool(workers=2).start()

    @classmethod
    def tearDownClass(cls):
        cls.pool.close()

    def test_run_returns_output_and_structured_errors(self):
        ok = self.pool.run("print(6 * 7)")
        self.assertTrue(ok.ok)
        self.assertEqual((ok.exit, ok.stdout, ok.text), (0, "42\n", "42\n"))
        failed = self.pool.run("x = 1\n1 / 0\n", name="calc.py")
        self.assertFalse(failed.ok)
        self.assertEqual(
            (failed.error["type"], failed.error["file"], failed.error["line"]),
            ("ZeroDivisionError", "calc.py", 2),
        )
        self.assertIn("ZeroDivisionError: division by zero\nat calc.py:2", failed.text)
        denied = self.pool.run("open('/etc/passwd').read()")
        self.assertEqual(denied.error["type"], "PermissionError")
        self.assertIn("--allow-read", denied.text)
        self.assertEqual(self.pool.run("print()").text, "\n")
        self.assertEqual(self.pool.run("pass").text, "(no output)")

    def test_permissions_come_from_the_host_not_the_code(self):
        with tempfile.TemporaryDirectory(prefix="pynox-pkg-") as temporary:
            target = Path(temporary) / "out.txt"
            code = f"open({str(target)!r}, 'w').write('x')"
            self.assertEqual(self.pool.run(code).error["type"], "PermissionError")
            self.assertTrue(self.pool.run(code, allow_write=[temporary]).ok)
            self.assertEqual(target.read_text(), "x")
        slow = self.pool.run("while True: pass", timeout="300ms")
        self.assertEqual((slow.exit, slow.error["type"]), (124, "TimeoutError"))
        loud = self.pool.run("print('x' * 10000)", max_output=100)
        self.assertTrue(loud.truncated)

    def test_concurrent_calls_use_both_workers(self):
        code = "import time, os\ntime.sleep(0.3)\nprint(os.getpid())\n"
        with concurrent.futures.ThreadPoolExecutor(4) as executor:
            results = list(executor.map(lambda _: self.pool.run(code), range(4)))
        self.assertTrue(all(r.ok for r in results), results)
        self.assertEqual(len({r.stdout for r in results}), 4, "fresh process per call")
        self.assertLess(max(r.duration_ms for r in results), 1000)

    def test_tool_is_a_documented_function_for_any_framework(self):
        run_python = self.pool.tool(allow_write=["/tmp"])
        self.assertEqual(run_python.__name__, "run_python")
        self.assertEqual(run_python.__annotations__, {"code": str, "return": str})
        self.assertIn("Args:", run_python.__doc__)
        self.assertIn("write /tmp", run_python.__doc__)
        self.assertEqual(run_python("print('hi')"), "hi\n")
        self.assertIn("PermissionError", run_python("open('/etc/passwd')"))
        custom = self.pool.tool(name="python_exec", description="Custom text.")
        self.assertEqual((custom.__name__, custom.__doc__), ("python_exec", "Custom text."))

    def test_pool_exit_is_reported_not_hung(self):
        pool = pynox.Pool(workers=1).start()
        pool.close()
        with self.assertRaises(pynox.PoolError):
            pool.request({"code": "pass"})


class ConfigTests(unittest.TestCase):
    def test_profiles_and_limits_from_config(self):
        with tempfile.TemporaryDirectory(prefix="pynox-pkg-config-") as temporary:
            root = Path(temporary)
            (root / "out").mkdir()
            config = root / "pynox.toml"
            config.write_text(
                "[pool]\nworkers = 1\n"
                '[pool.limits]\nallow_write = ["out"]\nallow_network = false\n'
                '[profiles.writer]\nallow_write = ["out"]\n'
            )
            with pynox.Pool(config=config) as pool:
                self.assertEqual(pool.ready["profiles"], ["writer"])
                code = f"open({str(root / 'out' / 'f.txt')!r}, 'w').write('ok')"
                self.assertEqual(pool.run(code).error["type"], "PermissionError")
                self.assertTrue(pool.run(code, profile="writer").ok)
                over = pool.run("pass", allow_network=True)
                self.assertEqual(over.error["type"], "RequestError")
                self.assertIn("limits", over.error["message"])


class DefaultPoolTests(unittest.TestCase):
    def test_run_and_tool_share_one_default_pool(self):
        self.assertEqual(pynox.run("print(1 + 1)").text, "2\n")
        tool = pynox.tool()
        self.assertEqual(tool("print(2 + 2)"), "4\n")
        self.assertIs(pynox.default_pool(), pynox.default_pool())
        self.assertEqual(pynox.binary_path(), str(runtime.BINARY))


class StatsTests(unittest.TestCase):
    def test_pool_stats(self):
        with pynox.Pool(workers=1) as pool:
            pool.run("print(1)")
            pool.run("1/0")
            stats = pool.stats()
        self.assertEqual(
            (stats["event"], stats["jobs"], stats["ok"], stats["failed"]), ("stats", 2, 1, 1)
        )
        self.assertEqual(stats["errors"], {"ZeroDivisionError": 1})
