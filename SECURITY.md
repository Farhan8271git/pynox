# Security scope

Pynox's sandbox modes, `pynox sandbox` and `pynox pool`, enforce explicit
capabilities using Linux Landlock and seccomp. The intended use is controlling Python automation and resource
usage. It is not a VM or a complete boundary against hostile native code.

Filesystem contents, metadata visibility, network grants, inherited runtime
reads and limits are described in [the sandbox contract](docs/sandbox.md).
Policy applies to Python and native extensions through kernel mediation.
Unsupported or partially enforced Landlock configuration aborts execution.

The default `pynox script.py` mode runs without a sandbox. Trusted pool preload runs before
sandboxing; only operator-controlled modules belong there. No task may request
preload, and writable task grants may not overlap preloaded runtime paths.
Native extensions remain subject to their own fork/thread safety requirements.
The backend supports Linux glibc and musl; Pynox does not build elsewhere.
Runtime availability in `doctor` does not imply sandbox availability.

The JSON diagnostic pipe is writable by the program and is not a trustworthy
audit log against malicious code. Parent-enforced timeout and kernel rules do
not depend on trusting that report.

For a vulnerability in a published repository, use GitHub private vulnerability
reporting when enabled. Do not put secrets or working exploits in public issues.
Version 0.1 is pre-alpha and is not intended for production use; no external
security audit is claimed. CPython 3.11–3.15 GIL builds are tested separately;
free-threaded builds are rejected.
