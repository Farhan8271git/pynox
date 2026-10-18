# Pool protocol

`pynox pool` keeps a trusted CPython template warm and forks a fresh, sandboxed
child per request. It speaks JSON Lines on stdin and stdout, so any language
can drive it; `examples/agent_tool.py` is the Python client.

## Starting

```bash
pynox pool --workers=2 [--preload=pandas,numpy] [--package-dir=/path/to/site-packages] [--config=pynox.toml]
```

| Flag | Meaning |
| --- | --- |
| `--workers=N` | Concurrent executions, 1 to 32; each has its own template |
| `--preload=a,b` | Trusted modules imported once in every template, before any job |
| `--package-dir=PATH` | Trusted package directory readable by every job |
| `--config=PATH` | `pynox.toml` with `[pool]`, `[pool.limits]` and `[profiles]`; default `$PYNOX_CONFIG`, then `./pynox.toml` |

The first output line is the `ready` event:

```json
{"event": "ready", "workers": 2, "runtime": "pynox", "preloaded_modules": [], "package_dirs": [],
 "config": "/etc/pynox/prod.toml", "profiles": ["analysis", "fetch"]}
```

Preloads are operator configuration, never a request field. A template that
leaves extra threads or open descriptors after preloading is rejected, and no
job may write over preloaded runtime paths.

## Requests

One JSON object per line. Exactly one of `script` and `code` is required.

```json
{"id": "task-1", "code": "import json\nprint(json.dumps({'ok': True}))", "name": "step.py", "allow_write": ["out"]}
```

```json
{"id": "task-2", "script": "app.py", "args": ["one"], "allow_read": ["data"], "package_dirs": [], "env": {"KEY": "value"}, "allow_network": false, "timeout": "5s", "memory": "512m", "max_output": 65536}
```

| Field | Default | Meaning |
| --- | --- | --- |
| `id` | line number | Copied into the result; assign unique IDs |
| `profile` | | Named permission set from the configuration; the request's own fields add to it |
| `session` | | Run in a persistent session of this name; see below |
| `close` | `false` | With `session`, end the session instead of running code |
| `code` | | Inline program text. No file, no file access, no `__file__` |
| `name` | `<code>` | Label for inline code in errors and `sys.argv[0]` |
| `script` | | Path of a file to run; only that file is implicitly readable |
| `args` | `[]` | `sys.argv[1:]` |
| `allow_read` | `[]` | Existing files or directories to read |
| `allow_write` | `[]` | Existing paths to write beneath; no implicit read |
| `package_dirs` | `[]` | Package directories to import from; no `.pth` execution |
| `env` | `{}` | Environment visible to the task; loader variables are rejected |
| `allow_network` | `false` | IPv4/IPv6, any host; never subprocesses or host Unix sockets |
| `timeout` | `5s` | Wall time from execution start, excluding queue wait; up to 24h |
| `memory` | `512m` | Address-space limit; minimum 32m |
| `max_output` | `65536` | Captured bytes per stream, up to 16 MiB; excess is dropped |

Unknown fields are rejected. A line longer than 1 MiB is discarded with an
error and the next line is read normally.

## Configuration

```toml
[pool]
workers = 4
preload = ["pandas"]
package_dirs = ["/srv/agent/site-packages"]
max_sessions = 32
session_idle = "15m"

[pool.limits]
allow_read = ["/srv/agent/data"]
allow_write = ["/srv/agent/work"]
allow_network = false
timeout = "30s"
memory = "1g"
max_output = 1048576

[profiles.analysis]
allow_read = ["/srv/agent/data"]
allow_write = ["/srv/agent/work"]

[profiles.fetch]
allow_network = true
```

`[pool]` provides defaults that the command line overrides. `[pool.limits]` is
a ceiling: every path a request asks for must lie beneath a listed one, the
network needs `allow_network = true` in the limits, and timeout, memory and
output may not exceed them. A request outside the ceiling fails with
`RequestError` before anything runs. Profiles are presets a request names;
its own fields are added on top, then the ceiling applies. The command-line
`[permissions]` section does not apply to the pool. Relative paths in the file
resolve against its directory, and a file beneath one of its own write grants
is refused. See [ADR 012](../adr/012-configuration.md).

