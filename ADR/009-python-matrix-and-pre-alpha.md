# ADR 009: CPython 3.11–3.15 matrix and pre-alpha maturity

## Status

Accepted — 2026-10-17. Amends the version scope of ADRs 002, 006 and 008.

## Context

Restricting builds to CPython 3.12 excludes existing applications unnecessarily.
Python 3.11 remains in security support until October 2027. The product needs
explicit engine choice across 3.11–3.15, while its own interfaces and sandbox
contract are still young. The earlier term "developer release" was imprecise.

## Decision

Label Pynox 0.1.0 **pre-alpha**, for development/evaluation. Keep CPython 3.11
as the minimum and default, and compile/test GIL builds of 3.11, 3.12, 3.13,
3.14 and 3.15. Store exact release pins in `.python-versions`; `.python-version`
selects the default. Require uv >= 0.13.1 for the current download registry.

Every binary embeds one CPython minor/ABI. Derive runtime paths, executable
names and package names from build metadata. Use `PYNOX_PYTHON_VERSION` for
explicit selection and `PYO3_PYTHON` for custom builds. Distribution accepts
only checked-in pins.

CI derives its Python axis from the manifest and runs the complete runtime
contract, framework smoke tests and relocation/installation for both Linux
architectures. Native dependency tests use a matching Python environment;
JSON comparisons use the exact Python embedded by the tested binary.

Install variants in directories keyed by Pynox version and ABI. Keep local
archive selection independent of whether Python is installed on the user's
machine. GitHub releases remain drafts and are marked as prereleases.

## Alternatives considered

- 3.12 as the minimum: unnecessary exclusion while 3.11 has security support.
- One ABI binary for all Pythons: incompatible with the chosen full C API.
- Declare beta now: implies a degree of interface stability not established.
- Make Python 3.15 a non-blocking CI exception: would hide compatibility failures.

## Consequences

The release matrix grows to ten native variants. Version-specific behavior
and dependency wheels must pass rather than assuming compilation proves
compatibility. Historical 3.12 benchmark results remain tied to their recorded
binary; they do not demonstrate performance on the new matrix.

Free-threaded builds and arbitrary in-process engine switching remain outside
scope. Supporting a CPython version does not extend its upstream support period.
Review the minimum version when upstream reaches end of life.

Reference: [CPython supported versions](https://devguide.python.org/versions/).
