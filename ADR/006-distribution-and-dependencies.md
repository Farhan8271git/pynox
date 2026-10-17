# ADR 006: Pinned, relocatable Python distribution

## Status

Accepted — 2026-10-17.

The initial version pins and single-minor scope below are amended by ADR 009.

## Context

End users should not need to install Python or compile Rust. Pynox must control
the engine version while allowing ordinary application packages. Earlier
bundles included many frameworks, which was not the desired product.

## Decision

Release Pynox with matching CPython and its standard distribution, no mandatory
application frameworks. Pin CPython 3.12.14 for the initial release and use uv
to obtain the build runtime. Record versions and hashes, keep upstream licenses,
use relative loader/runtime paths and test relocation.

Install versioned directories and switch the CLI symlink after verification.
Keep Pynox and Python versions distinct. Initial binaries target Linux glibc
x86_64/aarch64. Dependency resolution/lockfiles remain with existing tools;
restricted jobs receive explicit installed package directories.

## Alternatives considered

- Require arbitrary system Python: inconsistent ABI and user setup.
- Bundle popular frameworks: large, coupled dependency chain.
- Write a package resolver immediately: duplicates work before runtime proof.
- Copy a binary with absolute libpython paths: not a distributable release.

## Consequences

Artifacts are larger than the Rust executable alone. CPython and dependency
updates need rebuilds and tests. Existing extensions need matching wheels.
Version 0.1 installs one supported Python series; automatic multi-version
selection is future work. Checksums verify transfers, not an independent trust
root. The bundle is not a static binary and still depends on system facilities.
