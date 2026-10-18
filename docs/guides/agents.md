# Agents

Give a model a Python tool. This guide covers the shape of the tool, who
decides permissions, what the model gets back and how to register it with an
agent framework.

## Key features

- One function, `run_python(code) -> str`, that any framework can register
- Permissions and limits fixed by the host, per call; the model only writes code
- Results as text the model can act on: output, or error type, line and hint
- About 5 ms per call from a warm pool; fresh process state every time

## The tool

Start one pool for the life of your application and expose one function:

```python
from agent_tool import PynoxPool

pool = PynoxPool(workers=2, preload=["json"])

def run_python(code: str) -> str:
    """Run Python written by the model. It may write to ./out and nothing else."""
    result = pool.run(code, allow_write=["out"], timeout="10s", memory="256m")
    return result.describe()
```

The signature the model sees is just `code`. Paths, network, time and memory
are arguments of `pool.run` that your code supplies. If the model wants more
access, it has to ask you, not the runtime.

`PynoxPool` is a small client over the [pool protocol](pool.md), included in
the repository and exercised by the test suite:

??? example "examples/agent_tool.py"
    ```python
    --8<-- "examples/agent_tool.py"
    ```

## What the model gets back

`describe()` turns the structured result into text:

| Situation | Text returned |
| --- | --- |
| Success | stdout, or `(no output)` |
| Python exception | `TypeName: message`, then `at name.py:line`, then a hint when one applies, then stderr |
| Timeout | `TimeoutError: Wall time exceeded 10000ms` |
| Killed by a limit | `exit status 137` and stderr |

The structured form is there when you want to branch on it instead:

```python
result = pool.run(code)
result.ok              # exit == 0 and no error
result.exit            # Python exit status; 124 timeout, 125 setup failure
result.error           # None or {"type", "message", "file", "line", "path", "hint"}
result.stdout, result.stderr, result.truncated, result.duration_ms
```

Error locations use the request's `name` (default `agent.py`), so a model that
numbers its lines can map them back. Hints are written for models: a
`PermissionError` says which flag would grant the path; a `MemoryError` says to
reduce allocations or raise `memory`.

## Permissions model

| Capability | Default | How to grant |
| --- | --- | --- |
| Read a file or directory | denied, except the standard library | `allow_read=[path]` |
| Write beneath a path | denied | `allow_write=[path]`; does not imply read |
| Import installed packages | denied | `package_dirs=[site_packages]` or pool `--preload` |
| Environment variables | none | `env={"KEY": value}` |
| Network | denied | `allow_network=True` (IPv4/IPv6, any host) |
| Subprocesses, host Unix sockets | denied | not grantable |
| Wall time, address space, output | 10 s, 512 MiB, 64 KiB | `timeout`, `memory`, `max_output` |

Enforcement is in the kernel (Landlock and seccomp) and covers native code,
ctypes included. Read the [sandbox contract](../sandbox.md) before relying on
it: it is controlled automation, not a boundary for hostile code.

## Registering with a framework

Any framework that can call a Python function can call `run_python`. The
snippets below show the shape; only `examples/agent_tool.py` and
`examples/monty_bridge.py` run in Pynox's test suite.

=== "Plain function"
    ```python
    tools = {"run_python": run_python}
    # In your loop: when the model requests run_python, call it with the
    # code argument and append the returned text as the tool result.
    ```

=== "Pydantic AI"
    ```python
    from pydantic_ai import Agent

    agent = Agent("anthropic:claude-sonnet-4-5")

    @agent.tool_plain
    def run_python(code: str) -> str:
        """Run Python code. Only ./out is writable."""
        return pool.run(code, allow_write=["out"], timeout="10s").describe()
    ```

=== "smolagents"
    ```python
    from smolagents import tool

    @tool
    def run_python(code: str) -> str:
        """Run Python code in a sandbox.

        Args:
            code: Python source to execute. Only ./out is writable.
        """
        return pool.run(code, allow_write=["out"], timeout="10s").describe()
    ```

For agents whose glue code should itself run isolated, pair Pynox with
[Monty](monty.md).

## Patterns

**One pool per permission profile.** A pool is cheap. Run one for "read the
dataset, write the report" and another for "network allowed, no files" rather
than deciding per call from model input.

**Preload what every task imports.** `PynoxPool(preload=["pandas"])` pays the
import once in the template; each forked child starts with it loaded.

**Keep outputs small.** `max_output` bounds what is captured; the model gets
the first bytes and a `truncated` flag. Ask the model to print summaries.

**Treat the result as data, not as an audit log.** The task can write to its
own report channel. Use the kernel policy for guarantees, the report for
diagnostics.

## Latency

Measured on this project's Linux ARM64 host, p50, request to result:

| Task | `pynox pool` | New `python` process |
| --- | ---: | ---: |
| Tiny script | 4.68 ms | 7.68 ms |
| 200k-iteration loop | 14.07 ms | 16.95 ms |
| JSON pipeline, 3.89 MiB | 75.75 ms | 153.80 ms |

See [performance](../performance.md) for the method and the full table.

## Next steps

- [Pool protocol](pool.md): every field of a request and a result
- [Monty](monty.md): Monty orchestrates, Pynox executes
- [Sandbox contract](../sandbox.md): what the kernel enforces and what it does not
