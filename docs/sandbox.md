# Sandbox contract

The sandbox is the default: `pynox script.py` and every `pynox pool` task run
under it with explicit permissions, and `--no-sandbox` is the only way out. A
sandbox setup failure never falls back to unrestricted execution. The backend
supports Linux glibc and musl and needs Landlock ABI >= 3 (kernel 6.2 or newer).
See [deploy](deploy.md) for what containers and AWS Lambda allow, measured.

Linux Landlock ABI >= 3 mediates reads, writes and truncation. Seccomp denies
new processes/executables, host IPC, cross-process inspection/signals, metadata
mutations and alternate kernel interfaces. Enforcement covers native calls,
including calls made through ctypes, not just Python wrappers.

## Granted capabilities

- The script's directory and the CPython runtime directories are readable; for
  `-m` the current directory is readable instead, and `-c` grants nothing. The
  `implicit_read` field of a JSON result shows exactly what was granted this
  way. Runtime directories include their installed site-packages.
- System library directories `/lib`, `/lib64`, `/usr/lib`, `/usr/lib64`,
  loader cache and `/dev/urandom` are implicitly readable where present.
- Additional read/write/package paths are explicit, existing canonical paths.
  Writes do not imply reads. The policy is not a filesystem snapshot.
- Environment is filtered; selected variables are explicit. Python environment
  path settings do not affect isolated initialization. Loader variables cannot
  be forwarded. New task stdin is `/dev/null`; inherited descriptors are closed.
- Network is denied by default. Unnamed AF_UNIX stream socketpairs are allowed for
  asyncio and internal thread wakeups, but host pathname/abstract sockets are not.
- `--allow-network` grants IPv4/IPv6, including inbound and outbound sockets,
  with no domain/port allowlist. It also grants system DNS and CA configuration
  reads. It does not permit host Unix sockets or subprocesses.

## Limits and remaining boundaries

Wall timeout is enforced by the supervisor. Address-space, CPU, core dump and
descriptor limits apply in the child. Captured output is bounded and excess is
drained, avoiding unbounded supervisor memory. Address space is not physical
memory accounting or a shared budget across concurrent jobs.

Landlock does not hide all metadata (such as stat results) or virtualize the
filesystem. Pynox does not create user/PID/network namespaces, a private root,
cgroups, disk quotas or a transactional workspace. Explicit network access can
reach internal services. Granted writes are persistent. Native memory bugs and
kernel vulnerabilities are outside this developer release's guarantees.

Use grants narrow enough for the task. Policy enforcement is central to Pynox;
full hostile-code containment would require a separately reviewed stronger
boundary. JSON diagnostics are application-controlled, not an audit log.

See ADR 003 for the decision and `tests/test_runtime.py` for the executable
contract. The same tests run on cold, pool and preloaded execution.
