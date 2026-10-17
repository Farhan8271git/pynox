"""Build a relocatable release containing Pynox and its matching CPython only."""

import gzip
import hashlib
import json
import os
import platform
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    binary = ROOT / "target/release/pynox"
    info = json.loads(run(str(binary), "doctor"))
    version = info["version"]
    assert info["python"]["version"] == (ROOT / ".python-version").read_text().strip()
    home = Path(info["python"]["home"])
    # A release must never accidentally ship a developer's installed frameworks.
    for item in (home / "lib/python3.12/site-packages").iterdir():
        if item.name not in ("README.txt", "pip", "__pycache__") and not (
            item.name.startswith("pip-") and item.name.endswith(".dist-info")
        ):
            raise RuntimeError(f"Use a clean managed CPython; unexpected package: {item.name}")
    name = f"pynox-{version}-cp312-linux-{platform.machine()}"
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    if not shutil.which("patchelf"):
        raise SystemExit("Run with uv run --locked python scripts/package.py")
    with tempfile.TemporaryDirectory(prefix="pynox-package-") as temporary:
        bundle = Path(temporary) / name
        (bundle / "bin").mkdir(parents=True)
        shutil.copy2(binary, bundle / "bin/pynox")
        shutil.copytree(home, bundle / "python", symlinks=True)
        for filename in ("LICENSE", "README.md", "CONTRIBUTING.md", "SECURITY.md", "CHANGELOG.md"):
            shutil.copy2(ROOT / filename, bundle / filename)
        shutil.copytree(ROOT / "ADR", bundle / "ADR")
        shutil.copytree(ROOT / "docs", bundle / "docs", ignore=shutil.ignore_patterns("adr"))
        shutil.copytree(ROOT / "ADR", bundle / "docs/adr")
        shutil.copytree(
            ROOT / "examples",
            bundle / "examples",
            ignore=shutil.ignore_patterns("out", "__pycache__"),
        )
        # Erase build-host loader paths. The ELF loader resolves this before main().
        subprocess.run(
            ["patchelf", "--set-rpath", "$ORIGIN/../python/lib", bundle / "bin/pynox"],
            check=True,
        )
        for link in bundle.rglob("*"):
            if link.is_symlink() and not link.resolve().is_relative_to(bundle.resolve()):
                raise RuntimeError(f"Bundle symlink escapes release: {link}")
        metadata = {
            "pynox": version,
            "python": info["python"]["version"],
            "architecture": platform.machine(),
            "os": "linux",
            "abi": "cp312",
            "packages": "CPython distribution only; application dependencies are not bundled",
            "files": {
                str(p.relative_to(bundle)): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(bundle.rglob("*"))
                if p.is_file() and not p.is_symlink()
            },
        }
        (bundle / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n")
        # Exercise relocation, extensions and actual sandbox setup before archiving.
        moved = Path(temporary) / "relocated"
        bundle.rename(moved)
        relocated = moved / "bin/pynox"
        probe = json.loads(run(str(relocated), "doctor"))
        assert Path(probe["python"]["home"]) == moved / "python"
        run(
            str(relocated),
            "python",
            "-I",
            "-c",
            "import pathlib; "
            f"assert {str(moved / 'python/lib/libpython')!r} in "
            "pathlib.Path('/proc/self/maps').read_text()",
        )
        run(
            str(relocated),
            "python",
            "-I",
            "-c",
            "import ssl, sqlite3, zlib, sys; assert sys.version_info[:2] == (3,12)",
        )
        script = Path(temporary) / "smoke.py"
        script.write_text(
            "import asyncio\n"
            "async def main():\n"
            "    await asyncio.sleep(0)\n"
            "asyncio.run(main())\n"
            "try:\n"
            "    open('/etc/passwd').read()\n"
            "except PermissionError:\n"
            "    print('sandbox enforced')\n"
            "else:\n"
            "    raise AssertionError('sandbox absent')\n"
        )
        report = json.loads(run(str(relocated), str(script)))
        assert report["exit"] == 0 and report["stdout"] == "sandbox enforced\n", report
        archive = output / f"{name}.tar.gz"
        epoch = int(os.environ.get("SOURCE_DATE_EPOCH", "0"))

        def normalize(member):
            member.uid = member.gid = 0
            member.uname = member.gname = ""
            member.mtime = epoch
            return member

        with archive.open("wb") as raw:
            with gzip.GzipFile(fileobj=raw, mode="wb", filename="", mtime=epoch) as compressed:
                with tarfile.open(fileobj=compressed, mode="w|") as tar:
                    tar.add(moved, arcname=name, filter=normalize)
        digest = hashlib.sha256(archive.read_bytes()).hexdigest()
        archive.with_suffix(archive.suffix + ".sha256").write_text(f"{digest}  {archive.name}\n")
        print(f"{archive} ({archive.stat().st_size / 1024 / 1024:.1f} MiB)")


if __name__ == "__main__":
    main()
