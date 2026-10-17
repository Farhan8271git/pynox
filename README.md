# Pynox

A Python runtime with explicit permissions and native optimizations.

Pynox runs existing Python on **CPython**, with a Rust launcher. Its default
execution mode applies filesystem permissions, network policy and resource
limits through the Linux kernel. An explicit Python mode provides CPython's CLI.
Application frameworks are installed by the user, not included in Pynox.

**Version 0.1.0 is an initial developer release.** Supported target:
Linux x86_64/aarch64, CPython 3.12 with GIL, Landlock ABI 3 or newer.
The release pins CPython 3.12.14. Unsupported sandbox setup fails closed.
This release does not claim that Python programs universally run faster.

## Build and run

Development requires Rust and uv. `uv` obtains the pinned Python automatically.

```bash
make setup
make build
./target/release/pynox doctor
./target/release/pynox examples/hello.py
```

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
bash scripts/install.sh --archive dist/pynox-0.1.0-cp312-linux-$(uname -m).tar.gz
~/.local/bin/pynox doctor
```

The installer verifies the adjacent `.sha256`, installs a versioned directory
and updates the CLI symlink. With no `--archive`, it fetches GitHub release
assets from `PYNOX_REPOSITORY` (default `ferrumio/pynox`).
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
They test both supported architectures. Native x86_64-only checks run on x86_64.

- [Architecture decisions](ADR/README.md)
- [Sandbox contract](docs/sandbox.md)
- [Contributing and releases](CONTRIBUTING.md)
- [Security scope](SECURITY.md)
- [Historical research](docs/research.md)

Licensed under Apache-2.0. Bundles retain CPython's upstream licenses.
