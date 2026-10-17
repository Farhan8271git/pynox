#!/usr/bin/env bash
# Build inside a pinned PyPA userspace; exercise Pynox's own kernel restrictions.
set -euo pipefail
cd "$(dirname "$0")/.."
family="${1:?Choose gnu or musl}"
python_version="${2:-$(cat .python-version)}"
architecture="$(uname -m)"
image="$(python3 - "$family" "$architecture" <<'PY'
import json
from pathlib import Path
import sys
print(json.loads(Path(".ci/images.json").read_text())[f"linux-{sys.argv[1]}-{sys.argv[2]}"])
PY
)"
output="${3:-$PWD/dist}"
mkdir -p "$output"
output="$(cd "$output" && pwd)"
docker run --rm \
    --security-opt seccomp=unconfined \
    --security-opt no-new-privileges \
    -v "$PWD:/source:ro" -v "$output:/artifacts" \
    -e "PYNOX_PYTHON_VERSION=$python_version" \
    -e "PYNOX_LINUX_FAMILY=$family" \
    -e "PYNOX_OUTPUT_OWNER=$(id -u):$(id -g)" \
    "$image" bash /source/scripts/ci-container.sh
