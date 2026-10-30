# Ringen Van Patronage: Origins

A small Rust learning game with two connected environments:

- **Bevy game:** a pure level menu and playable 3D levels.
- **Web Learning Studio:** a local browser IDE-style viewer for the Rust source, explanations, review prompts, practice tasks, readings, and project checks.

The Web Learning Studio combines editable Rust source, lesson notes, review prompts, readings, and the live Bevy game. The game runs as WebAssembly in an embedded browser canvas, so the menu and every level are playable in the IDE. A small Rust HTTP server reads the level JSON and source files directly from this project; the interface uses plain HTML, CSS, and JavaScript.

## Run the game

```sh
cd ringen-van-patronage-origins
cargo run
```

Choose a level to play. Select **Open Web Learning Studio** in the desktop game's menu or a level HUD to start the local IDE at `http://127.0.0.1:4173`. The IDE also includes the actual game menu and levels in its embedded preview.

To start only the viewer from a terminal:

```sh
cargo run -- --viewer
```

The IDE includes **Learn**, **Review**, **Practice**, and **Readings** tabs. The source editor opens at a readable height without manual resizing. Edit the selected Rust file and choose **Save & Recompile**: the IDE writes to that level's `source_path`, builds the browser game, and reloads the preview when compilation succeeds. Expand the game preview when you need more room; scroll over the game menu or use the arrow and Page Up/Down keys to reach every level. The build output stays in the IDE. The Review tab runs `cargo fmt --all -- --check`, `cargo check`, and Clippy for the desktop game binary. Local Rust Book, Rust by Example, and standard-library pages are served from the reading shelf when present; the viewer also links to official online references.

### Browser build setup

The IDE builds the game for WebAssembly on first use. Install the target and matching `wasm-bindgen` command once:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked
```

The editor only writes the Rust source file listed for the selected level. Recompiling produces a browser build under `target/`; the desktop `cargo run` workflow remains available.

## Levels

1. **It all begins with a ring** — a torus floats and rotates in 3D space.
2. **The Void** — move a glowing flame over a tiled floor with W/A/S/D or arrow keys.
3. **The Labyrinth** — guide the fire through 12 generated mazes and collect a ring in each one.
4. **The Hook** — aim a chain hook with the mouse, catch a floating ring, and reel it back to the fire.
5. **The Infinite Labyrinth** — explore endlessly generated, increasingly intricate mazes; arc the hook over walls to catch a ring and teleport the fire to its former position.
6. **The Ringways** — travel through portal-linked maze schemas, catch autonomous rings across 72 rarity tiers, and follow their saved state between worlds.

## Project data

- `data/levels/level-001.json` through `level-006.json` are the shared level catalog. `level-006-schemas.json` stores Level 6 maze and portal values. The game reads menu fields; the viewer consumes the explanations, algorithm notes, curriculum, and readings.
- `data/saves/progress.json` records the levels opened in the game.
- `src/levels/` contains the Bevy scenes and game algorithms.
- `src/viewer_server.rs` serves the browser IDE, selected Rust source, offline readings, review commands, and WebAssembly build on loopback only.
- `viewer/` contains the browser IDE and Bevy canvas loader. It can be edited directly without a frontend package manager.
- `reading/` in the parent Rust learning folder holds the offline Rust documentation.

Rust uses structs, enums, and methods instead of traditional classes. Bevy organizes the game with entities, components, resources, and systems. Level 1 animates a ring with a sine wave; Level 2 scales keyboard movement by speed and frame time; Level 3 uses iterative depth-first search to carve mazes, breadth-first search to locate a far reachable tile, and grid collision checks to keep the fire inside passages. Level 4 converts the mouse cursor to a world-space ray, aims it at a plane, moves a hook through flight and return states, and checks the hook's path against the moving ring before retrieving it. Level 5 grows a seeded maze sequence, opens additional routes, arcs a hook above walls, and teleports the fire to each caught ring's saved floor position. Level 6 keeps maze schemas separate from the active scene, moves rings only in the active maze, routes rings and the player through portals, samples 72 ring tiers with inverse-square weights, and persists ring transforms in a versioned binary record (a local file in the desktop game and browser storage in the IDE preview).
