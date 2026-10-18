"""The agent-facing examples run against the real pool and, when installed, Monty."""

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

import test_runtime as runtime

sys.path.insert(0, str(runtime.ROOT / "python"))
sys.path.insert(0, str(runtime.ROOT / "examples"))

import pynox  # noqa: E402


class AgentToolTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.pool = pynox.Pool(workers=1, binary=runtime.BINARY).start()

    @classmethod
    def tearDownClass(cls):
        cls.pool.close()

    def test_inline_code_runs_with_fresh_state_and_structured_errors(self):
        first = self.pool.run("x = 41\nprint(x + 1)")
        self.assertTrue(first.ok, first)
        self.assertEqual(first.stdout, "42\n")
        second = self.pool.run("print(x)", name="second.py")
        self.assertFalse(second.ok)
        self.assertEqual(second.error["type"], "NameError")
        self.assertEqual((second.error["file"], second.error["line"]), ("second.py", 1))
        self.assertIn("NameError", second.describe())
        self.assertIn("second.py:1", second.describe())
        self.assertEqual(second.text, second.describe())

    def test_host_chooses_permissions_not_the_code(self):
        with tempfile.TemporaryDirectory(prefix="pynox-agent-") as temporary:
            root = Path(temporary)
            denied = self.pool.run(f"open({str(root / 'out.txt')!r}, 'w').write('x')")
            self.assertEqual(denied.error["type"], "PermissionError", denied)
            self.assertIn("--allow-write", denied.describe())
            allowed = self.pool.run(
                f"open({str(root / 'out.txt')!r}, 'w').write('x')", allow_write=[root]
            )
            self.assertTrue(allowed.ok, allowed)
            self.assertEqual((root / "out.txt").read_text(), "x")
        secret = self.pool.run("print(open('/etc/passwd').read())")
        self.assertEqual(secret.error["type"], "PermissionError")

    def test_limits_and_output_bounds_apply_per_call(self):
        slow = self.pool.run("while True: pass", timeout="300ms")
        self.assertEqual((slow.exit, slow.error["type"]), (124, "TimeoutError"))
        loud = self.pool.run("print('x' * 10000)", max_output=100)
        self.assertTrue(loud.truncated)
        self.assertEqual(len(loud.stdout), 100)

    def test_json_optimization_is_active_inside_jobs(self):
        with tempfile.TemporaryDirectory(prefix="pynox-agent-") as temporary:
            result = self.pool.run(
                "import json, _pynox_json\n"
                f"with open({str(Path(temporary) / 'o.json')!r}, 'w') as f:\n"
                "    json.dump({'rows': list(range(100))}, f)\n"
                "print(json.dumps(_pynox_json.stats()))\n",
                allow_write=[temporary],
            )
            self.assertTrue(result.ok, result)
            self.assertEqual(json.loads(result.stdout)["fast"], 1)


@unittest.skipUnless(importlib.util.find_spec("pydantic_monty"), "pip install pydantic-monty")
class MontyBridgeTests(unittest.TestCase):
    def test_monty_glue_code_delegates_heavy_work_to_pynox(self):
        from monty_bridge import run_agent

        with tempfile.TemporaryDirectory(prefix="pynox-monty-") as temporary:
            mean = run_agent(temporary, binary=runtime.BINARY)
            self.assertEqual(mean, 13)
            stored = json.loads((Path(temporary) / "stats.json").read_text())
            self.assertEqual(stored, {"mean": 13, "count": 3})
