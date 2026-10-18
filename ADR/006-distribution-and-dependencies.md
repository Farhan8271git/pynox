# ADR 006: Pinned, relocatable Linux distribution

## Status

Accepted — 2026-10-17. Version pins per ADR 009; platforms per ADR 010.

## Context

End users and agent frameworks should not need to install Python or compile
Rust. Pynox must control the engine version while allowing ordinary
application packages. Frameworks integrate most easily when the runtime is
one installable artifact.

## Decision

Release Pynox with its matching CPython and standard distribution, no
application frameworks. Use uv to obtain the pinned build runtime. Record
versions and hashes, keep upstream licenses, use relative loader paths and
test relocation and actual sandbox setup before archiving.

Install versioned directories and switch the CLI symlink after verification.
Keep Pynox and Python versions distinct. Dependency resolution stays with
existing tools; sandboxed jobs receive explicit installed package
directories. The bundle must always work on its own, copied into a container
image, a read-only filesystem or a Lambda package, without a launcher,
network or writable state.

Precompile the bundled standard library with unchecked-hash bytecode, so
read-only installs and sandboxed runs never recompile it and archive time
normalization cannot invalidate it.

Derive every other artifact from the bundle. A wheel per CPython minor and
platform carries the `pynox` package, the binary and the matching CPython,
tagged `cp3XY-cp3XY-manylinux_2_28` or `musllinux_1_2`, with a console script.
A Lambda layer per CPython minor and architecture, glibc only, carries a
custom runtime whose loop speaks the Runtime API over a socket, because
importing `urllib` cost about 250 ms of cold start. Container images per
CPython minor and libc copy the bundle onto a pinned Debian or Alpine base
with no build step, so both architectures build without emulation. CI builds
wheels and layers next to the bundles and tests them; the release workflow
uploads them to the draft release and pushes the images to GHCR.

Planned, not shipped: PyPI publication after the first release; a launcher
that installs and selects bundles per Python version in the style of
`uv python install`, with downloads manual by default and pinned to the Pynox
release.

## Alternatives considered

- Require arbitrary system Python: inconsistent ABI and user setup.
- Bundle popular frameworks: large, coupled dependency chain.
- Write a package resolver immediately: duplicates uv before runtime proof.
- Copy a binary with absolute libpython paths: not a distributable release.

## Consequences

Artifacts are larger than the Rust executable alone. CPython and dependency
updates need rebuilds and tests. Extensions need matching wheels. The bundle
links libpython dynamically; the stock interpreter links it statically, which
costs Pynox about 0.3 ms at startup and about 3% in interpreter-bound loops.
Static linking is a measured future change to this record.
