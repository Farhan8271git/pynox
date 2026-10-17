# ADR 010: Linux libc variants and native macOS

## Status

Accepted — 2026-10-17. Extends ADRs 002, 003, 006, 008 and 009.

## Context

The runtime needs Linux glibc, Linux musl and macOS on both x86_64 and ARM64.
Manylinux and musllinux describe Python packaging compatibility policies,
not additional operating systems. The existing sandbox relies on Landlock,
seccomp and Linux process interfaces.

The user explicitly accepted native Python mode first on macOS, with sandbox
and pool unavailable until a separate backend is validated.

## Decision

Build six platform variants for each pinned CPython from 3.11 through 3.15:
Linux glibc and musl on x86_64/aarch64, plus macOS Intel and Apple Silicon.
Keep separate native CI jobs and artifacts for all 30 combinations.

Use pinned PyPA manylinux_2_28 and musllinux_1_2 container images for Linux.
Audit glibc symbol requirements across the complete bundle against 2.28.
Use dynamic musl linking for CPython and native extensions. Kernel sandbox
requirements remain independent of the libc baseline.

Isolate Linux supervision, workers, pool and kernel policy behind platform
compilation guards. Share the CPython adapter and JSON optimization. On macOS,
provide native Python mode, runtime metadata and version reporting.
Restricted commands fail explicitly with exit 125 and `SandboxUnavailable`;
never execute them as ordinary Python implicitly.

Target macOS 13+ for releases. Rewrite Mach-O library references and apply
ad-hoc signatures after modifying executables. Developer ID signing and
notarization require a later distribution decision.

Include OS, libc and architecture in archive and installation directory names.
Reject incompatible local archives before extracting them. Derive runtime
paths from the embedded Python version. Verify relocation on every platform.

Rust test bodies stay in `tests/rust/`. Run portable tests on every target,
Linux kernel tests on Linux, and explicit sandbox-refusal tests on macOS.
Keep ADR sources versioned in the repository and included in every bundle.

## Alternatives considered

- Label Ubuntu binaries manylinux without an ABI audit: gives an unsupported
  compatibility promise.
- Reuse glibc binaries on musl: incompatible native library interfaces.
- Silently disable the sandbox on macOS: violates the execution contract.
- Present a Rust compile check as native runtime validation: insufficient for
  Python extensions, relocation and loader behavior.
- Bundle a Linux VM for macOS immediately: adds installation and operational
  scope beyond the agreed native runtime first.

## Consequences

The release matrix grows from ten to thirty variants. macOS initially has
fewer execution features, shown in documentation and doctor output. Native
package compatibility remains specific to Python, OS, architecture and libc.
Linux isolation behavior remains enforced by its existing kernel backend.
