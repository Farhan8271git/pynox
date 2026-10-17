# Performance and compatibility

The objective is faster existing Python workloads, for applications and agents
alike. CPython executes Python bytecode. Rust owns process control, the sandbox
and selected native paths; changing implementation language alone is not a
speedup. Every optimization ships on, applies transparently and must beat the
same program on stock CPython in a paired measurement.

## Transparent optimizations

Optimizations install before user code and activate when the application
imports the module they optimize. A meta path finder patches the module right
after CPython executes it, then removes itself. Programs that never import
that module pay nothing. `json.__loader__` and `json.__spec__` still describe
CPython's own loader.

**`json.dump`.** Exact builtin objects, default options and ordinary buffered
UTF-8/ASCII regular files use CPython's existing C encoder and one batched
write. Other calls keep the original streaming algorithm. `open` and
`json.load` are unchanged.

The optimization holds the GIL while touching Python objects. Objects pass
directly through the C API; there is no IPC or conversion into serde JSON.
The guard bounds depth (64), visited nodes (1 million) and estimated output
(64 MiB). This is a per-call estimate, not a process or concurrency memory cap.

Materializing output uses additional memory and changes write granularity.
Introspection, traceback identity, concurrent mutations, signal timing and
partial writes on storage failures are not universally equivalent. Memory and
recursion errors before writing fall back to streaming; fallback does not undo
already committed effects. Differential tests cover 158 observations in stock
and embedded Python and repeat them under the sandbox. These checks do not
prove equivalence for all programs.

## Startup budget

Startup is measured and defended, because a launcher that costs more than an
optimization gains makes the optimization invisible:

- Embedded Python is precompiled in `build.rs` by the linked CPython and loaded
  with marshal. Compiling source at startup was the interpreter's first use of
  the compiler and cost about 1 ms per process.
- The sandbox bootstrap imports only `os` and `sys`. Task data crosses into
  Python as objects built with the C API, and the report crosses back the same
  way, so no task pays for `import json` (about 5 ms through `re` and `enum`).
- The supervisor waits for the worker on a pidfd. A 5 ms polling sleep used to
  add 2.5 ms on average to every sandboxed run.

Known remaining costs: the shared libpython adds about 0.3 ms at startup and
about 3% to interpreter-bound loops compared with the statically linked stock
executable; the supervisor process loads libpython it never uses; Landlock and
seccomp setup takes about 1.3 ms; finalization about 2 ms.

## Measurement policy

Compare the same source, data, Python version/build and lifecycle. Include an
embedded control and a well-tuned stock alternative. Report process startup
separately from warm operations; record output correctness, memory,
concurrency, p50/p95 and complete-workload latency. Filesystem cache and
durability assumptions must be explicit.

No historical result is relabeled as a Pynox 0.1 measurement. See
[research](research.md) for the earlier JSON, pool and framework findings.
`scripts/benchmark.py` produces fresh, paired measurements for this version;
performance results are informational, not noisy shared-runner merge gates.

Next experiments: static libpython and upstream build options, a supervisor
without libpython, import and bytecode caching, buffer/serialization overhead
and asyncio integration. Use frameworks as workloads; do not replace their
internals without evidence. Model/network waiting time in agents must be
separated from local Python work.

## Pynox 0.1.0: local Linux aarch64 measurement

Measured on 2026-10-17 with CPython 3.11.17, two CPU cores, seven shuffled
rounds per combination and one discarded warmup. All 140 measured executions
validated their outputs. Each number is end-to-end p50 in milliseconds. The
pool column is the request/response latency seen by a client of
`pynox pool --workers=1`, whose template was started once before measuring.

| Workload | CPython | `pynox` | `pynox sandbox` | `pynox pool` |
| --- | ---: | ---: | ---: | ---: |
| Tiny script | 7.68 | 8.09 | 10.87 | 4.68 |
| Python CPU | 16.95 | 17.71 | 20.53 | 14.07 |
| Async tasks | 39.88 | 40.85 | 43.55 | 36.68 |
| JSON, 32 records | 13.40 | 13.78 | 16.74 | 10.68 |
| JSON, 32,768 records | 153.80 | 78.50 | 82.94 | 75.75 |

The large JSON input is 3.89 MiB. The pipeline includes reading, Python
transformation, encoding and writing: 1.96× faster than stock CPython in the
default mode and 1.85× in the sandbox. Small programs stay within about 0.4 ms
of CPython in the default mode. The sandbox adds about 3 ms per task for a new
supervised process, kernel policy and the JSON result. The pool, which keeps a
trusted CPython template warm and forks a fresh child per task, answers small
tasks faster than a new `python` process starts. None of this demonstrates a
faster interpreter: Python loops remain CPython loops.

The filesystem is warm and writes do not use fsync. Memory was not measured
in this run. Seven samples make nearest-rank p95 equal to the maximum; raw
observations, source/binary hashes, build flags and all p95 values are in the
[measurement artifact](benchmarks/v0.1.0-linux-aarch64.json). Results are from
this host, not a GitHub CI run or a guarantee for other applications.
