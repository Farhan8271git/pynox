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

A warm CPython template that forks a fresh, sandboxed child per JSONL request.
Requests carry inline `code` or a `script`, plus the same permissions and
limits as `pynox sandbox`. The full request and result reference is in the
[pool protocol](guides/pool.md).

## Agent integrations

See [Agents](guides/agents.md) for the tool shape and framework registration,
and [Monty](guides/monty.md) for the two-level sandbox.

## Diagnostics

`pynox doctor` prints build Python, architecture, platform tag, a Landlock
availability probe and separate `runtime_supported` / `sandbox_supported`
capabilities. Actual sandboxed execution still checks all sandbox setup steps. `--version` and
`-V` report both Pynox and the loaded CPython runtime, without initializing Python:

```text
pynox 0.1.0 (CPython 3.11.17)
```
