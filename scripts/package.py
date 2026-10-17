"""Build a relocatable release containing Pynox and its matching CPython only."""

import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess
import tarfile
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def patch_loader(binary):
    subprocess.run(["patchelf", "--set-rpath", "$ORIGIN/../python/lib", binary], check=True)


def libc_metadata(bundle, tag):
    if tag.startswith("linux-musl-"):
        return {"family": "musl", "minimum": "1.2"}
    versions = {(2, 0)}
    for path in bundle.rglob("*"):
        if not path.is_file() or path.is_symlink():
            continue
        with path.open("rb") as file:
            if file.read(4) != b"\x7fELF":
                continue
        output = run("readelf", "--version-info", str(path))
        versions.update(
            tuple(map(int, value.split(".")))
            for value in re.findall(r"\bGLIBC_(\d+(?:\.\d+)+)\b", output)
        )
    minimum = max(versions)
    ceiling = os.environ.get("PYNOX_GLIBC_MAX")
    if ceiling and minimum > tuple(map(int, ceiling.split("."))):
        raise RuntimeError(f"Bundle needs glibc {minimum}, above release baseline {ceiling}")
    return {"family": "glibc", "minimum": ".".join(map(str, minimum))}


def main():
    binary = Path(os.environ.get("PYNOX_BIN", ROOT / "target/release/pynox")).resolve()
    info = json.loads(run(str(binary), "doctor"))
    version = info["version"]
    assert info["python"]["version"] in (ROOT / ".python-versions").read_text().splitlines()
    home = Path(info["python"]["home"])
    major_minor = info["python"]["major_minor"]
    abi = info["python"]["abi"]
    # Preserve upstream bootstrap tooling (3.11 includes setuptools), not
    # application frameworks installed into a developer's Python.
    bootstrap = {
        "README.txt",
        "pip",
        "setuptools",
        "_distutils_hack",
        "distutils-precedence.pth",
        "__pycache__",
    }
    for item in (home / f"lib/python{major_minor}/site-packages").iterdir():
        if item.name not in bootstrap and not (
            item.name.startswith(("pip-", "setuptools-")) and item.name.endswith(".dist-info")
        ):
            raise RuntimeError(f"Use a clean managed CPython; unexpected package: {item.name}")
    tag = info["platform"]
    assert info["os"] == "linux", info
    name = f"pynox-{version}-{abi}-{tag}"
    output = ROOT / "dist"
    output.mkdir(exist_ok=True)
    if not shutil.which("patchelf"):
        raise SystemExit("Run with uv run --locked python scripts/package.py")
    with tempfile.TemporaryDirectory(prefix="pynox-package-") as temporary:
        staging = Path(temporary).resolve()
        bundle = staging / name
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
        patch_loader(bundle / "bin/pynox")
        for link in bundle.rglob("*"):
            if link.is_symlink() and not link.resolve().is_relative_to(bundle.resolve()):
                raise RuntimeError(f"Bundle symlink escapes release: {link}")
        metadata = {
            "pynox": version,
            "python": info["python"]["version"],
            "architecture": info["architecture"],
            "os": "linux",
            "platform": tag,
            "libc": libc_metadata(bundle, tag),
            "sandbox": info["sandbox_supported"],
            "abi": abi,
            "packages": "CPython distribution only; application dependencies are not bundled",
            "files": {
                str(p.relative_to(bundle)): hashlib.sha256(p.read_bytes()).hexdigest()
                for p in sorted(bundle.rglob("*"))
                if p.is_file() and not p.is_symlink()
            },
        }
        (bundle / "manifest.json").write_text(json.dumps(metadata, indent=2) + "\n")
        # Exercise relocation, extensions and actual sandbox setup before archiving.
        moved = staging / "relocated"
        bundle.rename(moved)
        relocated = moved / "bin/pynox"
        probe = json.loads(run(str(relocated), "doctor"))
        assert Path(probe["python"]["home"]) == moved / "python"
        loaded_libraries = "import pathlib; libraries = pathlib.Path('/proc/self/maps').read_text()"
        run(
            str(relocated),
            "-I",
            "-c",
            loaded_libraries + f"; assert {str(moved / 'python/lib/libpython')!r} in libraries",
        )
        run(
            str(relocated),
            "-I",
            "-c",
            f"import ssl, sqlite3, zlib, sys; assert '.'.join(map(str, sys.version_info[:2])) == {major_minor!r}",
        )
        script = staging / "smoke.py"
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
        report = json.loads(run(str(relocated), "sandbox", str(script)))
        assert report["exit"] == 0 and report["stdout"] == "sandbox enforced\n", report
        # The default mode is plain Python: the same script must read the file.
        plain = subprocess.run([relocated, script], capture_output=True, text=True)
        assert plain.returncode == 1 and "sandbox absent" in plain.stderr, plain
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
