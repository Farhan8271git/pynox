# Installation, platforms and Python versions

Pynox 0.1.0 is **pre-alpha**. Its Python matrix covers CPython 3.11–3.15 with
the GIL, defaulting to 3.11. `.python-versions` contains exact release pins.
Every binary embeds one Python minor and targets one OS, libc and architecture.

## Platform matrix

Pynox is **Linux only**. The sandbox is built on Landlock and seccomp, and the
pre-alpha keeps its matrix where that differentiator exists. On macOS, use a
Linux container or VM. See ADR 010.

| Bundle platform | Build environment | Execution modes |
| --- | --- | --- |
| `linux-gnu-x86_64` | manylinux_2_28 x86_64 | Python, sandbox, pool |
| `linux-gnu-aarch64` | manylinux_2_28 ARM64 | Python, sandbox, pool |
| `linux-musl-x86_64` | musllinux_1_2 x86_64 | Python, sandbox, pool |
| `linux-musl-aarch64` | musllinux_1_2 ARM64 | Python, sandbox, pool |

The release matrix has **20 variants**: four platforms × five Python versions.
Manylinux and musllinux are Python wheel compatibility policies and build
environments. Pynox uses their containers to build executable bundles; the
archives themselves are not wheels.

Linux glibc release builds target **glibc 2.28 or newer**. Packaging audits
GLIBC symbol versions across the executable and bundled ELF libraries.
Linux musl builds target **musl 1.2 or newer**, including Alpine, and link musl
dynamically to support CPython and native extensions. Images are pinned by
digest in `.ci/images.json`.

The libc baseline and kernel requirements are separate. Sandboxed execution
still needs Landlock ABI >= 3 and the kernel interfaces described in the
[sandbox contract](sandbox.md). An older userspace on a recent kernel is valid;
an old kernel does not gain sandbox support from a manylinux build.

## Building from source

Use `make build PYNOX_PYTHON_VERSION=3.14` to select a supported minor, or an
exact version listed in `.python-versions`. Each build replaces
`target/release/pynox`. Use the same selection with `make test`.

For a custom CPython installation, set `PYO3_PYTHON=/path/to/python`.
It must have the GIL and shared libpython. Release bundles require the
checked-in pins and a clean Python distribution. This uses the full CPython
C API, rather than a universal abi3 binary.

To reproduce Linux release builds on a matching architecture with Docker:

```bash
bash scripts/ci-linux.sh gnu 3.11.17
bash scripts/ci-linux.sh musl 3.11.17
```

The scripts build and test in pinned PyPA containers, audit and relocate the
bundle, and test installation. Docker's outer seccomp filter is disabled for
these trusted build jobs so Pynox can test its own kernel policy. The host's
kernel must meet the sandbox requirements.

## Bundles and installation

`make bundle` creates a tar.gz and an adjacent SHA-256 file:

```text
pynox-0.1.0-cp311-linux-gnu-aarch64/
  bin/pynox
  python/             matching CPython and standard library
  manifest.json       versions, platform, libc requirement and file hashes
  ADR/                architecture decisions
  docs/
```

Native application frameworks are not bundled. The upstream Python distribution
retains its bootstrap tooling and licenses. End users need Bash, tar and either
`sha256sum` or `shasum`; curl is required for downloads. Python and Rust are not
required on the target machine.

Pass the archive printed by the build to the installer:

```bash
bash scripts/install.sh --archive dist/pynox-0.1.0-cp311-linux-gnu-aarch64.tar.gz
```

The installer checks that the host is Linux with a matching architecture and
libc, then verifies the checksum. Local archive mode infers Python from the
filename and rejects conflicting `PYNOX_PYTHON_VERSION` selections. A glibc
archive cannot be installed on a musl host, or the reverse.

Without `--archive`, the installer downloads the matching platform from GitHub
release assets. These must first be published. Configuration:

| Variable | Default |
| --- | --- |
| `PYNOX_VERSION` | `0.1.0` |
| `PYNOX_PYTHON_VERSION` | `3.11`; downloads accept minors 3.11–3.15 |
| `PYNOX_REPOSITORY` | `ferrumio/pynox` |
| `PYNOX_INSTALL_ROOT` | `$XDG_DATA_HOME/pynox` or `~/.local/share/pynox` |
| `PYNOX_BIN_DIR` | `~/.local/bin` |

Each version, ABI and platform has its own directory, such as
`versions/0.1.0-cp311-linux-gnu-aarch64`. Installation switches the CLI symlink
and preserves other variants. Reinstalling the same variant is refused.
To switch back, point the symlink to a previous variant's `bin/pynox`.
Earlier pre-alpha directory layouts are preserved.

Use uv or other external tooling to resolve application dependencies for the
matching CPython, libc and architecture. Grant installed packages with
`--package-dir` for sandboxed tasks. A `pip install pynox` package delivering
the binary, project environment discovery and a built-in dependency resolver
remain future work.
