# Deploy: containers, Kubernetes and Lambda

Pynox ships as a self-contained bundle: `bin/pynox` plus its CPython. Nothing
is downloaded or written at run time, so the same archive works in a Docker
image, a read-only root filesystem or a Lambda package. What varies is the
**kernel**, and with it whether the sandbox can run.

## The kernel decides

The sandbox needs Landlock ABI 3 (Linux 6.2 or newer) and the ability to
install a seccomp filter. `pynox doctor` reports `landlock_abi` and
`sandbox_supported`; when the sandbox cannot start, `pynox` exits 125 with the
reason and never falls back to unrestricted execution. Use `--no-sandbox`
explicitly where the kernel cannot do it.

| Environment | Kernel | Sandbox | Notes |
| --- | --- | --- | --- |
| Your Linux host, VM, EC2 | distribution kernel | yes, if 6.2+ | Where the default mode shines |
| Docker on such a host | host kernel | yes, with a seccomp profile that allows `landlock_*` | Docker's default profile may block them; see below |
| Kubernetes | node kernel | same as Docker, per node | Check nodes with `pynox doctor` |
| AWS Lambda, `provided.al2023` arm64 | 5.10 (measured 2026-10-18) | **no** | Landlock `ENOSYS`; `prctl` and `pidfd_open` blocked; see below |
| AWS Fargate | Firecracker guest kernel | expect no; verify | Same family as Lambda |

## Docker

```dockerfile
FROM debian:bookworm-slim
ADD https://github.com/ferrumio/pynox/releases/download/v0.1.0/pynox-0.1.0-cp312-linux-gnu-x86_64.tar.gz /tmp/pynox.tgz
RUN tar -xzf /tmp/pynox.tgz -C /opt \
 && ln -s /opt/pynox-0.1.0-cp312-linux-gnu-x86_64/bin/pynox /usr/local/bin/pynox \
 && rm /tmp/pynox.tgz
COPY . /app
WORKDIR /app
CMD ["pynox", "pool", "--config", "/app/pynox.toml"]
```

Pick the bundle for your base image: glibc 2.28+ for Debian, Ubuntu, Amazon
Linux and RHEL-family images; musl 1.2+ for Alpine. The container itself can
be read-only.

Docker applies its own seccomp profile to the container. Pynox's sandbox makes
the `landlock_create_ruleset`, `landlock_add_rule`, `landlock_restrict_self`,
`seccomp` and `prctl` calls, and `pidfd_open` in the supervisor. If
`pynox doctor` inside the container shows `landlock_abi: -1` while the host
kernel is 6.2+, the profile is blocking them. Pynox's own CI runs its
containers with `--security-opt seccomp=unconfined` for this reason; a custom
profile that allows those syscalls is the narrower option. Running the
container as a non-root user is fine: Landlock and seccomp need no privilege.

Run the pool with `--log=json` and ship container stderr to your log system;
poll `{"stats": true}` for health and counters. See
[observability](guides/pool.md#observability).

## Kubernetes

The node's kernel and container runtime decide, exactly as with Docker. Run a
one-off pod with `pynox doctor` on each node pool before relying on the
sandbox, and set the pod's `seccompProfile` so the Landlock and seccomp
syscalls are allowed. Managed node images vary: check, do not assume.

## AWS Lambda

Measured on 2026-10-18 in `eu-west-2` with a `provided.al2023` arm64
function, 1769 MB, and the `cp311-linux-gnu-aarch64` bundle as the runtime
(a `bootstrap` script that executes
`pynox --no-sandbox runtime.py`, a 20-line Runtime API loop):

| Probe | Result |
| --- | --- |
| Guest kernel | `5.10.255` (Amazon Linux 2 kernel) |
| `landlock_create_ruleset` | `ENOSYS`; Landlock needs 5.13+ |
| `prctl(PR_SET_NO_NEW_PRIVS)`, `prctl(PR_SET_PDEATHSIG)` | `EPERM`: Lambda's own seccomp denies them |
| `pidfd_open`, `close_range` | `ENOSYS` |
| `unshare(CLONE_NEWUSER)` | `EPERM` |

So `pynox script.py` and `pynox pool` cannot start there: the supervisor's
`prctl` is refused before any policy exists, and Landlock is absent. Pynox
reports this and exits 125. `pynox --no-sandbox` works, and the bundle fits a
zip package (about 62 MB zipped, 98 MB unpacked, under the 250 MB limit) or a
container image. Lambda already isolates each function in its own microVM; the
practical pattern for agent code is a dedicated function per trust level.

What Pynox adds on Lambda is the runtime, not the sandbox. Against the
bundled `python3.11` in the same function, p50 of seven runs each:

| Workload | `python3.11` | `pynox --no-sandbox` |
| --- | ---: | ---: |
| Hello | 14.85 ms | 15.46 ms |
| 200k-iteration loop | 35.71 ms | 38.15 ms |
| JSON pipeline, 3.89 MiB | 350.05 ms | 205.06 ms |

Lambda CPU is slower than a laptop core, which is why every number is higher
than on the [performance page](performance.md); the ratios match: within a
few percent on plain Python, 1.7× on the JSON pipeline.

A `bootstrap` for a zip package:

```sh
#!/bin/sh
set -e
cd "$LAMBDA_TASK_ROOT"
exec "$LAMBDA_TASK_ROOT/pynox/bin/pynox" --no-sandbox "$LAMBDA_TASK_ROOT/runtime.py"
```

Expect Lambda's kernel to change without notice; run `pynox doctor` in a test
invocation when you upgrade, and keep `--no-sandbox` explicit in the command so
a future kernel with Landlock does not silently change the function's behavior.
