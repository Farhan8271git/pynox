# ADR 002: Rust integration and versioned CPython ABI

## Status

Accepted — 2026-10-17.

The initial Python version scope below is amended by ADR 009 to 3.11–3.15.

## Context

Python calls must reach native implementations without an RPC/serialization
proxy. ABI stability, semantic compatibility and maintenance are different
questions. Fast access to Python objects can require APIs outside the stable
limited API.

## Decision

Use pyo3-ffi and CPython's C API in the same process. Rust registers native
functions and operates on Python object references. CPython retains object
lifetime, exceptions and execution semantics. Avoid intermediate Rust object
graphs where direct access suffices.

Start with CPython 3.12 with GIL, built separately per Linux architecture.
Use public full C API where needed, without claiming abi3 compatibility.
Keep the adapter small and separate from candidate pure Rust algorithms.
Prefer limited API functions where there is no measured reason otherwise.
Private CPython APIs require a new maintenance justification.

## Alternatives considered

- Stable ABI exclusively: smaller binary matrix, but limits some access paths;
  it still does not guarantee behavior compatibility.
- One binary for arbitrary Pythons: incompatible with the selected interface.
- IPC proxy: adds transfer/serialization and process costs unnecessarily.
- Free-threaded builds immediately: requires a separate object/concurrency audit.

## Consequences

Each new Python minor needs a build and differential tests. Patch updates remain
tested, even when ABI-compatible. Native wheels must match the selected Python;
our adapter does not make third-party extensions universal. GIL release is only
valid around work that does not touch Python objects unsafely. Rust panics,
reference ownership and callbacks require explicit care at the FFI boundary.
