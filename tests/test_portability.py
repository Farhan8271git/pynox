"""Native runtime contracts, including explicit macOS sandbox refusal."""

import importlib.util
import json
import platform
import subprocess
import sysconfig
import tempfile
import unittest
from pathlib import Path

import test_runtime as runtime


class PortabilityTests(unittest.TestCase):
    def test_doctor_reports_runtime_and_sandbox_separately(self):
        doctor = json.loads(subprocess.check_output([runtime.BINARY, "doctor"]))
        self.assertTrue(doctor["runtime_supported"])
        self.assertEqual(doctor["sandbox_supported"], platform.system() == "Linux")
        architecture = platform.machine().replace("arm64", "aarch64")
        self.assertEqual(doctor["architecture"], architecture)
        self.assertTrue(doctor["platform"].endswith(f"-{architecture}"))

    def test_native_python_runs_asyncio_and_standard_library_extensions(self):
        source = (
            "import asyncio, ssl, sqlite3, zlib, json\n"
            "async def main():\n"
            "    await asyncio.sleep(0)\n"
            "    return sqlite3.connect(':memory:').execute('select 42').fetchone()[0]\n"
            "print(json.dumps({'result': asyncio.run(main())}))\n"
        )
        output = subprocess.check_output([runtime.BINARY, "python", "-I", "-c", source], text=True)
        self.assertEqual(json.loads(output), {"result": 42})

    @unittest.skipUnless(
        importlib.util.find_spec("pydantic"), "Install integration test dependencies"
    )
    def test_native_python_imports_pydantic_extension(self):
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

    @unittest.skipUnless(platform.system() == "Darwin", "macOS runtime-only contract")
    def test_sandbox_commands_refuse_to_execute_code(self):
        with tempfile.TemporaryDirectory(prefix="pynox-macos-") as temporary:
            root = Path(temporary)
            marker = root / "executed"
            script = root / "task.py"
            script.write_text(f"open({str(marker)!r}, 'w').write('unexpected execution')\n")
            for args in ([str(script)], ["run", str(script)], ["pool", "--workers=1"]):
                with self.subTest(args=args):
                    result = subprocess.run(
                        [runtime.BINARY, *args],
                        input=json.dumps({"script": str(script)}) + "\n",
                        capture_output=True,
                        text=True,
                        timeout=10,
                    )
                    self.assertEqual(result.returncode, 125, result)
                    self.assertEqual(
                        json.loads(result.stdout)["error"]["type"], "SandboxUnavailable"
                    )
                    self.assertFalse(marker.exists())
