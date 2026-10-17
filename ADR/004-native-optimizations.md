# ADR 004: Transparent, measured native optimizations

## Status

Accepted — 2026-10-17.

## Context

Bun's speed came from the engine it chose, from native runtime APIs and from
startup work, not from its implementation language. Python already starts in
about 8 ms and most of its standard library is C, so wins are narrower and
must be found operation by operation. The JSON experiment found one: routing
eligible `json.dump` calls through CPython's existing encoder with one batched
write, with memory and streaming tradeoffs. Putting it behind a flag meant
nobody benefited by default.

## Decision

Optimizations are always on and never behind a flag. They install lazily: a
meta path finder patches the target module right after the application
imports it and then removes itself, so programs that never import the module
pay nothing. `json.__loader__` and `json.__spec__` still describe CPython's
loader. Each optimization has an eligibility contract, falls back to
CPython's own algorithm for anything else and is measured against stock
CPython on complete workloads, including memory and concurrency effects.

Startup is a budget. The bootstrap imports nothing beyond what CPython
initializes, embedded Python is precompiled at build time and the supervisor
waits on a pidfd. A change that adds a stdlib import to startup is a
regression. Candidates are chosen from an agent-shaped workload suite:
lifecycle first (templates, sessions), then I/O paths (output batching, event
loop), then interpreter build options (static libpython, allocator).

## Alternatives considered

- A flag per optimization: no reach, and every optimization multiplies flags.
- Eager installation at startup: about 7 ms for programs that never use JSON.
- Replace standard functions with Rust equivalents broadly: most are already
  native and edge cases multiply maintenance.
- Promise every workload is faster: unsupported; pure Python loops stay
  CPython loops.

## Consequences

Eligible `json.dump` output is materialized, bounded by a per-call estimate of
64 MiB, instead of streamed. Introspection, concurrent mutation and partial
writes are not certified equivalent; 158 differential probes provide
evidence, not proof. Each patch is a compatibility contract per Python minor,
so the set stays small. Measured on 2026-10-17: the default mode within about
0.4 ms of CPython on small programs and 1.96× on a 3.9 MiB JSON pipeline.
