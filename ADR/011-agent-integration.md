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

Ship a Python package, `pynox`, as the integration surface: `pynox.run(code)`
returns a `Result`; `pynox.tool(profile=...)` returns a typed, documented
`run_python(code) -> str` that every agent framework accepts unchanged; `Pool`
gives explicit lifecycle. The package is a thin client of the pool protocol and
contains no agent loop, prompt or framework adapter, so Pynox serves
frameworks instead of competing with them. Its wheel is tagged per CPython
minor and embeds a matching CPython, so the sandbox imports the packages of
the environment that installed it. `examples/agent_tool.py` and
`examples/monty_bridge.py` use it and run in the test suite. Next contracts,
in order: explicit persistent sessions in the pool, a tool bridge from
sandboxed code back to the host, and network allowlists by domain.

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
