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
| Docker on such a host | host kernel | yes, with Docker's default seccomp profile (Docker 25, measured) | Older engines or custom profiles may block Landlock; check with `pynox doctor` |
| Kubernetes | node kernel | same as Docker, per node | Check nodes with `pynox doctor` |
| AWS Lambda, `provided.al2023` arm64 | 5.10 (measured 2026-10-18) | **no** | Landlock `ENOSYS`; `prctl` and `pidfd_open` blocked; see below |
| AWS Fargate | Firecracker guest kernel | expect no; verify | Same family as Lambda |

## Docker

Each release publishes images per CPython minor and libc, for `linux/amd64`
and `linux/arm64`:

```text
ghcr.io/ferrumio/pynox:0.1.0-py3.12           Debian bookworm-slim, glibc
ghcr.io/ferrumio/pynox:0.1.0-py3.12-alpine    Alpine 3.22, musl
ghcr.io/ferrumio/pynox:py3.12                 latest release for that Python
```

The image is the base plus the bundle in `/opt/pynox`, nothing else: no build
step runs inside it. The entry point is `pynox`, the working directory `/app`
and the user an unprivileged `65532`.

```bash
docker run --rm -v "$PWD:/app:ro" ghcr.io/ferrumio/pynox:0.1.0-py3.12 job.py
docker run --rm -i --read-only ghcr.io/ferrumio/pynox:0.1.0-py3.12 pool --workers=4
```

Build your own on top, with your dependencies:

```dockerfile
FROM ghcr.io/ferrumio/pynox:0.1.0-py3.12
COPY --chown=65532:65532 . /app
CMD ["pool", "--config", "/app/pynox.toml"]
```

Measured on 2026-10-18 with Docker 25.0.16 on Linux 6.18 (arm64), with the
default seccomp profile and no extra flags: `pynox doctor` reports
`sandbox_supported: true`, sandboxed scripts run, a readable file outside the
grants is denied while `--no-sandbox` reads it, and the pool works on a
read-only root filesystem. Landlock and seccomp need no privilege, so the
non-root user is enough. Older Docker engines and custom profiles may still
block the Landlock calls; `pynox doctor` names the blocked interface. Pynox's
own CI runs its build containers with `seccomp=unconfined` because it builds
and tests there, not because the runtime needs it.

`deploy/docker/Dockerfile` and `scripts/image_context.py` build the same image
from local bundles:

```bash
python scripts/image_context.py ctx dist/pynox-0.1.0-cp312-linux-gnu-*.tar.gz
docker buildx build --platform linux/amd64,linux/arm64 -t pynox:py3.12 ctx
```

## Kubernetes

The node's kernel and container runtime decide, exactly as with Docker. Run a
one-off pod with `pynox doctor` on each node pool before relying on the
sandbox, and set the pod's `seccompProfile` so the Landlock and seccomp
syscalls are allowed. Managed node images vary: check, do not assume.

## AWS Lambda

Lambda's kernel cannot run the sandbox, measured on 2026-10-18 in `eu-west-2`
with a `provided.al2023` arm64 function:

| Probe | Result |
| --- | --- |
| Guest kernel | `5.10.255` (Amazon Linux 2 kernel) |
| `landlock_create_ruleset` | `ENOSYS`; Landlock needs 5.13+ |
| `prctl(PR_SET_NO_NEW_PRIVS)`, `prctl(PR_SET_PDEATHSIG)` | `EPERM`: Lambda's own seccomp denies them |
| `pidfd_open`, `close_range` | `ENOSYS` |
| `unshare(CLONE_NEWUSER)` | `EPERM` |

So `pynox script.py` and `pynox pool` cannot start there, and `pynox doctor`
lists all three reasons. Lambda already isolates each function in a microVM;
the pattern for agent code is a dedicated function per trust level. What
Pynox brings to Lambda is the runtime.

### The layer

Each release ships `pynox-lambda-layer-<version>-cp3XY-<arch>.zip`. It
unpacks to `/opt/pynox`, `/opt/bootstrap` and `/opt/pynox-runtime`, a custom
runtime that runs your `_HANDLER` (`module.function`) on Pynox without the
sandbox, with the `(event, context)` signature of the managed Python runtimes.
The layer leaves out what a function cannot use: Tk, IDLE, ensurepip, C
headers, pip and setuptools.

```bash
aws lambda publish-layer-version --layer-name pynox-py311-arm64 \
  --content S3Bucket=my-bucket,S3Key=pynox-lambda-layer-0.1.0-cp311-arm64.zip \
  --compatible-runtimes provided.al2023 --compatible-architectures arm64
aws lambda create-function --function-name my-fn --runtime provided.al2023 --architectures arm64 \
  --handler handler.handler --zip-file fileb://function.zip --role "$ROLE_ARN" \
  --layers arn:aws:lambda:REGION:ACCOUNT:layer:pynox-py311-arm64:1
```

`function.zip` holds only your code; dependencies go in it or in another layer,
built for the same CPython minor and architecture. `scripts/lambda_layer.py`
builds the layer from a local glibc bundle.

### How it compares

Same handler on both, 1024 MB, arm64, writing 20,000 records with `json.dump`
to `/tmp`; four cold starts for the layer, three for the managed runtime:

| | Pynox layer | Managed `python3.11` |
| --- | ---: | ---: |
| Init duration, cold | 181–218 ms | 57–75 ms |
| Handler duration, warm | 18–52 ms | 130–176 ms |
| `json.dump` inside the handler, warm | 30–47 ms | 124–168 ms |
| Max memory used | 49 MB | 39 MB |

The managed runtime starts faster: its files are cached on Lambda's hosts,
while a layer's files are read for the first time on every cold start. The
layer's runtime loop speaks the Runtime API over a plain socket for that
reason; with `urllib` its init measured about 450 ms. Pynox runs the work
faster, here 4× on `json.dump`, so whether a cold invocation is faster
overall depends on the handler. For this one, init plus execution was about
240 ms with the layer and 250–330 ms with the managed runtime.

Expect Lambda's kernel to change without notice; keep `--no-sandbox`
explicit, which the layer's bootstrap does, so a future kernel with Landlock
does not silently change a function's behavior.
