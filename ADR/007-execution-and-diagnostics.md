# ADR 007: Structured diagnostics and explicit Python mode

## Status

Accepted — 2026-10-17.

## Context

Automation benefits from bounded machine-readable results, useful error
locations and explicit effective policy. Ordinary Python tooling expects
stdout/stderr, CLI switches and exit codes. Mixing the two invisibly is confusing.

## Decision

Restricted script execution returns one JSON result with captured streams,
exit, duration, policy and concise structured errors. Preserve script location
where possible and include actionable permission/limit hints. Pool responses
use the same result shape plus request IDs.

`pynox python` runs CPython's normal CLI without sandbox, capture or limits.
Documentation and help state this distinction directly. `pynox doctor` reports
runtime metadata and a capability probe; execution performs complete setup.

## Alternatives considered

- Force JSON wrapping on every Python tool: breaks conventional CLI integration.
- Hide sandbox errors or print only tracebacks: poor automation diagnostics.
- Treat task-generated diagnostic JSON as a security audit: untrustworthy.

## Consequences

Scripts retain Python syntax but the restricted launch interface is an explicit
execution contract, not byte-for-byte replacement of the CPython CLI.
Normal mode retains ordinary tracebacks. Structured error reporting does not
change the application's exception classes or language semantics.
