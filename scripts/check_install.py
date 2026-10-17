"""Exercise verified installation, relocation and rejection of corrupted assets."""

import hashlib
import json
import os
import subprocess
import tempfile
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]
    binary = Path(os.environ.get("PYNOX_BIN", ROOT / "target/release/pynox")).resolve()
    built = json.loads(subprocess.check_output([binary, "doctor"]))
    abi = built["python"]["abi"]
    platform_tag = built["platform"]
    python_minor = built["python"]["major_minor"]
    archive = ROOT / f"dist/pynox-{version}-{abi}-{platform_tag}.tar.gz"
    with tempfile.TemporaryDirectory(prefix="pynox-install-test-") as temporary:
        root = Path(temporary).resolve()
        env = {
            **os.environ,
            "PYNOX_INSTALL_ROOT": str(root / "runtime"),
            "PYNOX_BIN_DIR": str(root / "bin"),
            "PYNOX_VERSION": version,
        }
        # Local archive selection must work without a Python selection or install.
        env.pop("PYNOX_PYTHON_VERSION", None)
        other_minor = "3.12" if python_minor == "3.11" else "3.11"
        existing = (
            root / f"runtime/versions/{version}-cp{other_minor.replace('.', '')}-{platform_tag}"
        )
        existing.mkdir(parents=True)
        marker = existing / "preserve.txt"
        marker.write_text("another installed variant")
        command = ["bash", str(ROOT / "scripts/install.sh"), "--archive", str(archive)]
        subprocess.run(command, env=env, check=True)
        binary = root / "bin/pynox"
        doctor = json.loads(subprocess.check_output([binary, "doctor"]))
        assert Path(doctor["python"]["home"]).is_relative_to(root), doctor["python"]["home"]
        assert doctor["python"]["abi"] == abi
        assert doctor["python"]["version"] == built["python"]["version"]
        assert doctor["platform"] == platform_tag
        assert marker.read_text() == "another installed variant"
        prefix = subprocess.check_output(
            [binary, "python", "-I", "-c", "import sys; print(sys.base_prefix)"],
            text=True,
        ).strip()
        assert Path(prefix).resolve().is_relative_to(root), prefix
        command_prefix = [binary] if doctor["sandbox_supported"] else [binary, "python"]
        subprocess.run([*command_prefix, str(ROOT / "examples/hello.py")], check=True)
        repeat = subprocess.run(command, env=env, capture_output=True)
        assert repeat.returncode != 0 and b"already installed" in repeat.stderr
        mismatch = subprocess.run(
            command, env={**env, "PYNOX_PYTHON_VERSION": other_minor}, capture_output=True
        )
        assert mismatch.returncode == 2 and b"does not match" in mismatch.stderr
        other_platform = "macos-x86_64" if built["os"] == "linux" else "linux-gnu-x86_64"
        wrong_archive = root / f"pynox-{version}-{abi}-{other_platform}.tar.gz"
        incompatible = subprocess.run(
            ["bash", str(ROOT / "scripts/install.sh"), "--archive", str(wrong_archive)],
            env=env,
            capture_output=True,
        )
        assert incompatible.returncode == 2 and b"platform does not match" in incompatible.stderr
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
        assert not (root / f"rejected/versions/{version}-{abi}-{platform_tag}").exists()
    print("Installed bundle runs; repeat installation and corrupt archives rejected")


if __name__ == "__main__":
    main()
