.PHONY: setup build check test docs bundle
setup:
	uv sync --locked
build:
	bash scripts/build.sh
check:
	cargo fmt --all -- --check
	PYO3_PYTHON="$$(uv python find --system --managed-python "$$(cat .python-version)")" cargo clippy --locked --all-targets -- -D warnings
	uv run --locked ruff check src tests scripts examples
	uv run --locked ruff format --check src tests scripts examples
	uv run --locked python scripts/check_metadata.py
test: build
	uv run --locked pytest
docs:
	uv run --locked python scripts/prepare_docs.py
	uv run --locked --group docs zensical build
bundle: build
	uv run --locked python scripts/package.py
