# Installation and Python versions

Pynox and CPython have separate versions. Pynox 0.1.0 supports CPython 3.12
with the GIL; release builds pin 3.12.14. The Rust adapter uses the complete
CPython C API, so it is built for that minor series and architecture.
This is not an abi3 binary or a free-threaded build.

Release bundles contain:

```text
pynox-0.1.0-cp312-linux-ARCH/
  bin/pynox
  python/             matching CPython and standard library
  manifest.json       file hashes and versions
  LICENSE
  README.md
```

They retain the upstream Python distribution and its licenses. User application
frameworks are never copied from the development venv. Dynamic-loader paths and
runtime discovery are relative to the executable. Linux glibc/system libraries
are still required; this is not a static or musl build. CI builds on Ubuntu 24.04;
that is the initial binary deployment baseline, not a promise for older glibc.

`make bundle` creates a tar.gz plus a SHA-256 file. The packager first relocates
the tree and checks Python extensions and actual sandbox enforcement.

```bash
bash scripts/install.sh --archive dist/pynox-0.1.0-cp312-linux-$(uname -m).tar.gz
```

End users need Bash, tar, SHA-256 tools and curl for downloads, but no Python or
Rust. Download mode uses GitHub releases after an operator publishes the assets.
Checksums detect corrupted/mismatched transfers; the release source remains the
trust root.

`PYNOX_VERSION`, `PYNOX_REPOSITORY`, `PYNOX_INSTALL_ROOT` and `PYNOX_BIN_DIR`
configure installation. Defaults are version 0.1.0, `ferrumio/pynox`,
`$XDG_DATA_HOME/pynox` (or `~/.local/share/pynox`) and `~/.local/bin`.
Each version has its own directory; installation switches a symlink and preserves
older versions. Reinstalling an existing version is refused. To roll back, point
the symlink at a previously installed version.

Use external tools such as uv to resolve/lock application dependencies. For
restricted execution, install wheels for CPython 3.12 and the target platform,
then grant the resulting site-packages with `--package-dir`. Installation runs
outside sandboxed tasks. An abi3 extension does not make other extensions
version-independent.

Automatic project environment selection, multiple Python minors, lockfile
management and a built-in dependency resolver are future work. v0.1 provides
a matching embedded runtime without rewriting those tools.
