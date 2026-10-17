# ADR 001: An agent-first Python runtime on CPython

## Status

Accepted — 2026-10-17.

## Context

AI agents increasingly run Python they wrote themselves: tool calls, data
analysis, glue between APIs. Today that code runs either with the developer's
full privileges or inside remote containers that take hundreds of milliseconds
and cost money per hour. Agent frameworks need an executor that is fast,
local, isolated per task and compatible with the whole Python ecosystem.

The same runtime must still be an ordinary Python for scripts and services:
agents write normal Python, and the developers who integrate them run it on
their own machines and CI. Earlier drafts of this product leaned on bundled
frameworks and on "a faster Python"; neither is the differentiator.

## Decision

Name the product Pynox. Keep CPython as the language engine and build one
Linux binary with three doors: `pynox script.py` behaves like `python`,
`pynox sandbox` runs one task with explicit permissions and a structured
result, and `pynox pool` serves many such tasks from a warm template. Agents
are the primary workload; scripts, services and data jobs use the same binary.

Preserve ordinary Python source and imports; require no `import pynox`.
Integrate with agent frameworks, including Pydantic's Monty, as their
execution tool rather than as a competing framework. Ship optimizations that
are transparent and measured, and treat the sandbox as the product's proof
that agent code can run locally under control.

## Alternatives considered

- A new agent language or Python dialect: loses the fluency models have in
  Python and the ecosystem. Monty shows the subset approach works for glue
  code; Pynox complements it with full CPython for everything else.
- RustPython or a CPython fork with a JIT: separate compatibility and
  performance burdens before any product proof.
- A prepackaged agent environment with bundled frameworks: convenient, not a
  runtime, and quickly stale.
- A hosted sandbox service: already served by others; Pynox is the local,
  embeddable layer those services and frameworks can also use.

## Consequences

Python loops remain CPython loops. Rust is an implementation tool, not
evidence of speed; claims come from paired measurements of complete
workloads. The first adoption path is framework integration, not users
replacing their interpreter, so the agent-facing protocol and examples are
first-class deliverables. Version 0.1 establishes the runtime, the sandbox,
the pool, one transparent optimization and tested integration examples.
