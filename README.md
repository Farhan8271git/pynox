# Pynox 🐍⚙️

![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)
[![CPython 3.11–3.15](https://img.shields.io/badge/CPython-3.11%E2%80%933.15-blue)](#python-versions)
![Linux x86_64 / aarch64](https://img.shields.io/badge/Linux-x86__64%20%7C%20aarch64-lightgrey)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-green)](LICENSE)

Your Python, with a sandbox by default and Rust under the hood.

Pynox runs ordinary Python on **CPython**, with Rust handling execution control
and selected native optimizations. Use it for scripts, data processing,
automation and agent tools.

> 🚧 **Pre-alpha (0.1.0).** Ready to try, experiment and contribute.
> Interfaces and sandbox policies are still evolving; this is not a production release.

## Why Pynox?

- **Keep writing Python** — Familiar `.py` files, imports and libraries.
- **Sandbox by default** — Choose which files, environment variables and network access a script gets.
- **Control each run** — Set time, memory and output limits.
- **Get useful results** — JSON output with captured stdout, exit status, timing and error details.
- **Reuse startup work** — A worker pool keeps Python ready and gives each job fresh process state.
- **Bring your dependencies** — Install the packages you need. Pynox bundles its Python runtime, without application frameworks.

## Quick start

For now, build from a checkout. You'll need **Linux x86_64 or aarch64**, Rust
and **uv >= 0.13.1**. `uv` downloads the matching Python for you.

```bash
make setup
make build

# Make the local build available in this shell.
export PATH="$PWD/target/release:$PATH"
pynox doctor
```

Create `hello.py`:

```python
name = "Pynox"
print(f"Hello from {name}!")
```

Run it in the sandbox:

```bash
pynox hello.py
```

The command returns a JSON result with `Hello from Pynox!\n` in `stdout`,
plus the exit status, timing and permissions used. When a script fails, the
result includes the error type, message and script location when available.

The sandbox requires **Landlock ABI 3 or newer**. Pynox refuses restricted
execution if the kernel cannot enforce it. See the [sandbox guide](docs/sandbox.md).

## Give scripts the access they need

Grant access to input data and an output folder. This included example uses
ordinary `open`, `json.load` and `json.dump`:

```bash
mkdir -p examples/out
pynox run \
  --allow-read=examples/data \
  --allow-write=examples/out \
  --timeout=5s \
  examples/json_open.py examples/data/orders.json examples/out/orders.json
```

Create output directories first. Read and write access are separate grants.
Sibling imports and third-party packages also need access; see the
[command reference](docs/usage.md).

For a script that calls an API, allow network access and pass its API key:

```bash
pynox --allow-network --allow-env=API_KEY app.py
```

Network access is off by default. The flag enables IPv4/IPv6; host Unix sockets
and subprocesses stay blocked.

## Run repeated jobs

Keep Python initialized across jobs with a worker pool:

```bash
pynox pool --workers=2
```

After the `ready` event, send one JSON request per line:

```json
{"id":"hello","script":"examples/hello.py","timeout":"2s"}
```

Each job runs in a fresh child process with its own permissions. Results carry
the request ID and may arrive out of order. Python globals are not shared
between jobs; authorized file writes persist.
See [pool usage](docs/usage.md#pool) for preloading and integration details.

## Use the regular Python CLI

Use `pynox python` for `-c`, `-m` and normal terminal output.
This mode runs **without the sandbox or resource limits**:

```bash
pynox python -c 'print("Hello from Python!")'
pynox python -m json.tool --help
```

## What about speed?

The goal is to make real Python workloads faster. Today, Pynox can reuse
startup work through the pool and offers an experimental `json.dump`
optimization: add `--optimize-json` before the script to try it.

That optimization batches eligible JSON writes and uses extra memory, so it
is off by default. Speedups depend on the workload; Pynox does not yet make
Python code faster across the board. See [measurements and tradeoffs](docs/performance.md).

## Python versions

Pynox builds with **CPython 3.11–3.15**, using **3.11 by default**.
Choose another version when building:

```bash
make build PYNOX_PYTHON_VERSION=3.14
pynox --version
# pynox 0.1.0 (CPython 3.14.8)
```

`--version` always shows both Pynox and its Python runtime. Each binary uses
one Python version, with the GIL. Exact pins live in [.python-versions](.python-versions);
see [distribution](docs/distribution.md) for custom builds and version management.

## Install a bundle

Bundles include Pynox and its matching Python, so the target machine needs
neither Python nor Rust installed. Build and install a local bundle:

```bash
make bundle
bash scripts/install.sh --archive dist/pynox-0.1.0-cp311-linux-$(uname -m).tar.gz
~/.local/bin/pynox --version
```

The installer checks the checksum, selects the Python version from the archive
and preserves other installed variants. Add `~/.local/bin` to your `PATH` to
use the installed `pynox`. GitHub downloads require published release assets;
see [installation options](docs/distribution.md).

## Contributing

Try your scripts, report unexpected behavior and help us measure real workloads.
The [contributor guide](CONTRIBUTING.md) covers setup, checks and releases.

## Documentation

- [Commands and worker pools](docs/usage.md)
- [Sandbox and permissions](docs/sandbox.md)
- [Installation and Python versions](docs/distribution.md)
- [Performance and compatibility](docs/performance.md)
- [Architecture decisions](ADR/README.md)
- [Security scope](SECURITY.md)

## License

[Apache 2.0](LICENSE). Bundles retain CPython's upstream licenses.