## Results

One JSON object per line, in completion order, not request order:

```json
{"id": "task-1", "exit": 0, "stdout": "{\"ok\": true}\n", "stderr": "", "error": null,
 "stdout_truncated": false, "stderr_truncated": false,
 "duration_ms": 4, "duration_us": 4001, "queue_us": 120,
 "runtime": {"name": "pynox", "python": "3.11.17", "mode": "pool", "preloaded_modules": []},
 "policy": {"allow_read": [], "allow_write": ["/abs/out"], "network": "deny", "subprocesses": "deny",
            "timeout_ms": 5000, "memory_bytes": 536870912, "backend": "landlock+seccomp"}}
```

| Field | Meaning |
| --- | --- |
| `exit` | Python's exit status. 124 timeout, 125 setup or request failure, 128+N signal |
| `error` | `null`, or `{"type", "message", "file", "line", "path", "hint"}` |
| `stdout`, `stderr` | Captured text; `*_truncated` says whether `max_output` cut it |
| `duration_ms` | Execution time; `queue_us` is time spent waiting for a worker |
| `policy` | The effective policy, with canonical paths |

Error `type` is the Python exception class, or one of `RequestError`,
`TimeoutError`, `SignalError`, `SandboxSetupError`, `PoolWorkerError`.
`file` and `line` point at the script or inline `name`; `hint` is present for
permission and memory errors.

## Sessions

A session is one isolated process whose `__main__` survives between requests,
for agents that define a variable in one call and use it in the next.

```json
{"id": 1, "session": "notebook-7", "code": "import pandas as pd\ndf = pd.read_csv('data/orders.csv')", "allow_read": ["data"]}
{"id": 2, "session": "notebook-7", "code": "print(df.groupby('product').amount.sum())"}
{"id": 3, "session": "notebook-7", "close": true}
```

- The first request creates the session and fixes its permissions, limits and
  environment. Later requests carry only `code`, `name`, `args`, and optionally
  `timeout` and `max_output` for that step; any permission field is a
  `RequestError`. Sessions take inline code, not `script`.
- A Python exception leaves the session alive, like a notebook cell. A timeout,
  a crash or an exit ends it; the result then carries `"session_ended": true`
  and the next request under the same name starts a new process, with the
  permissions that request brings.
- Results carry `"session"` and `"step"`. `close` answers `{"closed": true}`.
- Each session owns a template process and its child, started on demand; the
  first step pays the template start (about 30 ms, plus preloaded imports).
- `[pool] max_sessions` (default 16) caps concurrent sessions; a session idle
  for `[pool] session_idle` (default `10m`) is closed. Sessions end with the
  pool. Steps to a session queue behind each other; a session with eight
  pending steps answers `RequestError` until it catches up.

From Python: `Pool.session(**grants)` returns a `Session` with `run` and
`close`, and `pynox.tool(session=True)` gives a model a stateful tool.

## Lifecycle

- Each job is a new process forked from the template after the request is
  parsed. Globals, modules imported by the job, environment and open files never
  reach the next job. Authorized filesystem writes persist.
- Timeouts kill the child; the pool keeps serving. Interrupted jobs are not
  retried, since their side effects may have happened.
- A broken template is replaced before the next request.
- Consume results while submitting; the pool's queue holds twice the worker
  count and stdout backpressure blocks it.
- EOF on stdin drains accepted jobs and exits 0. Inspect each result's `exit`;
  the pool's own exit status only reports protocol completion.

## From other languages

The protocol is plain JSONL over pipes. Spawn `pynox pool`, wait for `ready`,
write a line, read lines until the matching `id` arrives. For a Python client
see `examples/agent_tool.py`; for Monty's external functions calling into the
pool see [Monty](monty.md).
