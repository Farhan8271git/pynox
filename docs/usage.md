# Commands

`pynox` has one default mode and two sandboxed modes. All three run the same
CPython with the same transparent optimizations.

## Default mode: `pynox script.py`

```bash
pynox app.py --flag value
pynox -c 'print("hi")'
pynox -m json.tool data.json
pynox -I -S script.py
```

Everything after `pynox` is CPython's own command line: options, `-c`, `-m`,
the REPL when no arguments are given, normal stdout/stderr and the exit status.
There is no sandbox, capture or limit in this mode; it inherits the environment
like `python` does. `pynox python ...` is accepted as an alias.

Pynox's optimizations are installed before your code runs and activate when
your program imports the module they optimize. Programs that never import
`json` pay nothing for the JSON path. See [performance](performance.md).

Only `sandbox`, `run`, `pool`, `doctor`, `python`, `--version`/`-V` and
`--help`/`-h` are interpreted by Pynox as its first argument.

## Sandbox: `pynox sandbox [OPTIONS] script.py [args...]`

Runs one script with explicit permissions and limits and prints one JSON
result. Flags belong before the script; remaining arguments are Python's
`sys.argv`. Sibling imports need an explicit readable directory. `pynox run`
is an alias.

| Option | Meaning |
| --- | --- |
| `--allow-read=PATH` | Read an existing file/directory; repeatable |
| `--allow-write=PATH` | Write beneath an existing path; no implicit read |
| `--package-dir=PATH` | Import/read installed packages; no `.pth` execution |
| `--allow-env=NAME` | Forward this variable; native-loader variables rejected |
| `--allow-network` | IPv4/IPv6, including listen and outbound connections |
| `--timeout=5s` | Wall timeout, up to 24h |
| `--memory=512m` | Address space, not RSS; minimum 32 MiB |
| `--max-output=65536` | Captured bytes per stream, maximum 16 MiB |

The result includes `exit`, `error`, `stdout`, `stderr`, truncation flags,
duration, runtime and effective policy. Errors carry type, message and script
location when available. Exit 124 means timeout; 125 means launcher/setup
failure. A script can forge its diagnostic channel; do not use it as a
security audit record. Sandboxed tasks run in UTF-8 mode, like stock CPython
under `LANG=C.UTF-8`.

## Pool: `pynox pool --workers=2`

Accepts JSONL on stdin. First output is a `ready` event. A request names a
program with either `script`, a file path, or `code`, inline program text:

```json
{"id":"task-1","script":"app.py","args":["one"],"allow_read":["data"],"allow_write":["out"],"package_dirs":[],"env":{"KEY":"value"},"allow_network":false,"timeout":"5s","memory":"512m","max_output":65536}
{"id":"task-2","code":"import json\nprint(json.dumps({'ok': True}))","name":"step.py","allow_write":["out"]}
```

Inline code is what agents send. It needs no file, grants no file access, has
no `__file__`, and reports errors against `name` (default `<code>`). Exactly
one of `script` and `code` is required. All other fields default to the values
shown in the first example. Unknown fields are rejected. IDs are copied into results; callers must assign
unique IDs and handle out-of-order completion. Requests without ID use their
line number. Maximum line size is 1 MiB; queue capacity is twice worker count.
Consume responses while submitting to avoid pipe backpressure deadlocks.

Workers range from 1 to 32. Timeout starts at execution, excluding queue wait.
EOF drains accepted jobs. Pool exit 0 means protocol completion; inspect each
task's `exit`. Failed templates are replaced; interrupted tasks are not retried.
Each task gets fresh process state. Authorized filesystem writes persist.

Operator flags `--preload=module1,module2 --package-dir=PATH` preload trusted
code before sandboxing. Imports cannot leave extra threads/descriptors. Global
package reads apply to every job, and tasks cannot write over the preloaded
runtime. Modules with unsafe native fork behavior require a separate process
strategy; these startup checks do not certify every extension.

## Agent integrations

[`examples/agent_tool.py`](https://github.com/ferrumio/pynox/blob/main/examples/agent_tool.py)
wraps the pool as a Python function with `allow_read`, `allow_write`,
`allow_network`, `timeout` and `memory` arguments, returning `exit`, `stdout`,
`stderr`, the structured `error` and a `describe()` text for the model.
[`examples/monty_bridge.py`](https://github.com/ferrumio/pynox/blob/main/examples/monty_bridge.py)
runs agent glue code in Monty and delegates heavy work to Pynox through an
external function. Both examples run in the test suite.

## Diagnostics

`pynox doctor` prints build Python, architecture, platform tag, a Landlock
availability probe and separate `runtime_supported` / `sandbox_supported`
capabilities. Actual sandboxed execution still checks all sandbox setup steps. `--version` and
`-V` report both Pynox and the loaded CPython runtime, without initializing Python:

```text
pynox 0.1.0 (CPython 3.11.17)
```
