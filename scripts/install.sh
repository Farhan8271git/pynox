#!/usr/bin/env bash
# Install a released or locally built bundle. No Python/Rust is required.
set -euo pipefail
version="${PYNOX_VERSION:-0.1.0}"
repository="${PYNOX_REPOSITORY:-ferrumio/pynox}"
install_root="${PYNOX_INSTALL_ROOT:-${XDG_DATA_HOME:-$HOME/.local/share}/pynox}"
bin_dir="${PYNOX_BIN_DIR:-$HOME/.local/bin}"
local_archive=""
if [[ "${1:-}" == "--archive" && $# == 2 ]]; then
    local_archive="$2"
elif [[ $# != 0 ]]; then
    echo "Usage: bash install.sh [--archive bundle.tar.gz]" >&2
    exit 2
fi
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Invalid PYNOX_VERSION" >&2; exit 2; }
[[ "$(uname -s)" == Linux ]] || { echo "Pynox 0.1 requires Linux" >&2; exit 1; }
architecture="$(uname -m)"
[[ "$architecture" == x86_64 || "$architecture" == aarch64 ]] || exit 1
name="pynox-${version}-cp312-linux-${architecture}"
destination="$install_root/versions/$version"
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
actual="$(sha256sum "$staging/$name.tar.gz")"
[[ "${actual%% *}" == "$expected" ]] || { echo "Checksum mismatch" >&2; exit 1; }
tar -xzf "$staging/$name.tar.gz" -C "$staging" --no-same-owner
"$staging/$name/bin/pynox" doctor >/dev/null
mv -- "$staging/$name" "$destination"
ln -s "$destination/bin/pynox" "$staging/pynox"
mv -Tf -- "$staging/pynox" "$bin_dir/pynox"
echo "Installed Pynox $version: $bin_dir/pynox"
echo "Add $bin_dir to PATH if needed."
