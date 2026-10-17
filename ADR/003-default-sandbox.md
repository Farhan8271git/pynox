# ADR 003: Default Linux capability sandbox

## Status

Accepted — 2026-10-17.

## Context

Controlled execution and isolation are product differentiators. Python-level
replacement of `open` cannot enforce policy for native extensions. General
frameworks need threads and asyncio, which an indiscriminate socket ban broke
in the original prototype. The initial goal is controlled automation, not a
claim of complete hostile-code containment.

## Decision

Default script execution and pool jobs use Landlock ABI >= 3, seccomp and
resource limits. File, environment and IPv4/IPv6 grants are explicit. Unsupported
or partial sandbox setup fails closed. Keep an explicit unrestricted
`pynox python` mode for CPython CLI compatibility.

Allow unnamed AF_UNIX stream socketpairs for internal asyncio wakeups, while denying
host Unix sockets. Network access remains denied unless granted, and does not
enable subprocesses. Python and native extensions share the same kernel policy.
Cold execution applies policy before CPython initialization; pool templates
contain only trusted setup before each child's policy.

## Alternatives considered

- Monkeypatch file/network APIs: does not constrain native calls.
- Silent fallback on unsupported platforms: violates the policy contract.
- Namespaces, containers or microVMs in v0.1: stronger potential boundaries with
  separate portability, deployment and maintenance costs.
- Require sandbox opt-in everywhere: weaker default for controlled automation.

## Consequences

Linux x86_64/aarch64 only initially. Restricted mode needs explicit sibling
imports, writable paths and network permissions; it cannot run every unchanged
application under default grants. The contract discloses implicit runtime reads,
metadata visibility, persistent writes and limits. Domain policies, subprocess
grants, private filesystems and cgroup accounting remain future work.
