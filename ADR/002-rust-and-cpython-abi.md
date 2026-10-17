# ADR 002: Rust integration and versioned CPython ABI

## Status

Accepted — 2026-10-17. Python version scope per ADR 009.

## Context

Python calls must reach native implementations without an RPC/serialization
proxy. ABI stability, semantic compatibility and maintenance are different
questions. Fast access to Python objects can require APIs outside the stable
limited API. Startup matters for agents, so the launcher's own Python must
cost as little as possible.

## Decision

Use pyo3-ffi and CPython's C API in the same process. Rust registers native
functions and operates on Python object references. CPython retains object
lifetime, exceptions and execution semantics. Avoid intermediate Rust object
graphs where direct access suffices.

Build one binary per CPython minor with the GIL. Use the public full C API
where needed, without claiming abi3 compatibility. Private CPython APIs
require a new maintenance justification. Embedded Python helpers live in
`python/pynox/`, are precompiled by `build.rs` with the linked CPython and
loaded with marshal; task data crosses into Python as objects built with the
C API, never by importing `json` in the bootstrap.

## Alternatives considered

- Stable ABI exclusively: smaller binary matrix, but limits access paths and
  still does not guarantee behavior compatibility.
- One binary for arbitrary Pythons: incompatible with the selected interface.
- IPC proxy: adds transfer/serialization and process costs unnecessarily.
- Compile embedded Python at startup: about 1 ms per process, measured.
- Free-threaded builds immediately: requires a separate concurrency audit.

## Consequences

Each new Python minor needs a build and differential tests. Native wheels
must match the selected Python; the adapter does not make extensions
universal. GIL release is only valid around work that does not touch Python
objects. Embedded bytecode is specific to the linked minor version, which
bundles guarantee. Rust panics, reference ownership and callbacks require
explicit care at the FFI boundary.
