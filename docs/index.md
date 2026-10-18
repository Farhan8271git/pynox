# Pynox

The Python runtime for agents. Run the code they write, fast and under control.

!!! warning "Pre-alpha (0.1.0)"
    Pynox is ready to try, experiment with and contribute to. Interfaces and
    sandbox policies are still evolving; this is not a production release.
    Linux only, x86_64 and ARM64.

## The problem

Agents write Python: tool calls, data analysis, glue between APIs, tests for
the code they just changed. Today that code runs in one of two places:

- **On your machine, with your privileges.** `subprocess.run(["python", ...])`,
  `exec()`, a REPL tool. Zero latency, zero protection. One bad `rm -rf` or one
  `open(".env")` away from trouble.
- **In a remote container or microVM.** Strong isolation, 150 ms to 2 s to
  start, a bill per hour, and your data leaves the machine.

There was nothing in between: isolation enforced by the kernel, local, in
milliseconds, with the whole Python ecosystem available.

## The solution

Pynox is that layer. Send code, get back a result the model can act on:

```python
from agent_tool import PynoxPool

pool = PynoxPool(workers=2)

def run_python(code: str) -> str:
    """Tool exposed to the model. The host fixes permissions; the model only writes code."""
    return pool.run(code, allow_write=["out"], timeout="10s", memory="256m").describe()
```

```text
>>> run_python("print(sum(range(10)))")
45
>>> run_python("open('/etc/passwd').read()")
PermissionError: [Errno 13] Permission denied: '/etc/passwd'
at agent.py:1
For files, grant the specific path with --allow-read/--allow-write. ...
```

Each call runs in a fresh process forked from a warm CPython template, under
Landlock and seccomp, with the files, network, time and memory **you** chose.
A small task answers in about 5 ms. And it is still just Python:
`pynox app.py` behaves like `python app.py`, options, output and exit code
included.

## Why CPython, and why Rust

Pynox keeps **CPython** as the engine. The model writes Python because it has
seen more Python than any other language; the code it writes will run in
production on CPython; and the ecosystem, from pandas to pydantic, only exists
there. Pynox does not reimplement the language.

**Rust** owns everything around the interpreter: the launcher, process
supervision, the kernel sandbox, the pool and selected native paths. It is an
implementation tool, not a performance claim. Pure Python loops run exactly as
fast as in CPython.

What Pynox does make faster is measured, operation by operation, against the
same program on stock CPython:

| Workload | CPython | `pynox` | `pynox pool` |
| --- | ---: | ---: | ---: |
| Tiny script | 7.68 ms | 8.09 ms | 4.68 ms |
| JSON pipeline, 3.89 MiB | 153.80 ms | 78.50 ms | 75.75 ms |

End-to-end p50 on this project's Linux ARM64 host with CPython 3.11; see
[performance](performance.md) for method, limits and the full table.

## Pynox and Monty

[Monty](https://github.com/pydantic/monty), by Pydantic, runs a restricted
Python subset for agent glue code with snapshots and no OS access. Pynox runs
the real thing. They fit together: Monty orchestrates, Pynox executes the heavy
work, and the host decides what that work may touch. See
[Monty](guides/monty.md) for a complete, tested example.

## Key features

**Structured results** - `exit`, output, and an error with type, message, line
and a hint the model can act on, instead of a raw traceback.

**Kernel sandbox, no containers** - Landlock and seccomp per task. Which files,
environment variables and network a job gets; time, memory and output limits.
Fails closed; never silently runs unrestricted.

**Warm pool** - A trusted CPython template forks a clean child per task.
Preloaded libraries cost nothing per call; globals never leak between jobs.

**Inline code** - Pool requests carry `code` directly. No temporary files, no
file access granted by accident.

**Transparent optimizations** - Native paths for common operations, always on,
applied lazily when the program imports the module they optimize.

**Drop-in Python** - `pynox script.py`, `-c`, `-m` and every CPython option.
One binary bundles its own CPython, 3.11 through 3.15.

## Guides

### Agents

| Guide | Description |
| --- | --- |
| [Agents](guides/agents.md) | Give a model a Python tool: permissions, results, errors |
| [Pool protocol](guides/pool.md) | JSONL requests and results, limits, preloading |
| [Monty](guides/monty.md) | Monty orchestrates, Pynox executes |

### Runtime

| Guide | Description |
| --- | --- |
| [Getting started](getting-started.md) | Build, run a script, run it sandboxed |
| [Commands](usage.md) | `pynox`, `pynox sandbox`, `pynox pool`, `pynox doctor` |
| [Sandbox contract](sandbox.md) | What is granted, what is denied, what remains |
| [Performance](performance.md) | Optimizations, startup budget, measurements |

### Project

| Guide | Description |
| --- | --- |
| [Installation and versions](distribution.md) | Bundles, platforms, Python matrix |
| [Research provenance](research.md) | Where the earlier experiments live |
| [Architecture decisions](adr/README.md) | The eleven records behind the design |
