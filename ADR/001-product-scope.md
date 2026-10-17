# ADR 001: General-purpose Python runtime on CPython

## Status

Accepted — 2026-10-17.

## Context

The product should deliver the integrated developer experience that motivated
the comparison with Bun, while running existing Python and dependencies.
Agents are an important workload, alongside CLIs, web services, data processing
and ordinary business applications. Earlier experiments focused too heavily
on agent pools and bundled frameworks.

## Decision

Name the product Pynox. Keep CPython as the language engine. Implement a Rust
launcher, explicit permissions and selected native optimizations. Preserve
normal Python source and imports; require no `import pynox` for core benefits.
Treat fast installation, version control, concise diagnostics and sandboxing
as product capabilities, and measure execution speed independently.

## Alternatives considered

- A new language or application-specific API: requires source changes and
  reduces ecosystem compatibility.
- RustPython: a separate interpreter, with its own compatibility and performance
  responsibilities. It conflicts with retaining CPython for this product.
- A CPython fork with a new JIT: too much maintenance before finding broad wins.
- A prepackaged agent/data environment: convenient but not a faster runtime.

## Consequences

Python loops remain CPython loops unless the underlying engine is improved.
Rust is an implementation tool, not evidence of performance. Benchmark the
same program, packages and lifecycle; framework bundling and warm workers must
not be marketed as interpreter acceleration. Version 0.1 establishes the
runtime/sandbox foundation and one optional optimization, not universal speedup.
