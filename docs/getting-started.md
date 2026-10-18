# Getting started

This guide builds Pynox, runs a script three ways and sends the first request
to a pool. By the end you have a working executor for agent code.

## Requirements

- **Linux** on x86_64 or ARM64. glibc 2.28+ or musl 1.2+.
- **Kernel 6.2 or newer** for the sandbox (Landlock ABI 3). The default Python
  mode runs on any supported kernel.
- **Rust** (see `rust-toolchain.toml`) and **uv >= 0.13.1** to build from a
  checkout. `uv` downloads the matching CPython for you.

!!! note "macOS"
    Pynox is Linux only for now. On macOS, develop inside a Linux container or
    VM. See [ADR 010](adr/010-linux-platforms.md).

## Build

=== "From a checkout"
    ```bash
    git clone https://github.com/ferrumio/pynox.git
    cd pynox
    make setup
    make build
    export PATH="$PWD/target/release:$PATH"
    ```

=== "From a bundle"
    ```bash
    make bundle
    bash scripts/install.sh --archive dist/pynox-0.1.0-cp311-linux-gnu-aarch64.tar.gz
    export PATH="$HOME/.local/bin:$PATH"
    ```
    Bundles include Pynox and its CPython; the target machine needs neither
    Python nor Rust. See [installation](distribution.md).

Check the runtime and the kernel:

```bash
pynox doctor
```

```json
{"runtime": "pynox", "version": "0.1.0", "python": {"version": "3.11.17", "...": "..."},
 "platform": "linux-gnu-aarch64", "landlock_abi": 7,
 "runtime_supported": true, "sandbox_supported": true}
```

`sandbox_supported` is false on kernels without Landlock ABI 3. The runtime
still works; `pynox sandbox` and `pynox pool` refuse to run, they never fall
back to unrestricted execution.

## Run a script

Create `hello.py`:

```python
name = "Pynox"
print(f"Hello from {name}!")
```

Run it like Python. Options, `-c`, `-m`, output and exit codes are CPython's:

```bash
pynox hello.py
# Hello from Pynox!
pynox -c 'import sys; print(sys.version)'
```

## Run it sandboxed

```bash
pynox sandbox hello.py
```

```json
{"exit": 0, "stdout": "Hello from Pynox!\n", "stderr": "", "error": null,
 "duration_ms": 9, "policy": {"allow_read": [], "allow_write": [], "network": "deny", "...": "..."}}
```

Now a script that reads outside its grants:

```bash
echo "print(open('/etc/hostname').read())" > probe.py
pynox sandbox probe.py
```

```json
{"exit": 1, "error": {"type": "PermissionError",
  "message": "[Errno 13] Permission denied: '/etc/hostname'",
  "file": "/home/you/probe.py", "line": 1, "path": "/etc/hostname",
  "hint": "For files, grant the specific path with --allow-read/--allow-write. ..."}}
```

Grants are explicit and separate for reading and writing:

```bash
pynox sandbox --allow-read=/etc/hostname probe.py
pynox sandbox --allow-read=data --allow-write=out --timeout=5s job.py
```

See [commands](usage.md) for every option and the [sandbox contract](sandbox.md)
for what is and is not enforced.

## Start a pool

The pool is how agents use Pynox: a warm CPython template that forks a fresh,
sandboxed child per request.

```bash
pynox pool --workers=2
```

The first line out is a `ready` event. Then send one JSON request per line.
Inline `code` needs no file:

```json
{"id": 1, "code": "print(sum(range(10)))"}
```

```json
{"id": 1, "exit": 0, "stdout": "45\n", "stderr": "", "error": null, "duration_ms": 4, "...": "..."}
```

From Python, use the client in `examples/agent_tool.py`:

```python
from agent_tool import PynoxPool

with PynoxPool(workers=2) as pool:
    print(pool.run("print(sum(range(10)))").describe())
```

```bash
PYNOX_BIN=$PWD/target/release/pynox uv run python examples/agent_tool.py
```

## Next steps

- [Agents](guides/agents.md): turn `pool.run` into a tool your model can call.
- [Pool protocol](guides/pool.md): every request and result field.
- [Monty](guides/monty.md): combine Monty's glue-code sandbox with Pynox.
