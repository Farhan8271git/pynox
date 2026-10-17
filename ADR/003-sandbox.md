# ADR 003: Opt-in Linux capability sandbox

## Status

Accepted — 2026-10-17.

## Context

Controlled execution is the product's proof point for agents. Python-level
replacement of `open` cannot enforce policy for native extensions or ctypes.
General frameworks need threads and asyncio, which an indiscriminate socket
ban broke in the original prototype. The goal is controlled automation, not a
claim of complete hostile-code containment. Making the sandbox the default
command made `pynox hello.py` print JSON instead of `Hello` and hid the
runtime behind an agent-only interface.

## Decision

The sandbox is opt-in: `pynox sandbox` and `pynox pool` apply Landlock
ABI >= 3, seccomp and resource limits; `pynox script.py` runs like `python`.
File, environment and IPv4/IPv6 grants are explicit. Unsupported or partial
sandbox setup fails closed and never falls back to unsandboxed execution.

Allow unnamed AF_UNIX stream socketpairs for asyncio wakeups while denying
host Unix sockets. Network stays denied unless granted and never enables
subprocesses. Python and native extensions share the same kernel policy.
Cold execution applies policy before CPython initialization; pool templates
contain only trusted setup before each child's policy. Sandboxed workers run
in UTF-8 mode, matching stock CPython under the launcher's `C.UTF-8`.

## Alternatives considered

- Sandbox by default: safer per command, but it made the runtime unusable as
  a drop-in `python` and hid the optimizations from everyone.
- Monkeypatch file/network APIs: does not constrain native calls.
- Silent fallback on unsupported kernels: violates the policy contract.
- Namespaces, containers or microVMs in v0.1: stronger boundaries with
  separate portability and operational costs; candidates for later layers.

## Consequences

Linux only, kernel 6.2 or newer for ABI 3; older kernels such as current AWS
Lambda guests cannot run the sandbox until an explicitly weaker level is
designed and documented. Sandboxed tasks need explicit sibling imports,
writable paths and network grants. The contract discloses implicit runtime
reads, metadata visibility, persistent writes and limits. Domain allowlists,
subprocess grants and private filesystems remain future work.
