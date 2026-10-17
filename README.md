# Pynox

A Python runtime with explicit permissions and native optimizations.

Pynox runs existing Python on **CPython**, with a Rust launcher. Its default
execution mode applies filesystem permissions, network policy and resource
limits through the Linux kernel. An explicit Python mode provides CPython's CLI.
Application frameworks are installed by the user, not included in Pynox.

**Pre-alpha (0.1.0): for evaluation and development, not production use.**
CLI, sandbox policies and distribution layout may change. Pynox builds against
**CPython 3.11 through 3.15**, with the GIL, on Linux x86_64/aarch64.
Each binary embeds one Python minor version; this is not a universal ABI binary.
Landlock ABI 3 or newer is required for restricted execution, which fails closed
when setup is unsupported. No universal Python speedup is claimed.

| CPython | Pinned build | Role |
| --- | --- | --- |
| 3.11 | 3.11.17 | Minimum supported version and default |
| 3.12 | 3.12.15 | Build and test target |
| 3.13 | 3.13.16 | Build and test target |
| 3.14 | 3.14.8 | Build and test target |
| 3.15 | 3.15.0 | Build and test target |

The pins live in [.python-versions](.python-versions). Python 3.11 remains in
security support; supporting it broadens compatibility without targeting an
end-of-life version. The pre-alpha status applies to Pynox, independently of the
CPython release status.

## Build and run

Development requires Rust and uv >= 0.13.1. `uv` obtains the pinned Python automatically.

```bash
make setup
make build
./target/release/pynox doctor
./target/release/pynox examples/hello.py
```

`pynox --version` (or `-V`) reports both versions:
`pynox 0.1.0 (CPython 3.11.17)` for the default build.

Choose another supported Python minor or an exact pinned version:

```bash
make build PYNOX_PYTHON_VERSION=3.14
./target/release/pynox --version
# pynox 0.1.0 (CPython 3.14.8)
make test PYNOX_PYTHON_VERSION=3.14
```

Changing the selection rebuilds `target/release/pynox` for that Python.
`PYO3_PYTHON=/path/to/python` can select a custom CPython with shared libpython;
release bundles require the exact checked-in pins.

Restricted execution returns a JSON result containing stdout, stderr, exit
status, a concise error and the effective policy. The CLI exits with the result's
status. Only the script and runtime are implicitly readable; grant other paths:

```bash
mkdir -p examples/out
./target/release/pynox run \
  --allow-read=examples/data --allow-write=examples/out \
  examples/json_open.py examples/data/orders.json examples/out/orders.json
```

The script uses ordinary `open`, `json.load` and `json.dump`. Existing output
directories must be created before running. Writes do not imply read access.

```bash
# Opt into the first experimental optimization, with additional memory usage.
./target/release/pynox --optimize-json \
  --allow-read=examples/data --allow-write=examples/out \
  examples/json_open.py examples/data/orders.json examples/out/orders.json

# Allow IPv4/IPv6 explicitly; host Unix sockets and subprocesses remain denied.
./target/release/pynox --allow-network --allow-env=API_KEY app.py

# CPython CLI and normal output, WITHOUT sandbox or resource limits.
./target/release/pynox python -c 'print("ordinary Python")'
./target/release/pynox python -m json.tool --help
```

The JSON optimization is off by default. It accelerates eligible `json.dump`
calls by using CPython's existing native encoder and batching the write. It
does not replace `open`, `json.load`, the interpreter, or application packages.
See [performance and compatibility](docs/performance.md).

## Repeated tasks

```bash
./target/release/pynox pool --workers=2
```

Send one JSON request per line:

```json
{"id":"hello","script":"examples/hello.py","timeout":"2s"}
```

Every task runs in a disposable child of a clean CPython template. The pool
retains initialization, not user task state. Each result carries its request ID;
concurrent results may arrive out of order. Pooling is a lifecycle feature,
not evidence of a faster interpreter. See [pool and CLI](docs/usage.md).

## Install a release bundle

Bundles contain Pynox and its matching CPython, so end users need neither Python
nor Rust installed. For a locally built first release:

```bash
make bundle
bash scripts/install.sh --archive dist/pynox-0.1.0-cp311-linux-$(uname -m).tar.gz
~/.local/bin/pynox doctor
```

The installer infers Python from the local archive name, verifies the adjacent
`.sha256`, installs a directory keyed by Pynox version and Python ABI, and updates
the CLI symlink. Builds for different Python versions can coexist.
With no `--archive`, it fetches GitHub release
assets from `PYNOX_REPOSITORY` (default `ferrumio/pynox`).
Set `PYNOX_PYTHON_VERSION=3.14` to download the 3.14 variant (default: 3.11).
Those assets must first be published; creating this source tree does not publish
a release. Installation details and upgrade behavior are in
[distribution](docs/distribution.md).

## Development

```bash
make check
uv run --locked --group integration pytest
make docs
```

GitHub workflows cover formatting, lint, actual kernel policy tests, CPython
behavior comparisons, framework smoke tests, documentation and release bundles.
The configured matrix has **five Python versions × two architectures**.
Native x86_64-only checks run on x86_64. All variants are pre-alpha, and remote
CI results only exist after the workflows run on GitHub.

- [Architecture decisions](ADR/README.md)
- [Sandbox contract](docs/sandbox.md)
- [Contributing and releases](CONTRIBUTING.md)
- [Security scope](SECURITY.md)
- [Historical research](docs/research.md)

Licensed under Apache-2.0. Bundles retain CPython's upstream licenses.
