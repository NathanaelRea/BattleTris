#!/usr/bin/env bash

set -euo pipefail

os="$(uname -s)"
case "$os" in
    Linux)
        ;;
    *)
        exit 0
        ;;
esac

missing=0

print_install_hint() {
    printf '\nInstall Linux native build dependencies, then rerun this script.\n' >&2
    printf 'Debian/Ubuntu: sudo apt-get update && sudo apt-get install -y pkg-config libasound2-dev\n' >&2
    printf 'Fedora: sudo dnf install pkgconf-pkg-config alsa-lib-devel\n' >&2
    printf 'Arch: sudo pacman -S pkgconf alsa-lib\n' >&2
    printf 'openSUSE: sudo zypper install pkg-config alsa-devel\n' >&2
}

if ! command -v pkg-config >/dev/null 2>&1; then
    printf 'missing native build dependency: pkg-config\n' >&2
    missing=1
else
    if ! pkg_config_error="$(pkg-config --print-errors --exists alsa 2>&1)"; then
        printf 'missing native build dependency: ALSA development files (alsa.pc)\n' >&2
        if [[ -n "$pkg_config_error" ]]; then
            printf '%s\n' "$pkg_config_error" >&2
        fi
        missing=1
    fi
fi

if ((missing != 0)); then
    print_install_hint
    exit 1
fi
