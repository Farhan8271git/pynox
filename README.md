# Pynox 🐍⚙️

![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)
[![CPython 3.11–3.15](https://img.shields.io/badge/CPython-3.11%E2%80%933.15-blue)](#python-versions)
![Linux and macOS](https://img.shields.io/badge/platforms-Linux%20%7C%20macOS-lightgrey)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-green)](LICENSE)

**Let your agents run Python. Keep control of each task.**

Pynox gives AI agents a controlled way to run real Python: permissions per
task, resource limits and structured results. It runs ordinary Python on
**CPython**, with Rust handling execution control and selected native
optimizations. The same runtime serves your scripts, data jobs and automation.

> 🚧 **Pre-alpha (0.1.0).** Ready to try, experiment and contribute.
> Interfaces and sandbox policies are still evolving; this is not a production release.

| Platform | Architectures | Available modes |
| --- | --- | --- |
| Linux glibc | x86_64, ARM64 | Sandbox, pool and native Python |
| Linux musl / Alpine | x86_64, ARM64 | Sandbox, pool and native Python |
| macOS | Intel, Apple Silicon | Native Python; sandbox and pool are not available yet |

Linux release builds use manylinux 2.28 and musllinux 1.2 environments.
See [platforms and installation](docs/distribution.md) for compatibility details.

## Why Pynox?

- **Keep writing Python** — Familiar `.py` files, imports and libraries.
- **Sandbox by default on Linux** — Choose which files, environment variables and network access a script gets.
- **Control each run** — Set time, memory and output limits.
- **Fit your agent loop** — JSON results with captured stdout, exit status, timing and structured errors.
- **Reuse startup work** — A worker pool keeps Python ready and gives each job fresh process state.
- **Bring your dependencies** — Install the packages you need. Pynox bundles its Python runtime, without application frameworks.

## Quick start

For now, build from a checkout. You'll need **Linux or macOS on x86_64 or ARM64**, Rust
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

On Linux, run it in the sandbox:

```bash
pynox hello.py
```

The command returns a JSON result with `Hello from Pynox!\n` in `stdout`,
plus the exit status, timing and permissions used. When a script fails, the
result includes the error type, message and script location when available.

The sandbox requires **Landlock ABI 3 or newer**. Pynox refuses restricted
execution if the kernel cannot enforce it. See the [sandbox guide](docs/sandbox.md).

On macOS, explicitly use the native Python mode:

```bash
pynox python hello.py
```

This mode has ordinary Python output and no sandbox or resource limits.
Sandbox and pool commands on macOS return `SandboxUnavailable` without running
the task. They never silently fall back to unrestricted execution.

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

## Connect an agent to the worker pool

Use Pynox as the execution tool in your agent loop. Your application chooses
each task's permissions; the agent receives a result it can inspect to decide
its next step. No framework-specific SDK is required.

Start a pool once to keep Python initialized across tool calls:

```bash
pynox pool --workers=2
```

After the `ready` event, send one JSON request per line:

```json
{"id":"tool-42","script":"examples/hello.py","timeout":"2s","memory":"128m","max_output":8192}
```

Result, with timing and policy metadata omitted here:

```json
{"id":"tool-42","exit":0,"stdout":"Hello from Pynox\n","error":null}
```

Use `id` to match the tool call and `exit` to check its outcome. On failure,
`error` carries a type and message, plus source location and a hint when
available. The agent can use those details to revise its code or explain
what went wrong.

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
# Use the archive path printed by the build. Example for Linux glibc on ARM64:
bash scripts/install.sh --archive dist/pynox-0.1.0-cp311-linux-gnu-aarch64.tar.gz
~/.local/bin/pynox --version
```

The installer checks the checksum, matches the OS, libc and architecture,
selects Python from the archive and preserves other installed variants.
Add `~/.local/bin` to your `PATH` to
use the installed `pynox`. GitHub downloads require published release assets;
see [installation options](docs/distribution.md).

## Contributing

Try your scripts, report unexpected behavior and help us measure real workloads.
The [contributor guide](CONTRIBUTING.md) covers setup, checks and releases.

```bash
make test-rust  # Rust unit tests in tests/rust/
make test      # Rust + Python integration tests
```

See the [test guide](tests/README.md) for coverage and Python version selection.

## Documentation

- [Commands and worker pools](docs/usage.md)
- [Sandbox and permissions](docs/sandbox.md)
- [Installation and Python versions](docs/distribution.md)
- [Performance and compatibility](docs/performance.md)
- [Architecture decisions](ADR/README.md)
- [Security scope](SECURITY.md)

## License

[Apache 2.0](LICENSE). Bundles retain CPython's upstream licenses.
