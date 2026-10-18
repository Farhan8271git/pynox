"""Build a platform wheel: the `pynox` package plus the bundle for one CPython.

The wheel's tag matches the CPython the binary embeds (cp311-cp311-...), so
`pip install pynox` in a 3.11 environment gets a sandbox that runs 3.11 and
can import that environment's packages. Symlinked duplicates of large files
are dropped; wheels cannot carry symlinks and the runtime does not need them.
"""

import base64
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PACKAGE = ROOT / "python/pynox"


def record_hash(data: bytes) -> str:
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=")
    return f"sha256={digest.decode()}"


def main():
    binary = Path(os.environ.get("PYNOX_BIN", ROOT / "target/release/pynox")).resolve()
    info = json.loads(subprocess.check_output([binary, "doctor"]))
    version, abi, tag = info["version"], info["python"]["abi"], info["platform"]
    archive = ROOT / f"dist/pynox-{version}-{abi}-{tag}.tar.gz"
    if not archive.is_file():
        raise SystemExit(f"Run `make bundle` first; missing {archive}")
    family, arch = tag.removeprefix("linux-").split("-", 1)
    platform_tag = {"gnu": f"manylinux_2_28_{arch}", "musl": f"musllinux_1_2_{arch}"}[family]
    wheel_tag = f"{abi}-{abi}-{platform_tag}"
    name = f"pynox-{version}-{wheel_tag}.whl"
    output = ROOT / "dist" / name
    with tempfile.TemporaryDirectory(prefix="pynox-wheel-") as temporary:
        staging = Path(temporary)
        with tarfile.open(archive) as tar:
            tar.extractall(staging / "bundle", filter="data")
        bundle = next((staging / "bundle").iterdir())
        files: list[tuple[str, Path]] = []
        for source in sorted(PACKAGE.rglob("*.py")):
            if "__pycache__" in source.parts:
                continue
            files.append((f"pynox/{source.relative_to(PACKAGE)}", source))
        files.append(("pynox/py.typed", PACKAGE / "py.typed"))
        for part in ("bin", "python"):
            for source in sorted((bundle / part).rglob("*")):
                if source.is_dir():
                    continue
                if source.is_symlink():
                    target = source.resolve()
                    if not target.is_file() or target.stat().st_size > 1_000_000:
                        continue
                files.append((f"pynox/_bundle/{source.relative_to(bundle)}", source))
        dist_info = f"pynox-{version}.dist-info"
        metadata = "\n".join(
            [
                "Metadata-Version: 2.1",
                "Name: pynox",
                f"Version: {version}",
                "Summary: A Python with the sandbox built in, made for the code your agents write",
                "Home-page: https://github.com/ferrumio/pynox",
                "License: Apache-2.0",
                f"Requires-Python: =={info['python']['major_minor']}.*",
                "Classifier: Development Status :: 2 - Pre-Alpha",
                "Classifier: Operating System :: POSIX :: Linux",
                "Classifier: Programming Language :: Python :: 3",
                "Description-Content-Type: text/markdown",
                "",
                (ROOT / "README.md").read_text(),
            ]
        )
        wheel_meta = "\n".join(
            [
                "Wheel-Version: 1.0",
                "Generator: pynox scripts/wheel.py",
                "Root-Is-Purelib: false",
                f"Tag: {wheel_tag}",
                "",
            ]
        )
        entry_points = "[console_scripts]\npynox = pynox._cli:main\n"
        output.parent.mkdir(exist_ok=True)
        records = []
        with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as wheel:

            def add(arcname: str, data: bytes, mode: int = 0o644):
                entry = zipfile.ZipInfo(arcname, date_time=(1980, 1, 1, 0, 0, 0))
                entry.compress_type = zipfile.ZIP_DEFLATED
                entry.external_attr = (0o100000 | mode) << 16
                wheel.writestr(entry, data)
                records.append(f"{arcname},{record_hash(data)},{len(data)}")

            for arcname, source in files:
                mode = 0o755 if os.access(source, os.X_OK) else 0o644
                add(arcname, source.read_bytes(), mode)
            add(f"{dist_info}/METADATA", metadata.encode())
            add(f"{dist_info}/WHEEL", wheel_meta.encode())
            add(f"{dist_info}/entry_points.txt", entry_points.encode())
            add(f"{dist_info}/licenses/LICENSE", (ROOT / "LICENSE").read_bytes())
            records.append(f"{dist_info}/RECORD,,")
            add(f"{dist_info}/RECORD", ("\n".join(records) + "\n").encode())
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(output.suffix + ".sha256").write_text(f"{digest}  {output.name}\n")
    print(f"{output} ({output.stat().st_size / 1024 / 1024:.1f} MiB)")
    if shutil.which("unzip") is None:
        sys.exit(0)


if __name__ == "__main__":
    main()
