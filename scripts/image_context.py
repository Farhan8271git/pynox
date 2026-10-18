"""Prepare a Docker build context from release bundles.

    python scripts/image_context.py CONTEXT_DIR dist/pynox-0.1.0-cp312-linux-gnu-*.tar.gz

Each archive is extracted to bundle-<docker arch>/ beside deploy/docker/Dockerfile,
so `docker buildx build --platform linux/amd64,linux/arm64 CONTEXT_DIR`
produces one multi-architecture image for one CPython minor and libc.
"""

import re
import shutil
import sys
import tarfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ARCHES = {"x86_64": "amd64", "aarch64": "arm64"}
NAME = re.compile(
    r"pynox-(?P<version>[0-9.]+)-cp(?P<abi>3\d+)-linux-(?P<libc>gnu|musl)-(?P<arch>x86_64|aarch64)\.tar\.gz$"
)


def main():
    if len(sys.argv) < 3:
        raise SystemExit(__doc__)
    context = Path(sys.argv[1])
    archives = [Path(a) for a in sys.argv[2:]]
    shutil.rmtree(context, ignore_errors=True)
    context.mkdir(parents=True)
    shutil.copy2(ROOT / "deploy/docker/Dockerfile", context / "Dockerfile")
    seen = set()
    for archive in archives:
        match = NAME.search(archive.name)
        if not match:
            raise SystemExit(f"Not a Pynox bundle: {archive}")
        seen.add((match["version"], match["abi"], match["libc"]))
        target = context / f"bundle-{ARCHES[match['arch']]}"
        if target.exists():
            raise SystemExit(f"Two bundles for {match['arch']}")
        with tarfile.open(archive) as tar:
            tar.extractall(context / "extract", filter="data")
        next((context / "extract").iterdir()).rename(target)
        (context / "extract").rmdir()
    if len(seen) != 1:
        raise SystemExit(f"Bundles must share version, Python and libc: {sorted(seen)}")
    version, abi, libc = seen.pop()
    print(
        f"version={version} python=3.{abi[1:]} libc={libc} arches={sorted(p.name for p in context.glob('bundle-*'))}"
    )


if __name__ == "__main__":
    main()
