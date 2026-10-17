# Working on Pynox

Read README.md and ADR/README.md first. Pynox is a general-purpose CPython
runtime with a Rust launcher, explicit sandbox capabilities and measured
native optimizations. Agents are one workload, not the entire product.

- Preserve ordinary Python source/imports. Do not introduce mandatory frameworks.
- Kernel policy must fail closed; Python monkeypatching is not a sandbox.
- Treat runtime ABI and Python behavior as separate compatibility contracts.
- Keep native object/reference access safe under the supported GIL model.
- Optimize measured costs; compare equal lifecycles and record regressions/memory.
- Pool reuse is explicit. Never reuse task globals or fork the threaded supervisor.
- Keep Cargo.lock, uv.lock, version metadata, docs and ADRs consistent.
- Keep Rust tests in `tests/rust/`; implementation modules only reference them
  with `#[cfg(test)]` and `#[path]`, without inline test bodies.
- Use `make build`, `make check`, `uv run --locked --group integration pytest`,
  `make docs`, `make bundle`, and `uv run --locked python scripts/check_install.py`
  as appropriate to the change.
- Do not relabel historical research as results for a changed binary.
- CI files follow pydynox's pinned-action and draft-release conventions, adapted
  for executable bundles. Distinguish local validation from remote CI results.
