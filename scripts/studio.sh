#!/usr/bin/env bash
# Builds and starts the Web Learning Studio: a local server that compiles the
# Bevy game to WebAssembly and serves the browser IDE (editor, lessons, and
# the playable game) at http://127.0.0.1:4173.
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
game_dir="$root_dir/ringen-van-patronage-origins"
url="http://127.0.0.1:4173"

if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "Installing the wasm32-unknown-unknown target..."
    rustup target add wasm32-unknown-unknown
fi

if ! command -v wasm-bindgen >/dev/null 2>&1; then
    echo "Installing wasm-bindgen-cli 0.2.130..."
    cargo install wasm-bindgen-cli --version 0.2.130 --locked
fi

echo "Starting the Web Learning Studio at $url"
(cd "$game_dir" && cargo run -- --viewer) &
server_pid=$!
trap 'kill "$server_pid" 2>/dev/null' EXIT

for _ in $(seq 1 60); do
    if curl --silent --head --fail "$url" >/dev/null 2>&1; then
        break
    fi
    sleep 1
done

if command -v open >/dev/null 2>&1; then
    open "$url"
fi

wait "$server_pid"
