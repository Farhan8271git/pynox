# Pynox

Pynox combines CPython compatibility, a Rust runtime and explicit Linux
permissions. Ordinary Python scripts run without a project-specific import.

Version 0.1 ships a restricted script runner, JSONL task pool, explicit
unrestricted CPython CLI and an optional JSON optimization.

- [Commands and tasks](usage.md)
- [Sandbox contract](sandbox.md)
- [Installation and versions](distribution.md)
- [Performance and compatibility](performance.md)
- [Research provenance](research.md)
- [Architecture decisions](adr/README.md)

Build with `make setup && make build`. Run
`./target/release/pynox examples/hello.py` for a restricted asyncio example.
