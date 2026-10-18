# ADR 011: Agent integration contract

## Status

Accepted — 2026-10-17.

## Context

Agent frameworks adopt an executor through a small protocol and a working
example, not by replacing the interpreter. Integrators ask for the same
things: run a string of code without writing files, structured errors a
model can act on, permissions decided by the host, and a warm path for many
short calls. Pydantic's Monty runs a restricted Python subset for glue code
and calls back into the host through external functions; it has no ecosystem,
which is precisely what Pynox provides.

## Decision

Pool requests accept `code` with an optional `name` instead of `script`.
Inline code grants no file access and reports its errors against the given
name. Requests choose permissions and limits per call, or name a `profile`
from the pool's configuration, within the operator's `[pool.limits]`
(ADR 012); the model only supplies code. Results carry `exit`, captured output, a structured error with type,
message, location and hint, duration and the effective policy.

Ship tested, framework-agnostic examples as part of the product:
`examples/agent_tool.py` wraps the pool as a tool any framework can register,
and `examples/monty_bridge.py` shows Monty orchestrating with Pynox executing
the heavy work. Both run in the test suite. Next contracts, in order: a
`pip install pynox` package delivering the binary, explicit persistent
sessions in the pool, a tool bridge from sandboxed code back to the host, and
network allowlists by domain.

## Alternatives considered

- Require a file per task: forces temporary files into every integration.
- Let code requests choose their own grants: the model would set its policy.
- Build a Pynox agent framework: competes with integrators instead of serving
  them.
- A Python subset or new language: Monty already serves that role; Pynox
  complements it with CPython.

## Consequences

The pool protocol is a public contract and changes need a record. Examples
are tests, so a dependency such as pydantic-monty lives in the integration
group and is pinned. Pynox does not snapshot interpreter state mid-execution
as Monty does; persistent sessions approximate it with a living process.
Adoption is measured by integrations, not downloads.
