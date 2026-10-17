# ADR 008: Validation and GitHub release workflow

## Status

Accepted — 2026-10-17. Python matrix per ADR 009; platforms per ADR 010.

## Context

The repository accumulated independent experiments and provisional names.
The project needs reviewable decisions, repeatable checks and distribution.
pydynox provides the reference for numbered ADRs, Rust/Python validation,
pinned GitHub Actions, Dependabot, Zensical and tag-driven releases.

## Decision

Archive original research with hashes outside the active source tree. Use
locked Cargo and uv dependencies, format/lint checks, Rust unit tests in
`tests/rust/`, real-kernel integration tests, differential CPython tests,
framework smoke tests and the agent integration examples, including Monty,
as tests.

CI covers Linux glibc and musl on x86_64/aarch64 in pinned PyPA containers,
documentation and relocated release artifacts. Pin Actions by commit,
minimize token permissions and use Dependabot. Release tags must match the
source version and pass CI. Upload bundles and checksums to a draft
prerelease for review. Benchmarks are informational artifacts, not gates.

## Alternatives considered

- Relabel historical measurements as new results: misleading provenance.
- Timing thresholds on shared CI: noisy; use informational artifacts.
- Copy pydynox's wheel/PyPI workflow unchanged: Pynox distributes an
  executable and CPython, not a Python extension package.

## Consequences

Twenty native variants per release. Remote CI execution and publishing happen
only after repository setup; local validation must not be described as a
green GitHub run. No application dependencies enter release bundles.
