"""Small offline contracts for native and ordinary framework code."""

import importlib.util
import json
import subprocess
import sysconfig
import tempfile
import unittest
from pathlib import Path

import test_runtime as runtime


@unittest.skipUnless(
    importlib.util.find_spec("pydantic") and importlib.util.find_spec("aws_lambda_powertools"),
    "uv sync --group integration",
)
class FrameworkTests(unittest.TestCase):
    def test_validation_powertools_and_asyncio_under_sandbox(self):
        with tempfile.TemporaryDirectory() as temporary:
            script = Path(temporary) / "frameworks.py"
            script.write_text(
                "import asyncio, json\n"
                "from pydantic import BaseModel\n"
                "from aws_lambda_powertools import Logger\n"
                "class Order(BaseModel):\n"
                "    quantity: int\n"
                'order = Order.model_validate_json(\'{"quantity":"3"}\')\n'
                "Logger(service='pynox-test').info('validated', quantity=order.quantity)\n"
                "async def main():\n"
                "    return await asyncio.to_thread(lambda: order.quantity * 7)\n"
                "assert asyncio.run(main()) == 21\n"
                "print('frameworks passed')\n"
            )
            result = subprocess.run(
                [
                    runtime.BINARY,
                    "--optimize-json",
                    f"--package-dir={sysconfig.get_path('purelib')}",
                    script,
                ],
                capture_output=True,
                text=True,
                timeout=15,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report["exit"], 0, report)
            self.assertIn("frameworks passed", report["stdout"])
