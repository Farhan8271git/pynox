# Architecture Decision Records

Pynox uses the numbered ADR format used by pydynox. Records capture context,
decision, alternatives and consequences. Accepted records describe v0.1 unless
explicitly marked as future work; an ambition is not an implemented feature.

| # | Decision | Status |
| --- | --- | --- |
| 001 | [General-purpose Python runtime on CPython](001-product-scope.md) | Accepted |
| 002 | [Rust integration and versioned CPython ABI](002-rust-and-cpython-abi.md) | Accepted |
| 003 | [Default Linux capability sandbox](003-default-sandbox.md) | Accepted |
| 004 | [Measured native optimizations with explicit contracts](004-native-optimizations.md) | Accepted |
| 005 | [Explicit pool and disposable task state](005-task-lifecycle.md) | Accepted |
| 006 | [Pinned, relocatable Python distribution](006-distribution-and-dependencies.md) | Accepted |
| 007 | [Structured diagnostics and explicit Python mode](007-execution-and-diagnostics.md) | Accepted |
| 008 | [Validation and GitHub release workflow](008-validation-and-releases.md) | Accepted |
| 009 | [CPython 3.11–3.15 and pre-alpha maturity](009-python-matrix-and-pre-alpha.md) | Accepted; amends 002, 006, 008 |

New records use `NNN-short-title.md` with Status, Context, Decision,
Alternatives considered and Consequences. Supersede decisions explicitly.
All initial records were written on 2026-10-17 from the product discussion and
preserved experiments.
