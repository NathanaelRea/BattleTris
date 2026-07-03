#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

host_target="$(rustc -vV | while IFS=: read -r key value; do
    if [[ "$key" == "host" ]]; then
        printf '%s\n' "${value# }"
        break
    fi
done)"
target_triple="${1:-${BATTLETRIS_TARGET:-$host_target}}"
package_id="$(cargo pkgid -p battletris-client)"
version="${BATTLETRIS_VERSION:-${package_id##*#}}"
package_name="battletris-${version}-${target_triple}"
package_dir="dist/${package_name}"

cargo_args=(build --release --locked -p battletris-client -p battletris-server -p battletris-tools)
target_dir="target/release"
if [[ "$target_triple" != "$host_target" ]]; then
    cargo_args+=(--target "$target_triple")
    target_dir="target/${target_triple}/release"
fi

cargo "${cargo_args[@]}"

rm -rf "$package_dir"
mkdir -p "$package_dir/bin" "$package_dir/assets" "$package_dir/docs"

exe_suffix=""
if [[ "$target_triple" == *windows* ]]; then
    exe_suffix=".exe"
fi

install_binary() {
    local source_name="$1"
    local dest_name="$2"
    local source_path="${target_dir}/${source_name}${exe_suffix}"

    if [[ ! -x "$source_path" && ! -f "$source_path" ]]; then
        printf 'missing expected release binary: %s\n' "$source_path" >&2
        exit 1
    fi

    cp "$source_path" "${package_dir}/bin/${dest_name}${exe_suffix}"
}

install_binary client battletris-client
install_binary battletris-server battletris-server
install_binary battletris-tools battletris-tools

cp -R assets/. "$package_dir/assets/"
cp docs/rewrite-spec.md docs/traceability-checklist.md docs/rust-workspace.md docs/distribution.md "$package_dir/docs/"
cp Cargo.lock "$package_dir/"

cat >"$package_dir/README.md" <<README
# BattleTris ${version}

This archive contains the BattleTris desktop client, optional self-hosted server,
developer tools, converted/default assets, generated sound cues, and release
documentation snapshots.

## Run

- Client: \`bin/battletris-client${exe_suffix}\`
- Server: \`bin/battletris-server${exe_suffix}\`
- Tools: \`bin/battletris-tools${exe_suffix}\`

Packaged assets live in \`assets/\` next to this README. User settings and save
data use the platform project directories selected in ADR 0005.
README

cat >"$package_dir/release-manifest.toml" <<MANIFEST
name = "BattleTris"
version = "${version}"
target = "${target_triple}"
asset_manifest = "assets/manifest.toml"
client_binary = "bin/battletris-client${exe_suffix}"
server_binary = "bin/battletris-server${exe_suffix}"
tools_binary = "bin/battletris-tools${exe_suffix}"
MANIFEST

archive_path="dist/${package_name}.tar.gz"
rm -f "$archive_path" "${archive_path}.sha256"
tar -czf "$archive_path" -C dist "$package_name"

if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$archive_path" >"${archive_path}.sha256"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$archive_path" >"${archive_path}.sha256"
fi

printf '%s\n' "$archive_path"
