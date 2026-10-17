# Changelog

## 0.1.0 — unreleased

- Introduce Pynox as a pre-alpha Rust runtime embedding CPython 3.11–3.15,
  with 3.11 as the default and explicit per-minor release pins.
- Apply default Linux filesystem, network and process restrictions with bounded
  output, environment grants, wall timeout and address-space limits.
- Support asyncio's internal wakeup socketpair and opt-in IPv4/IPv6.
- Retain a JSONL pool with clean templates and disposable per-task children.
- Integrate an opt-in, bounded `json.dump` fast path without changing scripts.
- Provide explicit unrestricted CPython CLI mode and a capability probe.
- Report Pynox and the loaded CPython runtime together in `--version` / `-V`.
- Keep flags after the script in Python's arguments, preventing them from
  changing runtime permissions or limits.
- Add Rust unit tests under `tests/rust/` and run them alongside Python
  integration tests in `make test` and the CI matrix.
- Add relocatable CPython bundles, checksum installation, ADRs and GitHub CI.
- Build/test five Python versions on two Linux architectures and publish separate
  ABI bundles as draft prereleases; allow installed Python variants to coexist.
- Preserve the earlier research outside the active source tree.
