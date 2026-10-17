# ADR 005: Explicit pool and disposable task state

## Status

Accepted — 2026-10-17.

## Context

Repeated short Python tasks often pay startup/import costs. Reusing a process
can leak modules, globals, environment, descriptors and native state. Comparing
a warm service to a newly started Python is not a language-speed comparison.

## Decision

Expose pooling explicitly through `pynox pool` and JSONL. Each worker holds a
single-threaded trusted CPython template in a separate process; each job uses
a new disposable child. The threaded supervisor is never the fork source.
Deliver job data only after fork. Bound queues, request size, output and runtime.
Do not retry interrupted jobs automatically.

Trusted preloads are operator-only configuration. Reject templates leaving
extra native threads or descriptors; reject overlapping task write grants.
Per-task optimization and network settings apply only to the child.

## Alternatives considered

- Reuse user interpreters: stronger state leakage and cleanup obligations.
- Automatically pool every CLI invocation: changes ordinary Python lifecycle.
- Fresh processes only: simpler, but repeatedly pays initialization costs.

## Consequences

Authorized disk changes persist. Preload-time environment and state are fixed
at template creation. Native extension fork safety still matters. Some workloads
perform better with existing Python pools; compare like-for-like. Persistent
agent sessions and memory sharing require explicit future contracts.
