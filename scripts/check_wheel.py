"""Install the built wheel into a fresh environment of the matching CPython and use it."""

import json
import os
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    binary = Path(os.environ.get("PYNOX_BIN", ROOT / "target/release/pynox")).resolve()
    info = json.loads(subprocess.check_output([binary, "doctor"]))
    minor = info["python"]["major_minor"]
    wheels = sorted((ROOT / "dist").glob(f"pynox-{info['version']}-{info['python']['abi']}-*.whl"))
    if not wheels:
        raise SystemExit("Run `make wheel` first")
    wheel = wheels[-1]
    with tempfile.TemporaryDirectory(prefix="pynox-wheel-test-") as temporary:
        venv = Path(temporary) / "venv"
        subprocess.run(["uv", "venv", "--quiet", "--python", minor, str(venv)], check=True)
        python = venv / "bin/python"
        subprocess.run(
            ["uv", "pip", "install", "--quiet", "--python", str(python), str(wheel)], check=True
        )
        env = {k: v for k, v in os.environ.items() if k not in ("PYNOX_BIN", "PYNOX_CONFIG")}
        probe = (
            "import pynox, sys\n"
            "assert pynox.binary_path().endswith('/pynox/_bundle/bin/pynox'), pynox.binary_path()\n"
            "result = pynox.run('import sys; print(sys.version_info[:2])')\n"
            "assert result.ok, result\n"
            "assert result.text.strip() == str(tuple(sys.version_info[:2])), result.text\n"
            "denied = pynox.run(\"open('/etc/passwd').read()\")\n"
            "assert denied.error['type'] == 'PermissionError', denied\n"
            "tool = pynox.tool()\n"
            "assert tool('print(6*7)') == '42\\n'\n"
            "print('wheel works: sandbox runs the same CPython as the venv')\n"
        )
        subprocess.run([str(python), "-c", probe], check=True, env=env, cwd=temporary)
        version = subprocess.check_output(
            [str(venv / "bin/pynox"), "--version"], text=True, env=env
        )
        assert version.startswith(f"pynox {info['version']} (CPython {minor}."), version
        print(version.strip(), "via console script")


if __name__ == "__main__":
    main()
