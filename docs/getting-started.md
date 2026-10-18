# Getting started

Ten minutes: build Pynox, run a script, watch the sandbox act, write the
permissions down, then send one request to the pool.

## Requirements

- Linux on x86_64 or ARM64, glibc 2.28+ or musl 1.2+.
- Kernel 6.2 or newer for the sandbox. `--no-sandbox` runs anywhere.
- Rust (`rust-toolchain.toml`) and uv >= 0.13.1 to build. `uv` downloads the
  matching CPython.

!!! note "macOS"
    Use a Linux container or VM. Pynox is Linux only for now
    ([ADR 010](adr/010-linux-platforms.md)).

## Build

=== "From a checkout"
    ```bash
    git clone https://github.com/ferrumio/pynox.git
    cd pynox
    make setup && make build
    export PATH="$PWD/target/release:$PATH"
    ```

=== "From a bundle"
    ```bash
    make bundle
    bash scripts/install.sh --archive dist/pynox-0.1.0-cp311-linux-gnu-aarch64.tar.gz
    export PATH="$HOME/.local/bin:$PATH"
    ```
    The bundle carries Pynox and its CPython; the target machine needs neither
    Python nor Rust. See [installation](distribution.md).

Ask the runtime and the kernel what they support:

```bash
pynox doctor
```

```json
{"version": "0.1.0", "python": {"version": "3.11.17"}, "platform": "linux-gnu-aarch64",
 "landlock_abi": 7, "sandbox_supported": true, "sandbox_unavailable_reasons": []}
```

When `sandbox_supported` is false, the reasons name the missing kernel
interface. Sandboxed runs then refuse to start and point you at
`--no-sandbox`; they never run unrestricted on their own.

## Run a script

```python
# hello.py
name = "Pynox"
print(f"Hello from {name}!")
```

```bash
pynox hello.py
# Hello from Pynox!
```

Output and exit code are Python's. The script ran in the sandbox, and it could
read its own directory and the standard library, nothing else.

## Watch the sandbox act

```bash
echo "print(open('/etc/hostname').read())" > probe.py
pynox probe.py
```

```text
Traceback (most recent call last):
  File "/home/you/probe.py", line 1, in <module>
    print(open('/etc/hostname').read())
PermissionError: [Errno 13] Permission denied: '/etc/hostname'
pynox: For files, grant the specific path with --allow-read/--allow-write. ...
```

Grant the path:

```bash
pynox --allow-read=/etc/hostname probe.py
```

Grants are separate for reading and writing. `-c` grants nothing; `-m` makes
the current directory readable, as `sys.path[0]` would:

```bash
pynox --allow-read=data --allow-write=out --timeout=5s job.py
pynox -m pytest -q
```

## Write the permissions down

A project states its permissions once, in a `pynox.toml` where you run:

```toml
[permissions]
allow_read = ["data"]
allow_write = ["out"]
```

```bash
pynox job.py
```

Pynox reads `--config PATH`, then `$PYNOX_CONFIG`, then `./pynox.toml`. It
never reads a file beside the script, so a program cannot grant itself more.
Profiles, pool settings and limits live in the same file; see
[commands](usage.md#pynoxtoml).

## Get a JSON result

```bash
pynox --json hello.py
```

```json
{"exit": 0, "stdout": "Hello from Pynox!\n", "stderr": "", "error": null, "duration_ms": 9,
 "policy": {"allow_read": [], "implicit_read": ["/home/you"], "network": "deny", "...": "..."}}
```

Failures carry an `error` with type, message, file, line and hint. This is
the shape the pool returns for every task.

## Send one request to the pool

The pool keeps a CPython template warm and forks a fresh, sandboxed child per
request. Agents talk to it; so can you, with a line of JSON:

```bash
pynox pool --workers=2
```

```json
{"id": 1, "code": "print(sum(range(10)))"}
```

```json
{"id": 1, "exit": 0, "stdout": "45\n", "stderr": "", "error": null, "duration_ms": 4, "...": "..."}
```

From Python, `examples/agent_tool.py` wraps the protocol:

```python
from agent_tool import PynoxPool

with PynoxPool(workers=2) as pool:
    print(pool.run("print(sum(range(10)))").describe())
```

```bash
PYNOX_BIN=$PWD/target/release/pynox uv run python examples/agent_tool.py
```

## Plain Python

```bash
pynox --no-sandbox -c 'import sys; print(sys.version)'
pynox --no-sandbox -O app.py
```

The unrestricted CPython command line, REPL included, with Pynox's
optimizations installed.

## Next

- [Agents](guides/agents.md): make `pool.run` a tool your model can call.
- [Pool protocol](guides/pool.md): every field, profiles and limits.
- [Monty](guides/monty.md): Monty's glue-code sandbox plus Pynox.
