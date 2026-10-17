"""Runtime contracts for the default Python mode and the capability probe."""

import importlib.util
import json
import subprocess
import sysconfig
import unittest

import test_runtime as runtime


class PortabilityTests(unittest.TestCase):
    def test_doctor_reports_runtime_and_sandbox_separately(self):
        doctor = json.loads(subprocess.check_output([runtime.BINARY, "doctor"]))
        self.assertTrue(doctor["runtime_supported"])
        self.assertIn("sandbox_supported", doctor)
        self.assertIn(doctor["platform"].rsplit("-", 1)[0], ("linux-gnu", "linux-musl"))
        self.assertTrue(doctor["platform"].endswith(f"-{doctor['architecture']}"))

    def test_default_mode_runs_asyncio_and_standard_library_extensions(self):
        source = (
            "import asyncio, ssl, sqlite3, zlib, json\n"
            "async def main():\n"
            "    await asyncio.sleep(0)\n"
            "    return sqlite3.connect(':memory:').execute('select 42').fetchone()[0]\n"
            "print(json.dumps({'result': asyncio.run(main())}))\n"
        )
        output = subprocess.check_output([runtime.BINARY, "-I", "-c", source], text=True)
        self.assertEqual(json.loads(output), {"result": 42})

    @unittest.skipUnless(
        importlib.util.find_spec("pydantic"), "Install integration test dependencies"
    )
    def test_default_mode_imports_pydantic_extension(self):
        packages = sysconfig.get_path("purelib")
        source = (
            f"import sys; sys.path.insert(0, {packages!r})\n"
            "from pydantic import TypeAdapter\n"
            "adapter = TypeAdapter(list[int])\n"
            "print(adapter.dump_json(adapter.validate_python(['1', '2'])).decode())\n"
        )
        output = subprocess.check_output(
            [runtime.BINARY, "python", "-I", "-S", "-c", source], text=True
        )
        self.assertEqual(json.loads(output), [1, 2])
