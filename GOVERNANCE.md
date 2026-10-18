# Governance

Pynox has one maintainer today, [@leandrodamascena](https://github.com/leandrodamascena).
This page says how changes get in, so contributors know what to expect.

## How a change lands

1. **Issue first.** Comment on an issue before you start, and wait for a maintainer to
   reply. Issues marked `needs-design` are discussed before any code.
2. **Pull request.** Open a PR against `main`. CI must pass: the `ci-ok` check covers
   formatting, lint, Rust and Python tests, bundles and installs on every supported Linux
   platform and CPython version.
3. **Review.** Every PR needs one approving review from a code owner, after its last push,
   and every review thread resolved. Squash merge keeps `main` linear.
4. **Decisions that change a contract** (CLI, pool protocol, configuration, sandbox
   policy, distribution) get an ADR in `ADR/` in the same PR.

## Branch and tag rules

- `main` accepts changes only through pull requests with a passing `ci-ok` check and an
  approving code owner review. Force pushes and deletion are blocked.
- Tags matching `v*` can be created only by maintainers and can never be moved or deleted.
  A tag triggers the release workflow, which builds a **draft** release for review.
- Repository administrators can bypass these rules. While there is one maintainer, nobody
  else can approve the maintainer's own pull requests, so the maintainer merges them with
  the bypass, after CI passes. The bypass is also used for release tags and urgent fixes.

## Security

Report vulnerabilities privately, as described in [SECURITY.md](SECURITY.md), never in a
public issue. Secret scanning with push protection, Dependabot security updates, CodeQL
and the OpenSSF Scorecard run on the repository.

## Becoming a maintainer

Regular contributors whose reviews are consistently right will be invited. When there is
more than one maintainer, maintainers will review each other's pull requests instead of
using the bypass, and this page will say how they make decisions together.
