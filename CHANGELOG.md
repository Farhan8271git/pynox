# Changelog

## 0.1.0 — unreleased

- Introduce Pynox, a Rust runtime embedding CPython 3.12.
- Apply default Linux filesystem, network and process restrictions with bounded
  output, environment grants, wall timeout and address-space limits.
- Support asyncio's internal wakeup socketpair and opt-in IPv4/IPv6.
- Retain a JSONL pool with clean templates and disposable per-task children.
- Integrate an opt-in, bounded `json.dump` fast path without changing scripts.
- Provide explicit unrestricted CPython CLI mode and a capability probe.
- Add relocatable CPython bundles, checksum installation, ADRs and GitHub CI.
- Preserve the earlier research outside the active source tree.
