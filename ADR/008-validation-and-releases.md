# ADR 008: Validation and GitHub release workflow

## Status

Accepted — 2026-10-17.

ADR 009 expands the initial Python matrix to 3.11–3.15 and marks releases pre-alpha.

## Context

The repository accumulated independent experiments and provisional names.
The project needs reviewable decisions, repeatable checks and distribution.
pydynox provides the user's reference for numbered ADRs, Rust/Python validation,
pinned GitHub Actions, Dependabot, Zensical and tag-driven releases.

## Decision

Archive original research with source/result hashes outside the active source
tree. Retain useful behavioral probes and document provenance. Use locked Cargo
and uv dependencies, format/lint checks, Rust unit tests in `tests/rust/`,
real-kernel integration tests,
differential CPython tests and optional framework smoke tests.

CI covers Linux x86_64/aarch64, documentation and relocated release artifacts.
Pin Actions by commit, minimize token permissions and use Dependabot for updates.
Release tags must match the source version and pass CI. Upload bundles and
checksums to a draft release for review. Publish documentation explicitly.

## Alternatives considered

- Rename historical measurements as new results: misleading provenance.
- Delete all experiments: destroys evidence and reproducibility.
- Timing thresholds on shared CI: noisy; use informational benchmark artifacts.
- Copy pydynox's wheel/PyPI workflow unchanged: Pynox distributes an executable
  and CPython, not a Python extension package.

## Consequences

The initial matrix supports one Python series. More targets expand release and
behavior testing obligations. Remote CI execution and publishing happen only
after repository setup/push; local validation must not be described as a green
GitHub run. No application dependencies enter release bundles.
