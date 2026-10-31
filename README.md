# Rust learning repository

A hands-on start with Rust. Rust and Cargo were installed with `rustup` (stable toolchain).

## Quick start (any OS)

Prerequisites: [Rust via rustup](https://rustup.rs). On Linux, Bevy also needs system libraries (Debian/Ubuntu: `sudo apt-get install pkg-config g++ libasound2-dev libudev-dev libx11-dev libwayland-dev libxkbcommon-dev`). The Learning Studio script below installs the WebAssembly target and `wasm-bindgen-cli` for you.

```sh
./scripts/test.sh     # fmt, check, clippy and tests for every crate
./scripts/studio.sh   # builds the game for the browser and serves the IDE at http://127.0.0.1:4173
```

The offline reading shelf in `reading/` is optional and not part of the repository; the studio works without it and links to the online docs instead.

## Deployment

`.github/workflows/ci.yml` runs `./scripts/test.sh` on every push and pull request. When CI passes on `main`, `.github/workflows/deploy.yml` builds the game for the browser and publishes it to GitHub Pages at `https://<user>.github.io/<repo>/`. The published page is the playable game only; the editing Learning Studio stays a local tool (`./scripts/studio.sh`) because it runs `cargo` on request.

One-time setup: in the repository's **Settings → Pages**, set **Source** to **GitHub Actions**. You can also start a deploy by hand from the Actions tab (**Deploy → Run workflow**).

## Run the first program

```sh
cargo run
```

Cargo compiles the package and runs it. The entry point is `src/main.rs`.

## First ideas in the code

- `let` creates an immutable binding; use `let mut` when reassignment is needed.
- Types can often be inferred, but function parameters and return values are explicit.
- `println!` is a macro (the `!` marks it), with checked formatting placeholders.
- `if` and `match` can produce values.
- The final expression in a block is its value; a semicolon turns it into a statement.
- `cargo run` builds and executes; `cargo check` checks quickly without producing a final executable.

Run the ownership example with `cargo run --example ownership`.

## Rust and C: useful first differences

| Topic | C | Rust |
|---|---|---|
| Memory | Usually managed manually with `malloc`/`free`; mistakes can cause leaks or use-after-free | Ownership and borrowing let the compiler check many lifetime and aliasing errors without a garbage collector |
| Mutability | Local variables are mutable unless declared otherwise | Bindings are immutable unless marked `mut` |
| Types | Types are declared, with implicit conversions in some contexts | Strong static types; inference helps with local types, and conversions are generally explicit |
| Errors | Often reported through return codes, `errno`, or conventions | `Result<T, E>` represents recoverable errors; `Option<T>` represents a possibly absent value |
| Build | Compiler and build system vary | Cargo comes with Rustup and handles builds, dependencies, and project layout |
| Runtime | C has no built-in bounds checks for ordinary array access | Safe Rust checks array/slice bounds at runtime; many other safety constraints are checked at compile time |

Rust still supports low-level systems work and can call C through `unsafe` code and FFI. `unsafe` makes the programmer responsible for a small set of guarantees; it does not turn off all compiler checks.

## A good learning order

1. Bindings, scalar types, functions, control flow, and collections.
2. Ownership, borrowing, and slices (`examples/ownership.rs` is the first taste).
3. Enums, `Option`, `Result`, and error handling.
4. Structs, traits, modules, and Cargo dependencies.

The official [Rust Book](https://doc.rust-lang.org/book/) is the main reference. The installation guide is at [rust-lang.org/learn/get-started](https://rust-lang.org/learn/get-started/).

## Local reading shelf

See [`reading/README.md`](reading/README.md) for offline Rust books and searchable sources, and [`reading/GLOSSARY.md`](reading/GLOSSARY.md) for terminology and C comparisons. Repository-level agent notes are in `AGENTS.md`.

## Interactive game lessons

The project in [`ringen-van-patronage-origins`](ringen-van-patronage-origins/README.md) pairs playable Bevy scenes with a local browser IDE. The IDE embeds the Bevy menu and levels, lets you edit each level's Rust source, and saves then recompiles the WebAssembly preview. Level 4, **The Hook**, practices mouse-to-world aiming, projectile state, path collision, retrieval, and a repeating collect-and-respawn loop. Run `cargo run` in the game folder and open the studio from its level menu, or start only the IDE with `cargo run -- --viewer`.
