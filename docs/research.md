# Research carried into Pynox

The original experiments were archived outside the source tree at
`../pynox-research-20261017`, including source, raw measurements, reports and
`ARCHIVE-MANIFEST.json`. No historical benchmark has been relabeled as a result
for Pynox 0.1.0. Restore the original layout to reproduce those scripts.

The JSON experiment measured 122 ms → 29 ms for encoding/writing a 4 MiB
input, and 187 ms → 86 ms for a new process running the complete example.
Tiny inputs regressed by about 2 ms; peak memory increased by about 4.7 MiB
in the large pipeline. It reused CPython’s encoder, held filesystem caches
warm and did not call fsync. These are workload-specific historical results.

158 differential observations covered bytes, values, errors and file behavior.
Those probes are retained in tests/fixtures/json_contract.py. They do not
prove universal equivalence, especially concurrent mutation and partial writes.

Other experiments established that lifecycle/preload wins are not interpreter
speedups; framework initialization matters; optimized Python baselines must
be compared; isolation and warm execution need independent measurements.
