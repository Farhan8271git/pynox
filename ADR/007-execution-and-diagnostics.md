# ADR 007: Execution modes and structured diagnostics

## Status

Accepted — 2026-10-17.

## Context

Ordinary Python tooling expects stdout/stderr, CLI switches and exit codes.
Agents need bounded machine-readable results with error locations and the
effective policy. Mixing the two invisibly is confusing, and wrapping every
Python invocation in JSON made the runtime unusable as `python`.

## Decision

`pynox [PYTHON_OPTIONS] SCRIPT [ARGS]`, `-c` and `-m` are CPython's own
command line with Pynox's optimizations installed; stdout, stderr and the
exit status are Python's. `pynox python ...` is an alias. Anything that is
not a Pynox command is passed to CPython.

`pynox sandbox` and `pynox pool` return one JSON result per task with
captured streams, exit, duration, policy and a structured error carrying type,
message, script location and actionable permission or limit hints. Pool
responses add request IDs. `pynox doctor` reports runtime metadata, a
Landlock probe and `sandbox_supported` separately from `runtime_supported`.

## Alternatives considered

- JSON on every invocation: breaks conventional CLI integration.
- Tracebacks only: poor automation diagnostics for a model to act on.
- Treat task-generated diagnostic JSON as a security audit: untrustworthy,
  since the task can write to its own report channel.

## Consequences

Default execution has no sandbox, as with `python`. The sandbox launch
interface is an explicit execution contract, not byte-for-byte CPython CLI.
Partial output before a `RecursionError` differs by the runner's frame.
Passthrough stdout for `pynox sandbox` is a possible later addition.
