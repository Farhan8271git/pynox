"""Contracts for the v0.1 integrations, against real processes and kernel rules."""

import json
import os
import socket
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import test_runtime as runtime


class ProductTests(unittest.TestCase):
    def python(self, *args):
        return subprocess.run(
            [str(runtime.BINARY), "python", *args],
            capture_output=True,
            text=True,
            timeout=30,
        )

    def test_python_cli_and_exit_status(self):
        result = self.python("-c", "import sys; print(sys.argv[1:]); sys.exit(7)", "one")
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertEqual(result.stdout, "['one']\n")
        result = self.python("-m", "json.tool", "--help")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_doctor_and_version(self):
        result = subprocess.run([runtime.BINARY, "doctor"], capture_output=True, check=True)
        self.assertEqual(json.loads(result.stdout)["runtime"], "pynox")
        result = subprocess.run([runtime.BINARY, "--version"], capture_output=True, check=True)
        self.assertEqual(result.stdout.strip(), b"pynox 0.1.0")

    def test_json_differential(self):
        source = runtime.ROOT / "tests/fixtures/json_contract.py"
        base = subprocess.run(
            [sys.executable, "-I", "-S", "-B", source],
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        )
        expected = json.loads(base.stdout)["results"]
        self.assertEqual(len(expected), 158)
        for flags in ([], ["--optimize-json"]):
            result = self.python(*flags, "-I", "-S", "-B", str(source))
            self.assertEqual(result.returncode, 0, result.stderr)
            actual = json.loads(result.stdout)
            self.assertEqual(expected, actual["results"])
            if flags:
                self.assertEqual(actual["paths"]["nested"]["fast"], 1)
                self.assertEqual(actual["paths"]["writer_custom"]["fallback"], 1)

    def test_json_differential_under_sandbox(self):
        source = runtime.ROOT / "tests/fixtures/json_contract.py"
        results = []
        with tempfile.TemporaryDirectory() as temporary:
            for flags in ([], ["--optimize-json"]):
                completed = subprocess.run(
                    [
                        runtime.BINARY,
                        *flags,
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
                results.append(json.loads(report["stdout"]))
        self.assertEqual(results[0]["results"], results[1]["results"])
        self.assertEqual(results[1]["paths"]["nested"]["fast"], 1)

    def test_pool_optimization_does_not_leak_to_next_job(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            script = root / "test.py"
            script.write_text("import sys\nprint('_pynox_json_runtime' in sys.modules)\n")
            requests = [{"script": str(script), "optimize_json": flag} for flag in (True, False)]
            result = subprocess.run(
                [runtime.BINARY, "pool", "--workers=1"],
                input="".join(json.dumps(r) + "\n" for r in requests),
                text=True,
                capture_output=True,
                timeout=30,
                check=True,
            )
            reports = [json.loads(line) for line in result.stdout.splitlines()][1:]
            self.assertEqual([r["stdout"] for r in reports], ["True\n", "False\n"])

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
                [runtime.BINARY, "--allow-network", script],
                text=True,
                capture_output=True,
                timeout=10,
            )
            report = json.loads(result.stdout)
            self.assertEqual(report["exit"], 0, report)
            connection, _ = listener.accept()
            with connection:
                self.assertEqual(connection.recv(32), b"pynox")
