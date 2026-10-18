# ADR 003: Sandbox by default, with explicit permissions

## Status

Accepted — 2026-10-17. Revised 2026-10-18: the sandbox became the default
again once its output was decoupled from the JSON result (ADR 007).

## Context

Controlled execution is the product's proof point for agents. Python-level
replacement of `open` cannot enforce policy for native extensions or ctypes.
The first design made the sandbox the only way to run a file and printed a
JSON result, so `pynox hello.py` did not print `Hello`; the second made plain
Python the default and hid the sandbox behind a subcommand. Both were the
wrong trade: the output format was coupled to isolation. Deno showed that a
runtime can be secure by default and still feel like the ordinary tool when
permissions are explicit and the terminal behaves normally.

## Decision

`pynox script.py`, `pynox -c CODE` and `pynox -m MODULE` run inside the
sandbox with Python's own stdout, stderr, stdin and exit status. Permissions
are deny by default and granted explicitly by flags or by `pynox.toml`
(ADR 012). One implicit grant exists: the script's directory is readable, so
sibling imports work; `-m` makes the current directory readable instead,
mirroring `sys.path[0]`. `-c` grants nothing. `--no-sandbox` is the explicit
way to run CPython's unrestricted command line, including interpreter options
the sandbox does not support.

The backend is Landlock ABI >= 3, seccomp and resource limits; unsupported
or partial setup fails closed with a message pointing at `--no-sandbox`, and
never falls back silently. Allow unnamed AF_UNIX socketpairs for asyncio while
denying host Unix sockets; network stays denied unless granted and never
enables subprocesses. Python and native extensions share the kernel policy.
Cold execution applies policy before CPython initialization; pool templates
hold only trusted setup. Workers run in UTF-8 mode like stock CPython under
`C.UTF-8`.

## Alternatives considered

- Plain Python by default with an opt-in subcommand: compatible, but the
  product's differentiator was invisible and every agent integration had to
  remember to ask for it.
- Deny the script's directory too: stricter, and `import utils` fails on the
  first run; the directory of the file you chose to execute is the smallest
  grant that keeps ordinary scripts working.
- Interactive permission prompts: good terminal experience; deferred until
  the non-interactive contract is stable.
- Namespaces, containers or microVMs: stronger boundaries with separate
  operational costs; candidates for later layers, not replacements.

## Consequences

The default costs a supervised worker: about 2.5 ms over `python` on a tiny
script, measured, and documented rather than hidden. Scripts that read or
write outside their directory need grants, and `pynox.toml` exists so a
project states them once. The default mode needs Linux 6.2 or newer and an
environment that lets an unprivileged process call `prctl`, `seccomp` and
`pidfd_open`; `pynox doctor` probes these and lists the reasons when the
sandbox cannot run. Measured on 2026-10-18, AWS Lambda offers neither (kernel
5.10, `prctl` denied), so there and in similar microVM services the command
must be `--no-sandbox` explicitly, and the platform's own isolation is the
boundary. Metadata visibility, persistent writes and the absence of domain
allowlists remain disclosed limits. Full hostile-code containment is not
claimed.
