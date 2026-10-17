# Installation and Python versions

Pynox 0.1.0 is **pre-alpha**. Pynox and CPython have separate versions.
The build matrix covers CPython 3.11–3.15 with the GIL; the default is 3.11.
`.python-versions` pins one exact release per minor and `.python-version`
selects the default. The Rust adapter uses the complete CPython C API, so each
binary is built for one minor series and architecture. This is not an abi3
binary or a free-threaded build.

## Building from source

Use `make build PYNOX_PYTHON_VERSION=3.14` to select a supported minor, or pass an
exact version listed in `.python-versions`. Changing the selection rebuilds
`target/release/pynox` for that Python. Use the same selection with `make test`
to run the tests in a matching environment.

For a custom CPython installation, set `PYO3_PYTHON=/path/to/python` when building.
It must be CPython 3.11–3.15 with the GIL and a shared libpython. Release bundles
require the exact checked-in pins and a clean Python distribution.

## Release bundles

Release bundles contain:

```text
pynox-0.1.0-cp311-linux-ARCH/
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

`make bundle` creates a tar.gz plus a SHA-256 file for the default Python.
`make bundle PYNOX_PYTHON_VERSION=3.14` builds the `cp314` variant instead.
The packager first relocates the tree and checks Python extensions and actual
sandbox enforcement. It rejects unpinned Python releases for distribution.

```bash
bash scripts/install.sh --archive dist/pynox-0.1.0-cp311-linux-$(uname -m).tar.gz
```

End users need Bash, tar, SHA-256 tools and curl for downloads, but no Python or
Rust. Download mode uses GitHub releases after an operator publishes the assets.
Checksums detect corrupted/mismatched transfers; the release source remains the
trust root.

`PYNOX_VERSION`, `PYNOX_PYTHON_VERSION`, `PYNOX_REPOSITORY`, `PYNOX_INSTALL_ROOT`
and `PYNOX_BIN_DIR` configure installation. Defaults are version 0.1.0, Python
3.11, `ferrumio/pynox`, `$XDG_DATA_HOME/pynox` (or `~/.local/share/pynox`) and
`~/.local/bin`. Download mode accepts minor selections 3.11 through 3.15.
Local archive mode infers the minor from its filename and rejects an explicitly
conflicting selection.

Each Pynox/ABI pair has its own directory, such as `versions/0.1.0-cp311`
and `versions/0.1.0-cp315`. Installation switches a symlink and preserves other
variants. Reinstalling the same pair is refused. To switch back, point the
symlink at a previously installed variant's `bin/pynox`. No installed Python
is required to choose a downloadable variant.

Use external tools such as uv to resolve/lock application dependencies. For
restricted execution, install wheels for the binary's CPython minor and platform,
then grant the resulting site-packages with `--package-dir`. Installation runs
outside sandboxed tasks. An abi3 extension does not make other extensions
version-independent.

Automatic project environment selection, lockfile management and a built-in
dependency resolver remain future work. Version selection is explicit; a binary
does not switch engines in place or automatically translate native wheel ABIs.
