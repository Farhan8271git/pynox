# Pynox 🐍⚙️

![Status: pre-alpha](https://img.shields.io/badge/status-pre--alpha-orange)
[![CI](https://github.com/ferrumio/pynox/actions/workflows/ci.yml/badge.svg)](https://github.com/ferrumio/pynox/actions/workflows/ci.yml)
[![Documentation](https://img.shields.io/badge/docs-ferrumio.github.io%2Fpynox-indigo)](https://ferrumio.github.io/pynox/)
[![CPython 3.11–3.15](https://img.shields.io/badge/CPython-3.11%E2%80%933.15-blue)](#python-versions)
![Linux x86_64 and ARM64](https://img.shields.io/badge/platform-Linux%20x86__64%20%7C%20ARM64-lightgrey)
[![License: Apache 2.0](https://img.shields.io/badge/license-Apache%202.0-green)](LICENSE)

**The Python runtime for agents. Run the code they write, fast and under control.**

Pynox runs ordinary Python on **CPython** with a per-task kernel sandbox,
a warm worker pool for many short calls, structured results a model can act
on, and native optimizations applied transparently where they measurably
help. It is also just a Python: `pynox app.py` works like `python app.py`.
No framework, no `import pynox`.

> 🚧 **Pre-alpha (0.1.0).** Ready to try, experiment and contribute.
> Interfaces and sandbox policies are still evolving; this is not a production release.

| Platform | Architectures | Modes |
| --- | --- | --- |
| Linux glibc 2.28+ | x86_64, ARM64 | `pynox`, `pynox sandbox`, `pynox pool` |
| Linux musl 1.2+ / Alpine | x86_64, ARM64 | `pynox`, `pynox sandbox`, `pynox pool` |

Pynox is **Linux only** for now; the sandbox is built on Landlock and seccomp.
On macOS, use a Linux container or VM. See [platforms](docs/distribution.md).

## Why Pynox?

- **Built for agent loops** — Send code, get back `exit`, output, a structured error with type, line and a hint, in a few milliseconds. The host chooses each task's permissions; the model only writes Python.
- **Kernel sandbox, no containers** — Landlock and seccomp per task: which files, environment variables and network a job gets, plus time, memory and output limits. Fails closed.
- **Warm pool** — A trusted CPython template forks a clean child per task. Small tasks answer faster than a new `python` process starts; preloaded libraries cost nothing per call.
- **Real Python** — The whole ecosystem, every CPython option, unchanged code. Pairs with [Monty](https://github.com/pydantic/monty) for glue code that needs snapshots.
- **Faster where it counts** — Native paths for common operations, always on, each measured against stock CPython and falling back to CPython's behavior when a call is not eligible.

## Quick start

For now, build from a checkout. You'll need **Linux on x86_64 or ARM64**, Rust
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

Run it:

```bash
pynox hello.py
# Hello from Pynox!
```

`-c`, `-m` and every other CPython option work the same way:

```bash
pynox -c 'import sys; print(sys.version)'
pynox -m json.tool data.json
```

## What about speed?

Pynox optimizes specific operations, transparently, and reports the results
honestly. Today it ships one optimization: eligible `json.dump` calls use
CPython's C encoder with one batched write. It activates only when your program
imports `json`, so other programs pay nothing.

Measured on this project's Linux ARM64 host with CPython 3.11, end-to-end p50:

| Workload | CPython | `pynox` | `pynox pool` |
| --- | ---: | ---: | ---: |
| Tiny script | 7.68 ms | 8.09 ms | 4.68 ms |
| JSON pipeline, 3.89 MiB | 153.80 ms | 78.50 ms | 75.75 ms |

Python loops are still CPython loops; Pynox does not make the interpreter
itself faster. The optimized path uses extra memory for large outputs. See
[measurements, limits and the startup budget](docs/performance.md).

## Sandbox a script

On Linux, `pynox sandbox` runs a script with only the access you grant and
returns one JSON result. This included example uses ordinary `open`,
`json.load` and `json.dump`:

```bash
mkdir -p examples/out
pynox sandbox \
  --allow-read=examples/data \
  --allow-write=examples/out \
  --timeout=5s \
  examples/json_open.py examples/data/orders.json examples/out/orders.json
```

The result has `stdout`, the exit status, timing and the permissions used.
When a script fails, it includes the error type, message and script location
when available, plus a hint for permission and limit errors. Create output
directories first; read and write access are separate grants. Sibling imports
and third-party packages also need access; see the [command reference](docs/usage.md).

For a script that calls an API, allow network access and pass its API key:

```bash
pynox sandbox --allow-network --allow-env=API_KEY app.py
```

Network access is off by default. The flag enables IPv4/IPv6; host Unix sockets
and subprocesses stay blocked. The sandbox requires **Landlock ABI 3 or newer**
and refuses to run if the kernel cannot enforce it. It never falls back to
unrestricted execution. See the [sandbox guide](docs/sandbox.md).

## Give an agent a Python tool

Use Pynox as the execution tool in your agent loop. Your application chooses
each task's permissions; the model only supplies code and receives a result it
can inspect to decide its next step. No framework-specific SDK is required.

A tool is a function the model can call. [`examples/agent_tool.py`](examples/agent_tool.py)
gives you the pool client; the host fixes the permissions, the model only
writes code, and the result comes back as text the model can read:

```python
from agent_tool import PynoxPool

pool = PynoxPool(workers=2)

def run_python(code: str) -> str:
    """Run Python written by the model. It can write to ./out and nothing else."""
    result = pool.run(code, allow_write=["out"], timeout="10s", memory="256m")
    return result.describe()
```

```python
>>> print(run_python("print(sum(range(10)))"))
45
>>> print(run_python("open('/etc/passwd').read()"))
PermissionError: [Errno 13] Permission denied: '/etc/passwd'
at agent.py:1
For files, grant the specific path with --allow-read/--allow-write. ...
```

Register `run_python` with your framework's tool decorator and you are done.
Under the hood this is `pynox pool --workers=2` speaking JSONL: one request per
line with `code`, permissions and limits; one result per line with `id`,
`exit`, `stdout`, `error` and timing. Each job runs in a fresh child forked
from a warm template, so a small task answers in about 5 ms. Python globals
are not shared between jobs; authorized file writes persist. See the
[agents guide](docs/guides/agents.md) and the [pool protocol](docs/guides/pool.md).

### With Monty

[Monty](https://github.com/pydantic/monty) runs a restricted Python subset for
agent glue code, with snapshots and no filesystem or network. When that code
needs real libraries, hand the work to Pynox through an external function.
[`examples/monty_bridge.py`](examples/monty_bridge.py) is a complete, tested
example: Monty orchestrates, Pynox executes, and the host decides what the
heavy code may touch.

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

Rust source lives in [`src/`](src/), and embedded Python helpers live in
[`python/pynox/`](python/pynox/), following pydynox's layout.

```bash
make test-rust  # Rust unit tests in tests/rust/
make test      # Rust + Python integration tests
```

See the [test guide](tests/README.md) for coverage and Python version selection.

## Documentation

Full documentation lives at **[ferrumio.github.io/pynox](https://ferrumio.github.io/pynox/)**.

- [Getting started](docs/getting-started.md)
- [Agents: tools, pool protocol and Monty](docs/guides/agents.md)
- [Commands](docs/usage.md)
- [Sandbox and permissions](docs/sandbox.md)
- [Installation and Python versions](docs/distribution.md)
- [Performance and compatibility](docs/performance.md)
- [Architecture decisions](ADR/README.md)
- [Security scope](SECURITY.md)

## License

[Apache 2.0](LICENSE). Bundles retain CPython's upstream licenses.
