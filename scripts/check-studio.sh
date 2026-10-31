#!/usr/bin/env bash
# Verifies the Learning Studio export without compiling the game: exports the static site from
# stand-in build files, then runs the page script in both modes (see scripts/studio-smoke.js).
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
work_dir="$(mktemp -d)"
trap 'rm -rf "$work_dir"' EXIT

mkdir -p "$work_dir/build"
: > "$work_dir/build/ringen_game.js"
: > "$work_dir/build/ringen_game_bg.wasm"
"$root_dir/scripts/export-studio.sh" "$work_dir/site" "$work_dir/build" > /dev/null

for page in index.html game/index.html game/loader.js app.js; do
    if grep -nE '(src|href)="/[^/]|import\(`/|fetch\(["`]/' "$work_dir/site/$page"; then
        echo "FAIL: $page uses a root-absolute URL, which breaks under a Pages sub-path" >&2
        exit 1
    fi
done
grep -q 'name="studio-mode" content="static"' "$work_dir/site/index.html"
grep -q 'name="studio-mode" content="live"' "$root_dir/ringen-van-patronage-origins/viewer/index.html"
echo "export: ok"

jsc_path=/System/Library/Frameworks/JavaScriptCore.framework/Versions/A/Helpers/jsc
if command -v node > /dev/null 2>&1; then
    run_smoke() { node "$root_dir/scripts/studio-smoke.js" "$@"; }
elif [[ -x "$jsc_path" ]]; then
    run_smoke() { "$jsc_path" "$root_dir/scripts/studio-smoke.js" -- "$@"; }
else
    echo "skip: install Node.js (or use macOS) to run the page smoke test" >&2
    exit 0
fi

# jsc always exits 0, so a run only counts as passing if it also prints the final success line.
smoke_passes() {
    local result
    result="$(run_smoke "$@" 2>&1)" || { printf '%s\n' "$result"; return 1; }
    printf '%s\n' "$result"
    grep -qx 'ALL PASSED' <<< "$result"
}

app_js="$root_dir/ringen-van-patronage-origins/viewer/app.js"
for mode in static live; do
    echo "== studio page, $mode mode"
    smoke_passes "$mode" "$work_dir/site" "$app_js"
done
