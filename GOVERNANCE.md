# Governance

Pynox has one maintainer today, [@leandrodamascena](https://github.com/leandrodamascena).
This page says how changes get in, so contributors know what to expect.

## How a change lands

1. **Issue first.** Comment on an issue before you start, and wait for a maintainer to
   reply. Issues marked `needs-design` are discussed before any code.
2. **Pull request.** Open a PR against `main`. CI must pass: the `ci-ok` check covers
   formatting, lint, Rust and Python tests, bundles and installs on every supported Linux
   platform and CPython version.
3. **Review.** A maintainer reviews every PR, as `CODEOWNERS` requires. Squash merge keeps
   `main` linear.
4. **Decisions that change a contract** (CLI, pool protocol, configuration, sandbox
   policy, distribution) get an ADR in `ADR/` in the same PR.

## Branch and tag rules

- `main` accepts changes only through pull requests with a passing `ci-ok` check. Force
  pushes and deletion are blocked.
- Tags matching `v*` can be created only by maintainers and can never be moved or deleted.
  A tag triggers the release workflow, which builds a **draft** release for review.
- Repository administrators can bypass these rules. While there is one maintainer, the
  maintainer uses the bypass for release tags and for urgent fixes, and says so in the
  commit message.

## Security

Report vulnerabilities privately, as described in [SECURITY.md](SECURITY.md), never in a
public issue. Secret scanning with push protection, Dependabot security updates, CodeQL
and the OpenSSF Scorecard run on the repository.

## Becoming a maintainer

Regular contributors whose reviews are consistently right will be invited. When there is
more than one maintainer, `main` will require an approving review from someone other than
the author, and this page will say how maintainers make decisions together.
