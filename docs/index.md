# Pynox

Pynox is a Python runtime for agents and the people who build them. It runs
ordinary Python on CPython with a per-task kernel sandbox, a warm worker pool,
structured results and transparent native optimizations. `pynox script.py`
behaves like `python script.py`.

Version 0.1 is **pre-alpha**, with build targets for CPython 3.11–3.15 (GIL),
defaulting to 3.11, on Linux glibc and musl, x86_64 and ARM64. APIs and
policies can change; this is an evaluation release.

- [Commands](usage.md)
- [Sandbox contract](sandbox.md)
- [Installation and versions](distribution.md)
- [Performance and compatibility](performance.md)
- [Research provenance](research.md)
- [Architecture decisions](adr/README.md)

Build with `make setup && make build`, then run
`./target/release/pynox examples/hello.py`. Try the same script in the sandbox
with `./target/release/pynox sandbox examples/hello.py`, and the agent examples
with `PYNOX_BIN=$PWD/target/release/pynox uv run python examples/agent_tool.py`.
