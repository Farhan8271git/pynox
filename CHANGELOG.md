# Changelog

## 0.1.0 — unreleased

- Introduce Pynox as a pre-alpha, agent-first Python runtime: a Rust launcher
  embedding CPython 3.11–3.15, with 3.11 as the default and per-minor pins.
- Make `pynox script.py` (and `-c`, `-m`) the default mode: CPython's command
  line with transparent optimizations and no sandbox.
- Provide opt-in Linux sandboxing through `pynox sandbox` and `pynox pool`,
  with filesystem, network and process restrictions, bounded output,
  environment grants, wall timeout and address-space limits.
- Support asyncio's internal wakeup socketpair and opt-in IPv4/IPv6.
- Retain a JSONL pool with clean templates and disposable per-task children;
  accept inline `code` with an optional `name` instead of a script file.
- Add tested agent integration examples: a framework-agnostic pool tool and a
  Monty bridge where Monty orchestrates and Pynox executes.
- Apply a bounded `json.dump` fast path transparently, installed lazily when the
  application imports `json`; `--optimize-json` and the pool's `optimize_json`
  field no longer exist.
- Keep `pynox python` as an alias of the default mode and `pynox doctor` as the
  capability probe, reporting runtime and sandbox support separately.
- Report Pynox and the loaded CPython runtime together in `--version` / `-V`.
- Keep flags after the script in Python's arguments, preventing them from
  changing runtime permissions or limits.
- Add Rust unit tests under `tests/rust/` and run them alongside Python
  integration tests in `make test` and the CI matrix.
- Separate Rust sources in `src/` from embedded Python in `python/pynox/`,
  following pydynox's layout.
- Add relocatable CPython bundles, checksum installation, ADRs and GitHub CI.
- Build/test five Python versions on two Linux architectures and publish separate
  ABI bundles as draft prereleases; allow installed Python variants to coexist.
- Target Linux only: glibc and musl on x86_64/ARM64, built in pinned
  manylinux/musllinux images with a glibc symbol audit. The macOS build drafted
  earlier was removed before release.
- Cut launcher overhead: precompile embedded Python at build time, pass task
  data to the worker as objects instead of importing `json`, and wait for the
  worker on a pidfd instead of a 5 ms polling sleep.
- Run sandboxed tasks in UTF-8 mode; the isolated configuration previously
  left `open()`, stdout and the filesystem encoding in ASCII.
- Measure the pool's request/response latency in `scripts/benchmark.py`.
- Preserve the earlier research outside the active source tree.
