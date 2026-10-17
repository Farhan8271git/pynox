"""Exercise verified installation, relocation and rejection of corrupted assets."""

import hashlib
import json
import os
import platform
import subprocess
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    archive = ROOT / f"dist/pynox-{version}-cp312-linux-{platform.machine()}.tar.gz"
    with tempfile.TemporaryDirectory(prefix="pynox-install-test-") as temporary:
        root = Path(temporary)
        env = {
            **os.environ,
            "PYNOX_INSTALL_ROOT": str(root / "runtime"),
            "PYNOX_BIN_DIR": str(root / "bin"),
            "PYNOX_VERSION": version,
        }
        command = ["bash", str(ROOT / "scripts/install.sh"), "--archive", str(archive)]
        subprocess.run(command, env=env, check=True)
        binary = root / "bin/pynox"
        doctor = json.loads(subprocess.check_output([binary, "doctor"]))
        assert Path(doctor["python"]["home"]).is_relative_to(root)
        subprocess.run([binary, str(ROOT / "examples/hello.py")], check=True)
        repeat = subprocess.run(command, env=env, capture_output=True)
        assert repeat.returncode != 0 and b"already installed" in repeat.stderr
        bad = root / archive.name
        bad.write_bytes(b"corrupted release")
        bad.with_suffix(bad.suffix + ".sha256").write_text(
            f"{hashlib.sha256(b'expected release').hexdigest()}  {bad.name}\n"
        )
        env["PYNOX_INSTALL_ROOT"] = str(root / "rejected")
        result = subprocess.run(
            ["bash", str(ROOT / "scripts/install.sh"), "--archive", str(bad)],
            env=env,
            capture_output=True,
        )
        assert result.returncode != 0 and b"Checksum mismatch" in result.stderr
        assert not (root / f"rejected/versions/{version}").exists()
    print("Installed bundle runs; repeat installation and corrupt archives rejected")


if __name__ == "__main__":
    main()
