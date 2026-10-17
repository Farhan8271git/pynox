# Performance and compatibility

The objective is faster existing Python workloads, for general applications and
agents. CPython executes Python bytecode. Rust owns integration and selected
native paths; changing implementation language alone is not a speedup.

## First optimization

`--optimize-json` activates an experimental `json.dump` path. Exact builtin
objects, default options and ordinary buffered UTF-8/ASCII regular files can
use CPython's existing C encoder and one batched write. Other supported cases
keep the original streaming algorithm. `open` and `json.load` are unchanged.

The optimization holds the GIL while touching Python objects. Objects pass
directly through the C API; there is no IPC or conversion into serde JSON.
The guard bounds depth (64), visited nodes (1 million) and estimated output
(64 MiB). This is a per-call estimate, not a process or concurrency memory cap.

Materializing output uses additional memory and changes write granularity.
Introspection, traceback identity, concurrent mutations, signal timing and
partial writes on storage failures are not universally equivalent. The option
is therefore off by default. Memory/recursion errors before writing may fall
back to streaming. Fallback does not undo already committed effects.

Differential tests cover 158 observations in stock and embedded Python and
repeat the comparison under the sandbox. Separate tests check policy, async
behavior and task state. These checks do not prove equivalence for all programs.

## Measurement policy

Compare the same source, data, Python version/build and lifecycle. Include an
embedded control with optimization disabled and a well-tuned stock alternative.
Report process startup separately from warm operations; record output
correctness, memory, concurrency, p50/p95 and complete-workload latency.
Filesystem cache and durability assumptions must be explicit.

No historical result is relabeled as a Pynox 0.1 measurement. See
[research](research.md) for the earlier JSON, pool and framework findings.
`scripts/benchmark.py` produces fresh, paired measurements for this version;
performance results are informational, not noisy shared-runner merge gates.

Next experiments should examine upstream CPython execution/build options,
startup/import costs, buffer/serialization overhead and asyncio integration.
Use frameworks as workloads; do not replace their internals without evidence.
Pydantic and other dependencies already contain native implementations.
Model/network waiting time in agents must be separated from local Python work.

## Pynox 0.1.0: local Linux aarch64 measurement

Measured on 2026-10-17 with CPython 3.12.14, two CPU cores, seven shuffled
rounds per combination and one discarded warmup. All 175 measured executions
validated their outputs. Each number is end-to-end p50 in milliseconds.

| Workload | CPython | Pynox Python | Python + JSON | Sandbox | Sandbox + JSON |
| --- | ---: | ---: | ---: | ---: | ---: |
| Tiny script | 8.92 | 9.16 | 16.62 | 23.50 | 23.55 |
| Python CPU | 19.32 | 20.59 | 28.03 | 33.68 | 33.71 |
| Async tasks | 47.76 | 47.94 | 48.92 | 54.14 | 54.15 |
| JSON, 32 records | 14.99 | 15.40 | 16.74 | 23.57 | 23.60 |
| JSON, 32,768 records | 152.73 | 158.13 | 74.21 | 165.14 | 79.20 |

The large JSON input is 3.89 MiB. The pipeline includes reading, Python
transformation, encoding and writing. With sandbox and JSON optimization it is
1.93× faster than stock CPython in this sample. Small scripts regress; the
normal Python mode stays close to CPython and does not demonstrate a faster
interpreter. Pool startup amortization is not included in these results.

The filesystem is warm and writes do not use fsync. Memory was not measured
in this run. Seven samples make nearest-rank p95 equal to the maximum; raw
observations, source/binary hashes, build flags and all p95 values are in the
[measurement artifact](benchmarks/v0.1.0-linux-aarch64.json). Results are from
this host, not a GitHub CI run or a guarantee for other applications.
