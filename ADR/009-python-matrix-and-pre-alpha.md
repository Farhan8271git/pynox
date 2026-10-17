# ADR 009: CPython 3.11–3.15 matrix and pre-alpha maturity

## Status

Accepted — 2026-10-17.

## Context

Restricting builds to one CPython minor excludes existing applications
unnecessarily. Python 3.11 remains in security support until October 2027.
The product needs explicit engine choice across 3.11–3.15 while its own
interfaces and sandbox contract are still young.

## Decision

Label Pynox 0.1.0 **pre-alpha**, for development and evaluation. Keep
CPython 3.11 as the minimum and default, and compile/test GIL builds of 3.11
through 3.15. Store exact pins in `.python-versions`; `.python-version`
selects the default. Require uv >= 0.13.1 for the current download registry.

Every binary embeds one CPython minor/ABI. Derive runtime paths and package
names from build metadata. Use `PYNOX_PYTHON_VERSION` for explicit selection
and `PYO3_PYTHON` for custom builds. CI derives its Python axis from the
manifest and runs the complete contract for every version. Install variants
in directories keyed by Pynox version, ABI and platform.

## Alternatives considered

- 3.12 as the minimum: unnecessary exclusion while 3.11 is supported.
- One ABI binary for all Pythons: incompatible with the full C API choice.
- Declare beta now: implies interface stability not yet established.
- Non-blocking CI for 3.15: would hide compatibility failures.

## Consequences

Version-specific behavior and dependency wheels must pass rather than
assuming compilation proves compatibility. Benchmark results stay tied to the
recorded binary and Python. Free-threaded builds remain out of scope. Review
the minimum when upstream reaches end of life.

Reference: [CPython supported versions](https://devguide.python.org/versions/).
