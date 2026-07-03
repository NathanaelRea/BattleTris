#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
original_args=("$@")

usage() {
    printf '%s\n' "Usage: $0 [--out-dir target/visual/legacy] [--fixture NAME ...]"
    printf '%s\n' ""
    printf '%s\n' "Default fixtures: startup challenge sleep about roster game-playing"
    printf '%s\n' "Reachable fixtures: startup challenge sleep about roster game-playing"
}

for arg in "$@"; do
    case "$arg" in
        -h|--help)
            usage
            exit 0
            ;;
    esac
done

need() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'missing required command: %s\n' "$1" >&2
        exit 1
    fi
}

out_dir="$repo_root/target/visual/legacy"
fixtures=()

while [[ $# -gt 0 ]]; do
    case "$1" in
        --out-dir)
            if [[ $# -lt 2 ]]; then
                printf '%s requires a value\n' "$1" >&2
                exit 2
            fi
            out_dir="$2"
            shift 2
            ;;
        --fixture)
            if [[ $# -lt 2 ]]; then
                printf '%s requires a value\n' "$1" >&2
                exit 2
            fi
            fixtures+=("$2")
            shift 2
            ;;
        --fixture=*)
            fixtures+=("${1#--fixture=}")
            shift
            ;;
        *)
            printf 'unrecognized argument: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
done

if [[ ${#fixtures[@]} -eq 0 ]]; then
    fixtures=(startup challenge sleep about roster game-playing)
fi

write_manifest() {
    local manifest=$1
    {
        printf 'command=%q' "$0"
        for arg in "${original_args[@]}"; do
            printf ' %q' "$arg"
        done
        printf '\n'
        printf 'git_sha=%s\n' "$(git rev-parse --short HEAD 2>/dev/null || printf unknown)"
        printf 'display=%s\n' "${DISPLAY:-private-xvfb-per-fixture}"
        printf 'screen=1280x1024x24\n'
        printf 'binary=%s\n' "$legacy_bin"
        printf 'fixtures=%s\n' "${fixtures[*]}"
    } > "$manifest"
}

if [[ "${BATTLETRIS_LEGACY_INSIDE_XVFB:-}" != "1" ]]; then
    need xvfb-run
    mkdir -p "$out_dir"
    for fixture in "${fixtures[@]}"; do
        BATTLETRIS_LEGACY_INSIDE_XVFB=1 xvfb-run -a -s "-screen 0 1280x1024x24" "$0" --out-dir "$out_dir" --fixture "$fixture"
    done
    legacy_bin="$repo_root/usr/src/game/BattleTris"
    write_manifest "$out_dir/manifest.txt"
    printf 'legacy visual manifest -> %s\n' "$out_dir/manifest.txt"
    exit 0
fi

need xdotool
need xwd
if command -v magick >/dev/null 2>&1; then
    image_convert=(magick xwd:- png:-)
elif command -v convert >/dev/null 2>&1; then
    image_convert=(convert xwd:- png:-)
else
    printf 'missing required command: magick or convert\n' >&2
    exit 1
fi

legacy_bin="$repo_root/usr/src/game/BattleTris"
if [[ ! -x "$legacy_bin" ]]; then
    printf 'legacy binary is not executable: %s\n' "$legacy_bin" >&2
    exit 1
fi

mkdir -p "$out_dir"

wm_pid=""
for wm in openbox fluxbox matchbox-window-manager twm; do
    if command -v "$wm" >/dev/null 2>&1; then
        "$wm" >/dev/null 2>&1 &
        wm_pid=$!
        sleep 0.25
        break
    fi
done

cleanup() {
    if [[ -n "$wm_pid" ]]; then
        kill "$wm_pid" >/dev/null 2>&1 || true
        wait "$wm_pid" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

wait_for_window() {
    local pattern=${1:-BattleTris}
    local attempt windows
    for attempt in $(seq 1 100); do
        mapfile -t windows < <(xdotool search --onlyvisible --name "$pattern" 2>/dev/null || true)
        if [[ ${#windows[@]} -gt 0 ]]; then
            printf '%s\n' "${windows[$((${#windows[@]} - 1))]}"
            return 0
        fi
        sleep 0.1
    done
    printf 'timed out waiting for %s window\n' "$pattern" >&2
    return 1
}

click_window() {
    local window=$1
    local x=$2
    local y=$3
    xdotool windowfocus "$window" sleep 0.5 mousemove --sync --window "$window" "$x" "$y" mousedown 1 sleep 0.08 mouseup 1
    sleep 0.5
}

activate_focused_button() {
    local window=$1
    xdotool windowfocus "$window" sleep 0.5 key space
    sleep 0.5
}

capture_window() {
    local window=$1
    local output=$2
    xdotool windowmove "$window" 20 20 >/dev/null 2>&1 || true
    sleep 0.2
    xwd -silent -id "$window" | "${image_convert[@]}" > "$output"
}

run_legacy() {
    local fixture=$1
    shift
    local log_file="$out_dir/${fixture}.log"
    "$legacy_bin" "$@" >"$log_file" 2>&1 &
    printf '%s\n' "$!"
}

capture_fixture() {
    local fixture=$1
    local legacy_pid window output
    output="$out_dir/$fixture.png"

    case "$fixture" in
        startup)
            legacy_pid=$(run_legacy "$fixture" -X -m)
            window=$(wait_for_window)
            capture_window "$window" "$output"
            ;;
        challenge)
            legacy_pid=$(run_legacy "$fixture" -X -m)
            window=$(wait_for_window)
            activate_focused_button "$window"
            window=$(wait_for_window)
            capture_window "$window" "$output"
            ;;
        sleep)
            legacy_pid=$(run_legacy "$fixture" -X -m -s)
            window=$(wait_for_window biff)
            capture_window "$window" "$output"
            ;;
        about)
            legacy_pid=$(run_legacy "$fixture" -X -m)
            window=$(wait_for_window)
            click_window "$window" 210 545
            window=$(wait_for_window)
            capture_window "$window" "$output"
            ;;
        roster)
            legacy_pid=$(run_legacy "$fixture" -X -m)
            window=$(wait_for_window)
            click_window "$window" 320 560
            window=$(wait_for_window)
            capture_window "$window" "$output"
            ;;
        game-playing)
            legacy_pid=$(run_legacy "$fixture" -X -m)
            window=$(wait_for_window)
            activate_focused_button "$window"
            window=$(wait_for_window)
            click_window "$window" 170 488
            window=$(wait_for_window)
            sleep 0.5
            capture_window "$window" "$output"
            ;;
        *)
            printf 'legacy fixture is not automated yet: %s\n' "$fixture" >&2
            return 2
            ;;
    esac

    kill "$legacy_pid" >/dev/null 2>&1 || true
    wait "$legacy_pid" >/dev/null 2>&1 || true
    printf 'captured %s -> %s\n' "$fixture" "$output"
}

manifest="$out_dir/manifest.txt"
write_manifest "$manifest"

for fixture in "${fixtures[@]}"; do
    capture_fixture "$fixture"
done

printf 'legacy visual manifest -> %s\n' "$manifest"
