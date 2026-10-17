#!/usr/bin/env bash
# Install a released or locally built bundle. No Python/Rust is required.
set -euo pipefail
version="${PYNOX_VERSION:-0.1.0}"
python_minor="${PYNOX_PYTHON_VERSION:-3.11}"
repository="${PYNOX_REPOSITORY:-ferrumio/pynox}"
install_root="${PYNOX_INSTALL_ROOT:-${XDG_DATA_HOME:-$HOME/.local/share}/pynox}"
bin_dir="${PYNOX_BIN_DIR:-$HOME/.local/bin}"
local_archive=""
architecture="$(uname -m)"
[[ "$architecture" != arm64 ]] || architecture=aarch64
[[ "$architecture" == x86_64 || "$architecture" == aarch64 ]] || { echo "Unsupported architecture" >&2; exit 1; }
case "$(uname -s)" in
    Darwin) platform="macos-$architecture" ;;
    Linux)
        if [[ -e "/lib/ld-musl-$architecture.so.1" ]]; then
            platform="linux-musl-$architecture"
        else
            platform="linux-gnu-$architecture"
        fi
        ;;
    *) echo "Pynox requires Linux or macOS" >&2; exit 1 ;;
esac
if [[ "${1:-}" == "--archive" && $# == 2 ]]; then
    local_archive="$2"
elif [[ $# != 0 ]]; then
    echo "Usage: bash install.sh [--archive bundle.tar.gz]" >&2
    exit 2
fi
if [[ -n "$local_archive" ]]; then
    archive_name="$(basename "$local_archive")"
    if [[ "$archive_name" =~ ^pynox-"${version}"-cp(311|312|313|314|315)-((linux-(gnu|musl)|macos)-(x86_64|aarch64))\.tar\.gz$ ]]; then
        archive_abi="${BASH_REMATCH[1]}"
        archive_platform="${BASH_REMATCH[2]}"
        [[ "$archive_platform" == "$platform" ]] || { echo "Archive platform does not match this host ($platform)" >&2; exit 2; }
        archive_python="3.${archive_abi:1}"
        if [[ -n "${PYNOX_PYTHON_VERSION:-}" && "$python_minor" != "$archive_python" ]]; then
            echo "Archive Python version does not match PYNOX_PYTHON_VERSION" >&2
            exit 2
        fi
        python_minor="$archive_python"
    else
        echo "Unrecognized Pynox archive name" >&2
        exit 2
    fi
fi
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Invalid PYNOX_VERSION" >&2; exit 2; }
[[ "$python_minor" =~ ^3\.(11|12|13|14|15)$ ]] || { echo "Choose PYNOX_PYTHON_VERSION=3.11 through 3.15" >&2; exit 2; }
name="pynox-${version}-cp${python_minor//./}-${platform}"
destination="$install_root/versions/$version-cp${python_minor//./}-${platform}"
[[ ! -e "$destination" ]] || { echo "Version already installed: $destination" >&2; exit 1; }
mkdir -p "$install_root/versions" "$bin_dir"
staging="$(mktemp -d "$install_root/versions/.install-XXXXXXXX")"
trap 'rm -rf -- "$staging"' EXIT
if [[ -n "$local_archive" ]]; then
    cp -- "$local_archive" "$staging/$name.tar.gz"
    cp -- "$local_archive.sha256" "$staging/$name.tar.gz.sha256"
else
    base="https://github.com/$repository/releases/download/v$version"
    curl --proto '=https' --tlsv1.2 -fLsS "$base/$name.tar.gz" -o "$staging/$name.tar.gz"
    curl --proto '=https' --tlsv1.2 -fLsS "$base/$name.tar.gz.sha256" -o "$staging/$name.tar.gz.sha256"
fi
expected="$(awk 'NR == 1 {print $1}' "$staging/$name.tar.gz.sha256")"
[[ "$expected" =~ ^[a-f0-9]{64}$ ]] || { echo "Invalid checksum file" >&2; exit 1; }
if command -v sha256sum >/dev/null 2>&1; then
    actual="$(sha256sum "$staging/$name.tar.gz")"
else
    actual="$(shasum -a 256 "$staging/$name.tar.gz")"
fi
[[ "${actual%% *}" == "$expected" ]] || { echo "Checksum mismatch" >&2; exit 1; }
tar -xzf "$staging/$name.tar.gz" -C "$staging" --no-same-owner
"$staging/$name/bin/pynox" doctor >/dev/null
mv -- "$staging/$name" "$destination"
ln -s "$destination/bin/pynox" "$staging/pynox"
[[ ! -d "$bin_dir/pynox" ]] || { echo "CLI destination is a directory: $bin_dir/pynox" >&2; exit 1; }
mv -f -- "$staging/pynox" "$bin_dir/pynox"
echo "Installed Pynox $version: $bin_dir/pynox"
echo "Add $bin_dir to PATH if needed."
