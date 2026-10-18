# Monty

[Monty](https://github.com/pydantic/monty) is Pydantic's sandbox interpreter for
agent code: a restricted Python subset, written in Rust, with no filesystem or
network, snapshots of the whole execution state, and external functions the
host provides. Pynox runs full CPython. They solve different halves of the
same problem.

## Key features

- Monty runs the model's glue code: decisions, loops, tool calls, pause and resume
- Pynox runs the heavy code: pandas, numpy, any library, real CPython semantics
- The host owns the bridge and fixes permissions per call
- The complete example below runs in Pynox's test suite

## Why both

| | Monty | Pynox |
| --- | --- | --- |
| Engine | Own interpreter, Python subset | CPython 3.11–3.15 |
| Libraries | 24 reimplemented stdlib modules | Everything, including C extensions |
| Isolation | No OS surface at all | Kernel policy: Landlock, seccomp, limits |
| Startup | Under 1 ms from its pool | About 5 ms from the Pynox pool |
| Snapshots | Dump and resume mid-execution | No; fresh process per task |
| Platforms | Python, JavaScript, Rust bindings | Linux binary, JSONL protocol |

Glue code rarely needs libraries; data work always does. Running glue in Monty
keeps it tiny, deterministic and resumable. Handing the heavy part to Pynox
keeps the ecosystem and the semantics of production Python.

## The bridge

Monty calls functions you register with `external_lookup`. Register one that
runs code on a Pynox pool:

```python
def run_python(code: str, timeout: str = "10s") -> dict:
    result = pool.run(code, name="heavy.py", allow_read=[work_dir], allow_write=[work_dir], timeout=timeout)
    return {"exit": result.exit, "stdout": result.stdout, "error": result.error}
```

The model's glue code, inside Monty, uses it like any function. Permissions are
not part of the signature, so the model cannot widen them:

```python
import json

job = run_python('''
import json, statistics
values = [3, 9, 27]
summary = {"mean": statistics.mean(values), "count": len(values)}
with open("/work/stats.json", "w") as f:
    json.dump(summary, f)
print(json.dumps(summary))
''')
if job["exit"] != 0:
    error = job["error"]
    raise RuntimeError(f"{error['type']} at line {error['line']}: {error['message']}")
json.loads(job["stdout"])["mean"]
```

The heavy code writes a file. The glue code could not have: Monty has no
filesystem. Pynox allowed exactly one directory, chosen by the host.

## Complete example

Install the optional dependency and run the example against a local build:

```bash
uv sync --group integration        # includes pydantic-monty
PYNOX_BIN=$PWD/target/release/pynox uv run python examples/monty_bridge.py
# mean: 13
# stored: {"mean": 13, "count": 3}
```

??? example "examples/monty_bridge.py"
    ```python
    --8<-- "examples/monty_bridge.py"
    ```

## What each layer guarantees

| Layer | Runs | Guarantees |
| --- | --- | --- |
| Monty | The agent's logic | No access to the host. Can pause at `run_python` and be snapshotted while Pynox works |
| Pynox pool | The heavy code | New process per job, forked from a warm template. Reads and writes only where granted; no subprocesses; no network unless granted |
| Host | The bridge | Decides permissions per call. The model asks "run this"; the host says where |

## Without Monty

If you only need the model's code to call your functions, Pynox can do that by
itself: `pynox.tool(functions={...})` exposes them inside the sandbox as
`pynox_host`. Monty adds snapshots and a much smaller interpreter for the glue;
choose it when pausing and resuming the agent matters.

## Limits

- Pynox cannot snapshot a running CPython. If the heavy code must survive a
  host restart, split it into idempotent steps and let Monty hold the state.
- `install_dependencies` in Monty 1.1 installs packages into Monty's own
  worker; libraries that need inheritance, generators or C extensions still
  belong on the Pynox side.
- Both sandboxes are controlled automation, not hostile-code containment.
  Read Monty's security model and Pynox's [sandbox contract](../sandbox.md).

## Next steps

- [Agents](agents.md): the tool shape without Monty
- [Pool protocol](pool.md): what `run_python` sends under the hood
