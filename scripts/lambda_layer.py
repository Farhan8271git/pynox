"""Build an AWS Lambda layer from a glibc bundle.

    python scripts/lambda_layer.py dist/pynox-0.1.0-cp311-linux-gnu-aarch64.tar.gz

The layer unpacks to /opt: /opt/pynox (binary and CPython), /opt/bootstrap and
/opt/pynox-runtime/runtime.py, so a `provided.al2023` function with this layer
and no bootstrap of its own runs `_HANDLER` on Pynox without the sandbox.
"""

import hashlib
import re
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAME = re.compile(
    r"pynox-(?P<version>[0-9.]+)-(?P<abi>cp3\d+)-linux-gnu-(?P<arch>x86_64|aarch64)\.tar\.gz$"
)


# Parts of a CPython install that a Lambda function cannot use: GUI toolkits,
# the IDE, package bootstrapping, C headers and installer tooling. Dropping
# them shrinks what Lambda unpacks at every cold start.
EXCLUDE = (
    "idlelib/",
    "tkinter/",
    "turtledemo/",
    "ensurepip/",
    "lib2to3/",
    "site-packages/",
    "config-",
    "lib-dynload/_tkinter",
    "libtcl",
    "libtk",
    "tcl",
    "tk",
    "itcl",
    "thread2",
    "include/",
    "share/",
)


def excluded(relative: str) -> bool:
    parts = relative.split("/")
    if parts[:2] == ["python", "include"] or parts[:2] == ["python", "share"]:
        return True
    tail = "/".join(parts[1:])
    return any(
        tail.startswith(f"lib/python3.{minor}/{name}") or tail.startswith(f"lib/{name}")
        for name in EXCLUDE
        for minor in range(11, 16)
    )


def add(layer, path, arcname):
    info = zipfile.ZipInfo(arcname, date_time=(1980, 1, 1, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    mode = path.stat().st_mode & 0o777
    info.external_attr = (0o100000 | mode) << 16
    layer.writestr(info, path.read_bytes())


def main():
    if len(sys.argv) != 2:
        raise SystemExit(__doc__)
    archive = Path(sys.argv[1])
    match = NAME.search(archive.name)
    if not match:
        raise SystemExit("Lambda layers are built from linux-gnu bundles")
    arch = {"x86_64": "x86_64", "aarch64": "arm64"}[match["arch"]]
    output = ROOT / "dist" / f"pynox-lambda-layer-{match['version']}-{match['abi']}-{arch}.zip"
    with tempfile.TemporaryDirectory(prefix="pynox-layer-") as temporary:
        with tarfile.open(archive) as tar:
            tar.extractall(temporary, filter="data")
        bundle = next(Path(temporary).iterdir())
        output.parent.mkdir(exist_ok=True)
        with zipfile.ZipFile(output, "w") as layer:
            for part in ("bin", "python"):
                for path in sorted((bundle / part).rglob("*")):
                    if excluded(str(path.relative_to(bundle))):
                        continue
                    if path.is_file() and not path.is_symlink():
                        add(layer, path, f"pynox/{path.relative_to(bundle)}")
                    elif path.is_symlink() and path.resolve().is_file():
                        # Zip has no symlinks; keep small aliases such as python3 -> python3.11.
                        if path.resolve().stat().st_size < 1_000_000:
                            add(layer, path.resolve(), f"pynox/{path.relative_to(bundle)}")
            add(layer, ROOT / "deploy/lambda/bootstrap", "bootstrap")
            add(layer, ROOT / "deploy/lambda/runtime.py", "pynox-runtime/runtime.py")
            add(layer, ROOT / "LICENSE", "pynox/LICENSE")
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(".zip.sha256").write_text(f"{digest}  {output.name}\n")
    print(f"{output} ({output.stat().st_size / 1024 / 1024:.1f} MiB)")


if __name__ == "__main__":
    main()
