# Pynox

Pynox combines CPython compatibility, a Rust runtime and explicit Linux
permissions. Ordinary Python scripts run without a project-specific import.

Version 0.1 is **pre-alpha**, with build targets for CPython 3.11–3.15 (GIL),
defaulting to 3.11. Linux glibc and musl builds include a restricted script
runner and JSONL task pool. Linux and macOS both provide an explicit unrestricted
CPython CLI and an optional JSON optimization, on ARM64 and x86_64. APIs and
policies can change; this is an evaluation release.

- [Commands and tasks](usage.md)
- [Sandbox contract](sandbox.md)
- [Installation and versions](distribution.md)
- [Performance and compatibility](performance.md)
- [Research provenance](research.md)
- [Architecture decisions](adr/README.md)

Build with `make setup && make build`. On Linux, run
`./target/release/pynox examples/hello.py` for a restricted asyncio example.
On macOS, use `./target/release/pynox python examples/hello.py` for native execution.
