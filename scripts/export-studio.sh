#!/usr/bin/env bash
# Exports the Learning Studio as a static site: the same IDE page, lessons, source viewer, and
# embedded game as the local server, with the server's /api responses saved as files.
#
#   ./scripts/export-studio.sh <output-dir> [wasm-build-dir]
#
# The wasm build dir defaults to ringen-van-patronage-origins/target/game-web, which
# scripts/studio.sh and the IDE's "Recompile game" button both write to.
# Editing, recompiling, and project checks need cargo, so they stay in the local studio.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
game_dir="$root_dir/ringen-van-patronage-origins"
viewer_dir="$game_dir/viewer"

out_dir="${1:?usage: export-studio.sh <output-dir> [wasm-build-dir]}"
build_dir="${2:-$game_dir/target/game-web}"

for file in ringen_game.js ringen_game_bg.wasm; do
    if [[ ! -f "$build_dir/$file" ]]; then
        echo "Missing $build_dir/$file. Build the browser game first (./scripts/studio.sh or the CI build)." >&2
        exit 1
    fi
done

if [[ -e "$out_dir" && -n "$(ls -A "$out_dir")" ]]; then
    echo "Refusing to export into non-empty directory: $out_dir" >&2
    exit 1
fi
mkdir -p "$out_dir/game" "$out_dir/game-build" "$out_dir/api/levels"

# Same files the server embeds; only the mode flag differs.
sed 's#<meta name="studio-mode" content="live" />#<meta name="studio-mode" content="static" />#' \
    "$viewer_dir/index.html" > "$out_dir/index.html"
if ! grep -q 'content="static"' "$out_dir/index.html"; then
    echo "index.html has no studio-mode meta tag to switch to static mode." >&2
    exit 1
fi
cp "$viewer_dir/styles.css" "$viewer_dir/app.js" "$out_dir/"
cp "$viewer_dir/game.html" "$out_dir/game/index.html"
cp "$viewer_dir/game-loader.js" "$out_dir/game/loader.js"
cp "$build_dir/ringen_game.js" "$build_dir/ringen_game_bg.wasm" "$out_dir/game-build/"
touch "$out_dir/.nojekyll"

# api/levels.json mirrors GET /api/levels; api/levels/<id>/source.txt mirrors .../source.
python3 - "$game_dir" "$out_dir" <<'PY'
import json
import pathlib
import sys

game_dir = pathlib.Path(sys.argv[1]).resolve()
out_dir = pathlib.Path(sys.argv[2]).resolve()

level_files = sorted(game_dir.glob("data/levels/level-[0-9][0-9][0-9].json"))
if not level_files:
    sys.exit("No level files found in data/levels")
levels = [json.loads(path.read_text(encoding="utf-8")) for path in level_files]
(out_dir / "api" / "levels.json").write_text(json.dumps(levels, ensure_ascii=False), encoding="utf-8")

for level in levels:
    source = (game_dir / level["source_path"]).resolve()
    source.relative_to(game_dir)  # the source must live inside the project
    target = out_dir / "api" / "levels" / level["id"]
    target.mkdir(parents=True, exist_ok=True)
    (target / "source.txt").write_text(source.read_text(encoding="utf-8"), encoding="utf-8")

print(f"Exported {len(levels)} levels")
PY

echo "Static studio written to $out_dir"
