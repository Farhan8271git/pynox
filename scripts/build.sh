#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -z "${PYO3_PYTHON:-}" ]]; then
    uv python install "$(cat .python-version)"
    export PYO3_PYTHON
    PYO3_PYTHON="$(uv python find --system --managed-python "$(cat .python-version)")"
fi
cargo build --release --locked
