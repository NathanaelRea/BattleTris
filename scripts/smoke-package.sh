#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

archive_path="${1:-}"
if [[ -z "$archive_path" ]]; then
    printf 'usage: %s dist/battletris-<version>-<target>.tar.gz\n' "$0" >&2
    exit 2
fi

if [[ ! -f "$archive_path" ]]; then
    printf 'package archive does not exist: %s\n' "$archive_path" >&2
    exit 1
fi

scratch="$(mktemp -d)"
trap 'rm -rf "$scratch"' EXIT

tar -xzf "$archive_path" -C "$scratch"
package_dir="$(find "$scratch" -mindepth 1 -maxdepth 1 -type d | sort | head -n 1)"

required_files=(
    "$package_dir/README.md"
    "$package_dir/release-manifest.toml"
    "$package_dir/Cargo.lock"
    "$package_dir/assets/manifest.toml"
    "$package_dir/assets/themes/original-inspired/theme.toml"
    "$package_dir/assets/themes/high-contrast/theme.toml"
    "$package_dir/assets/sounds/generated-default/sound-pack.toml"
    "$package_dir/docs/rewrite-spec.md"
    "$package_dir/docs/traceability-checklist.md"
    "$package_dir/docs/distribution.md"
)

for required in "${required_files[@]}"; do
    if [[ ! -f "$required" ]]; then
        printf 'package missing required file: %s\n' "$required" >&2
        exit 1
    fi
done

binary_count=0
for binary in "$package_dir"/bin/*; do
    if [[ -f "$binary" ]]; then
        binary_count=$((binary_count + 1))
    fi
done

if [[ "$binary_count" -lt 3 ]]; then
    printf 'package should include client, server, and tools binaries\n' >&2
    exit 1
fi

if ! grep -q 'asset_manifest = "assets/manifest.toml"' "$package_dir/release-manifest.toml"; then
    printf 'release manifest does not point at bundled assets\n' >&2
    exit 1
fi

printf 'package smoke passed: %s\n' "$archive_path"
