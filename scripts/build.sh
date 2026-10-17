#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
if [[ -z "${PYO3_PYTHON:-}" ]]; then
    request="${PYNOX_PYTHON_VERSION:-$(cat .python-version)}"
    selected="$(awk -F. -v request="$request" '$0 == request || $1 "." $2 == request { print; exit }' .python-versions)"
    [[ -n "$selected" ]] || { echo "Unsupported Python request: $request (choose 3.11–3.15)" >&2; exit 2; }
    if ! PYO3_PYTHON="$(uv python find --system --managed-python "$selected" 2>/dev/null)"; then
        uv python install "$selected"
        PYO3_PYTHON="$(uv python find --system --managed-python "$selected")"
    fi
    export PYO3_PYTHON
fi
if [[ "${1:-}" == "--check" ]]; then
    cargo clippy --locked --all-targets -- -D warnings
else
    cargo build --release --locked
fi
