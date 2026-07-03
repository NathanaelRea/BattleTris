#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

usage() {
    printf '%s\n' "Usage: $0 [--theme original-inspired] [--out-dir target/visual/current] [--fixture NAME]"
    printf '%s\n' ""
    printf '%s\n' "Runs the Bevy visual capture harness on a private Xvfb display when available."
    printf '%s\n' "Set BATTLETRIS_VISUAL_ALLOW_DESKTOP=1 to allow use of the current desktop display."
}

theme="original-inspired"
out_dir="target/visual/current"
fixture=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)
            usage
            exit 0
            ;;
        --theme)
            if [[ $# -lt 2 ]]; then
                printf '%s requires a value\n' "$1" >&2
                exit 2
            fi
            theme="$2"
            shift 2
            ;;
        --theme=*)
            theme="${1#--theme=}"
            shift
            ;;
        --out-dir)
            if [[ $# -lt 2 ]]; then
                printf '%s requires a value\n' "$1" >&2
                exit 2
            fi
            out_dir="$2"
            shift 2
            ;;
        --out-dir=*)
            out_dir="${1#--out-dir=}"
            shift
            ;;
        --fixture)
            if [[ $# -lt 2 ]]; then
                printf '%s requires a value\n' "$1" >&2
                exit 2
            fi
            fixture="$2"
            shift 2
            ;;
        --fixture=*)
            fixture="${1#--fixture=}"
            shift
            ;;
        *)
            printf 'unrecognized argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ -n "$fixture" ]]; then
    command=(cargo run --bin client -- headless capture --fixture "$fixture" --theme "$theme" --output "$out_dir/$fixture.png")
else
    command=(cargo run --bin client -- headless capture-all --theme "$theme" --out-dir "$out_dir")
fi

if command -v xvfb-run >/dev/null 2>&1; then
    exec xvfb-run -a -s "-screen 0 1280x1024x24" "${command[@]}"
fi

if command -v gamescope >/dev/null 2>&1; then
    exec gamescope --backend headless -W 1040 -H 720 -w 1040 -h 720 -- "${command[@]}"
fi

if [[ "${BATTLETRIS_VISUAL_ALLOW_DESKTOP:-}" == "1" ]]; then
    printf 'xvfb-run not found; using current desktop because BATTLETRIS_VISUAL_ALLOW_DESKTOP=1\n' >&2
    exec "${command[@]}"
fi

printf 'neither xvfb-run nor gamescope is installed; refusing to open capture windows on the current desktop.\n' >&2
printf 'Install Xvfb/xvfb-run or gamescope, or rerun with BATTLETRIS_VISUAL_ALLOW_DESKTOP=1 if window popups are acceptable.\n' >&2
exit 1
