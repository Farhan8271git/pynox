# ADR 005: Explicit pool and disposable task state

## Status

Accepted — 2026-10-17.

## Context

Agents issue many short tasks. Each pays CPython startup and, worse, the
imports of the libraries it uses: hundreds of milliseconds for pandas. Reusing
a process leaks modules, globals, environment, descriptors and native state.
Comparing a warm service to a newly started Python is not a language-speed
comparison, but for agents the warm path is the one that matters.

## Decision

Expose pooling explicitly through `pynox pool` and JSONL. Each worker holds a
single-threaded trusted CPython template in a separate process; each job uses
a new disposable child forked from it. The threaded supervisor is never the
fork source. Deliver job data only after fork. Bound queues, request size,
output and runtime. Do not retry interrupted jobs automatically.

Trusted preloads are operator-only configuration and run before any job.
Reject templates leaving extra threads or descriptors; reject task write
grants overlapping preloaded runtime paths. Per-task permissions, environment
and limits apply only to the child. Measured on 2026-10-17, a small task
through the pool answers in about 4.7 ms against 7.7 ms for a new `python`.

## Alternatives considered

- Reuse user interpreters: stronger state leakage and cleanup obligations.
- Automatically pool every CLI invocation: changes ordinary Python lifecycle.
- Fresh processes only: simpler, but repeatedly pays initialization costs.

## Consequences

Authorized disk changes persist. Preload-time environment and state are fixed
at template creation. Native extension fork safety still matters.

Sessions are the explicit exception to one-process-per-task. A request naming
a `session` gets a dedicated template and child whose `__main__` persists
across steps; permissions are fixed by the first request and later steps carry
only code. Exceptions keep a session alive, timeouts and crashes end it, idle
sessions close after a configurable time and the pool caps their number. A
session trades isolation between steps for state, so it is never implicit and
its results say which session and step they belong to.
