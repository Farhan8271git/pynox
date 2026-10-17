# Commands and tasks

`pynox [OPTIONS] script.py [args...]` and `pynox run ...` run one restricted
script. Flags belong before the script; remaining arguments are Python's
`sys.argv`. Sibling imports need an explicit readable directory.

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
| `--optimize-json` | Experimental bounded JSON optimization |

Restricted stdout is one JSON object. It includes `exit`, `error`, `stdout`,
`stderr`, truncation flags, duration, runtime and effective policy. Errors carry
type, message and script location when available. Exit 124 means timeout;
125 means launcher/setup failure. A script can forge its diagnostic channel;
do not use it as a security audit record.

`pynox python [--optimize-json] [CPYTHON_ARGS...]` runs **without sandbox** and
preserves normal stdout/stderr, script / `-m` / `-c` behavior and exit status.
The optional optimization flag must immediately follow `python`. Use `--` to
end Pynox's prefix parsing. This mode inherits the normal environment.

`pynox doctor` prints build Python, architecture and a Landlock availability
probe. Actual execution still checks all sandbox setup steps. `--version` and
`-V` report both Pynox and the loaded CPython runtime, without initializing Python:

```text
pynox 0.1.0 (CPython 3.12.14)
```

## Pool

`pynox pool --workers=2` accepts JSONL on stdin. First output is a `ready`
event. Request example:

```json
{"id":"task-1","script":"app.py","args":["one"],"allow_read":["data"],"allow_write":["out"],"package_dirs":[],"env":{"KEY":"value"},"allow_network":false,"optimize_json":false,"timeout":"5s","memory":"512m","max_output":65536}
```

Unknown fields are rejected. IDs are copied into results; callers must assign
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
