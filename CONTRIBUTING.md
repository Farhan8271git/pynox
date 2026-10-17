# Contributing to Pynox

Use the Rust toolchain in `rust-toolchain.toml` and uv >= 0.13.1 on Linux
(x86_64 or ARM64). Sandbox tests require Landlock ABI >= 3, kernel 6.2 or newer.
On macOS, develop inside a Linux container or VM.
Default Python (3.11) is pinned in `.python-version`; the five
build/test pins for 3.11–3.15 are in `.python-versions`.

```bash
make setup
make build
make check
make test
make docs
make bundle
```

Select a version with `make test PYNOX_PYTHON_VERSION=3.15` or
`make bundle PYNOX_PYTHON_VERSION=3.15`. The Makefile selects both the embedded
engine and the Python test environment. Each build replaces the default binary;
use `CARGO_TARGET_DIR` / `PYNOX_BIN` explicitly when keeping separate local builds.
Every new minor needs ABI, behavior, real-kernel policy, framework and relocation
validation. Free-threaded variants are not supported.

`make clean` removes build/release artifacts, generated documentation and local
caches. `make distclean` also removes `.venv`. Restore the development environment
and executable with `make setup && make build`. Neither command removes source,
tests, canonical ADRs or recorded benchmark evidence.

The source layout follows pydynox:

```text
src/              Rust runtime, process control, sandbox and native adapters
python/pynox/     Embedded Python bootstrap and compatibility helpers
tests/rust/       Rust unit tests
tests/test_*.py   Python integration tests
scripts/          Development, packaging and validation tools
examples/         User-facing Python examples
```

`build.rs` compiles the Python helpers with the linked CPython and embeds the
bytecode; the executable does not need the source checkout to run. `python/pynox/` holds
internal runtime sources, not a separately installed Python package.

Preserve ordinary Python APIs and document every behavior difference. Keep
framework dependencies in optional test groups; never add them to the shipped
runtime.

Rust unit tests live in `tests/rust/`, separate from implementation files.
Run them with `make test-rust`; `make test` also runs the Python integration
suite, including the agent integration examples (`tests/test_agents.py`,
with Monty from the `integration` group). CI runs every Python/platform
combination: glibc and musl Linux on x86_64 and ARM64.
See [the test guide](tests/README.md) for the coverage split.

Python integration tests exercise the actual kernel, native calls and child
processes. Do not replace permission checks with mocks or silently skip unsupported hosts.
Optimization tests need a stock CPython reference, values/errors/side effects,
memory and complete-workload timing. Run comparable lifecycles.

Use conventional commit prefixes (`feat`, `fix`, `perf`, `docs`, `ci`, `chore`).
Add an ADR when changing an architectural contract. ADRs follow the numbered
format used in pydynox. Keep user-facing policy documentation in sync.

## Release

1. Update Cargo.toml, pyproject.toml, installer default and CHANGELOG.md together.
2. Regenerate Cargo.lock / uv.lock if affected and pass `make check`, tests,
   docs and relocated bundle tests across `.python-versions`. Keep every pin explicit.
3. Push the reviewed change to main. A `vX.Y.Z` tag triggers release CI.
4. CI reruns validation for all five Pythons across four Linux platforms, checks
   the tag against the Cargo version and uploads twenty bundles plus checksums to
   a **draft prerelease**. Pynox remains pre-alpha even when CPython itself is stable.
5. Review the draft's assets and notes, then publish it.

The workflow derives its repository from GitHub context. Creating/configuring
the GitHub repository, Pages and branch protection is an operator action.
Set required checks for Linux and quality; permit publishing only after
review. No PyPI package or mandatory service token is needed.

Documentation uses Zensical, as pydynox does. PRs/main validate the site;
manual workflow dispatch with `deploy=true` publishes Pages after Pages is
enabled on the repository. Dependency updates are grouped by Dependabot.
