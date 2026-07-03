#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

package_path_file="$(mktemp)"
trap 'rm -f "$package_path_file"' EXIT

printf '\n==> ./scripts/package-release.sh\n'
./scripts/package-release.sh >"$package_path_file"

archive_path="$(<"$package_path_file")"

printf '\n==> ./scripts/smoke-package.sh %s\n' "$archive_path"
./scripts/smoke-package.sh "$archive_path"
