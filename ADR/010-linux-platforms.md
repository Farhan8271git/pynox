# ADR 010: Linux only: glibc and musl

## Status

Accepted — 2026-10-17. Supersedes the native macOS decision drafted earlier
the same day and never released.

## Context

The sandbox relies on Landlock, seccomp, pidfd and other Linux interfaces,
and it is the product's proof point for agents. A macOS build could only
offer the plain Python mode, which gives a developer's first contact with
Pynox no sandbox to see, and it doubled the release matrix and the CI
surface for a pre-alpha maintained by a small team.

## Decision

Build and release for Linux only: glibc and musl on x86_64 and aarch64, four
platforms for each pinned CPython from 3.11 through 3.15, twenty variants.
The crate refuses to compile elsewhere. Use pinned PyPA manylinux_2_28 and
musllinux_1_2 images, audit glibc symbol requirements across the whole
bundle against 2.28 and link musl dynamically for CPython and extensions.

Kernel requirements are independent of the libc baseline: the sandbox needs
Landlock ABI >= 3; the default mode runs on any supported userspace.

## Alternatives considered

- Native macOS with Python mode only: doubled the matrix for a runtime without
  its differentiator; deferred, not rejected.
- A Linux VM for macOS: operational scope beyond the current team.
- Ubuntu binaries labeled manylinux without an audit: an unsupported promise.
- Reuse glibc binaries on musl: incompatible native library interfaces.

## Consequences

Developers on macOS use Linux containers or remote machines for now. Every
code path compiles unconditionally, with no platform guards to maintain.
macOS returns when a sandbox backend exists for it or when demand justifies a
Python-only build, as a new record.
