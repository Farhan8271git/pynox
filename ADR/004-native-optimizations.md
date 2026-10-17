# ADR 004: Measured native optimizations with explicit contracts

## Status

Accepted — 2026-10-17.

## Context

Bun-inspired performance can come from the engine, fewer allocations/copies,
native APIs, batching and eliminating repeated work. Rust alone does not make
an existing C implementation faster. The JSON experiment found a concrete
benefit by routing selected `json.dump` calls through an existing CPython
encoder, with memory and streaming tradeoffs.

## Decision

Integrate the bounded JSON path behind `--optimize-json`, off by default.
Maintain normal imports/calls and conservative fallback. Keep `open`, `json.load`
and arbitrary framework internals unchanged. Register the native module through
CPython and install the integration once before user code, not by intercepting
every language operation.

Every optimization requires an eligibility contract, direct CPython comparison,
side-effect/error checks and complete-workload measurements. Report allocations,
memory and concurrency effects, not just an isolated best-case multiplier.
Prefer a small set of broad, maintainable improvements.

## Alternatives considered

- Replace all standard functions with Rust equivalents: many are already native
  and edge cases would multiply maintenance.
- Make the JSON path unconditional: changes streaming/memory behavior.
- Require `pynox.read_json` in user code: useful as an optional API, but does not
  satisfy transparent acceleration.
- Promise every workload is faster: unsupported by the experiments.

## Consequences

JSON output may be materialized rather than streamed. Limits are per call,
not aggregate memory budgets. Introspection, concurrent mutation and all
partial-write cases are not certified equivalent. The retained 158 probes
provide evidence, not universal proof. Future candidates include upstream
CPython builds/execution, imports, buffers and event loops; each must beat a
strong existing-Python baseline before becoming a product claim.
