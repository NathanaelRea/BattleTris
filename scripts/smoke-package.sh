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
    "$package_dir/assets/themes/original/theme.toml"
    "$package_dir/assets/themes/original/images/blocks.png"
    "$package_dir/assets/themes/original/images/startup.png"
    "$package_dir/assets/themes/original/images/bazaar.png"
    "$package_dir/assets/themes/original/images/biff.png"
    "$package_dir/assets/themes/original/images/gimp.png"
    "$package_dir/assets/themes/original/images/crest.png"
    "$package_dir/assets/themes/high-contrast/theme.toml"
    "$package_dir/assets/themes/high-contrast/images/blocks.png"
    "$package_dir/assets/themes/high-contrast/images/startup.png"
    "$package_dir/assets/themes/high-contrast/images/bazaar.png"
    "$package_dir/assets/themes/high-contrast/images/biff.png"
    "$package_dir/assets/themes/high-contrast/images/gimp.png"
    "$package_dir/assets/themes/high-contrast/images/crest.png"
    "$package_dir/assets/sounds/generated-default/sound-pack.toml"
    "$package_dir/assets/sounds/generated-default/menu-action.wav"
    "$package_dir/assets/sounds/generated-default/piece-locked.wav"
    "$package_dir/assets/sounds/generated-default/line-clear.wav"
    "$package_dir/assets/sounds/generated-default/bazaar-entered.wav"
    "$package_dir/assets/sounds/generated-default/purchase.wav"
    "$package_dir/assets/sounds/generated-default/weapon-launch.wav"
    "$package_dir/assets/sounds/generated-default/warning.wav"
    "$package_dir/assets/sounds/generated-default/game-over.wav"
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

for sound in menu-action.wav piece-locked.wav line-clear.wav bazaar-entered.wav purchase.wav weapon-launch.wav warning.wav game-over.wav; do
    if ! grep -q "$sound" "$package_dir/assets/sounds/generated-default/sound-pack.toml"; then
        printf 'sound-pack manifest does not declare required sound %s\n' "$sound" >&2
        exit 1
    fi
done

for theme in original high-contrast; do
    theme_manifest="$package_dir/assets/themes/$theme/theme.toml"
    for asset in images/blocks.png images/startup.png images/bazaar.png images/biff.png images/gimp.png images/crest.png; do
        if ! grep -q "$asset" "$theme_manifest"; then
            printf 'theme manifest %s does not declare required asset %s\n' "$theme_manifest" "$asset" >&2
            exit 1
        fi
    done
done

tools_binary="$package_dir/bin/battletris-tools"
if [[ ! -x "$tools_binary" && -x "$package_dir/bin/battletris-tools.exe" ]]; then
    tools_binary="$package_dir/bin/battletris-tools.exe"
fi
server_binary="$package_dir/bin/battletris-server"
if [[ ! -x "$server_binary" && -x "$package_dir/bin/battletris-server.exe" ]]; then
    server_binary="$package_dir/bin/battletris-server.exe"
fi
if [[ -x "$tools_binary" ]]; then
    for theme in original high-contrast; do
        "$tools_binary" validate-theme "$package_dir/assets/themes/$theme"
    done
    "$tools_binary" net-smoke --help >/dev/null
    "$tools_binary" net-smoke direct-loopback
else
    printf 'packaged tools binary is not executable; skipping decoded theme validation\n' >&2
fi

if [[ -x "$server_binary" && -x "$tools_binary" ]]; then
    "$server_binary" --help >/dev/null
    smoke_port=$((44040 + RANDOM % 1000))
    smoke_db="$scratch/package-smoke.db"
    "$server_binary" --listen "127.0.0.1:$smoke_port" --db "$smoke_db" --community package-smoke --seed 900 >"$scratch/server.out" 2>"$scratch/server.err" &
    server_pid=$!
    cleanup_server() {
        if kill -0 "$server_pid" 2>/dev/null; then
            kill "$server_pid" 2>/dev/null || true
            wait "$server_pid" 2>/dev/null || true
        fi
    }
    trap 'cleanup_server; rm -rf "$scratch"' EXIT
    sleep 1
    "$tools_binary" net-smoke hosted-lobby --server "127.0.0.1:$smoke_port"
    cleanup_server
else
    printf 'packaged server/tools binaries are not executable; skipping networking package smoke\n' >&2
fi

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
