# Contributing to Pynox

Use the Rust toolchain in `rust-toolchain.toml`, uv and a Linux host with
Landlock ABI >= 3. Development Python is pinned in `.python-version`.

```bash
make setup
make build
make check
uv run --locked --group integration pytest
make docs
make bundle
```

Rust implements process control, policy, native adapters and optimized paths.
Small Python bootstraps integrate with CPython. Preserve ordinary Python APIs
and document every behavior difference. Keep framework dependencies in optional
test groups; never add them to the shipped runtime.

Tests exercise the actual kernel, native calls and child processes. Do not
replace permission checks with mocks or silently skip unsupported hosts.
Optimization tests need a stock CPython reference, values/errors/side effects,
memory and complete-workload timing. Run comparable lifecycles.

Use conventional commit prefixes (`feat`, `fix`, `perf`, `docs`, `ci`, `chore`).
Add an ADR when changing an architectural contract. ADRs follow the numbered
format used in pydynox. Keep user-facing policy documentation in sync.

## Release

1. Update Cargo.toml, pyproject.toml, installer default and CHANGELOG.md together.
2. Regenerate Cargo.lock / uv.lock if affected and pass `make check`, tests,
   docs and a relocated bundle test. Keep the CPython version pinned.
3. Push the reviewed change to main. A `vX.Y.Z` tag triggers release CI.
4. CI reruns validation, builds native Linux x86_64/aarch64 bundles, checks tag
   against Cargo version and uploads checksum files to a **draft** GitHub release.
5. Review the draft's assets and notes, then publish it.

The workflow derives its repository from GitHub context. Creating/configuring
the GitHub repository, Pages and branch protection is an operator action.
Set required checks for both architectures and quality; permit publishing only
after review. No PyPI package or mandatory service token is needed.

Documentation uses Zensical, as pydynox does. PRs/main validate the site;
manual workflow dispatch with `deploy=true` publishes Pages after Pages is
enabled on the repository. Dependency updates are grouped by Dependabot.
