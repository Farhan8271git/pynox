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
case "${1:-}" in
    --check) cargo clippy --locked --all-targets -- -D warnings ;;
    --test) cargo test --locked --all-targets ;;
    "") cargo build --release --locked ;;
    *) echo "Usage: bash scripts/build.sh [--check|--test]" >&2; exit 2 ;;
esac
