# Rust reading shelf

This folder combines searchable upstream source with offline HTML docs from the installed stable Rust toolchain. The HTML links are symlinks into rustup's toolchain docs, so they do not duplicate the roughly 90 MB of books and references.

## Start here

1. [The Rust Programming Language (The Book)](offline-book/index.html) — beginner-friendly, sequential course. Start at chapter 1; ownership is chapter 4.
2. [Rust by Example](offline-rust-by-example/index.html) — short runnable examples; useful to browse by topic.
3. [The Rust Reference](offline-reference/index.html) — precise language details; use after learning the basics.
4. [Standard library documentation](offline-std/index.html) — API reference for `std`, including collections, strings, files, and I/O.
5. [Rust glossary](GLOSSARY.md) — concise definitions and C-oriented notes.

Online source repositories (also checked out locally as searchable Markdown/source):

- [Book source](book-source/src/SUMMARY.md) — upstream: https://github.com/rust-lang/book
- [Rust by Example source](rust-by-example-source/src/SUMMARY.md) — upstream: https://github.com/rust-lang/rust-by-example
- [Reference source](reference-source/src/SUMMARY.md) — upstream: https://github.com/rust-lang/reference

Open generated offline docs in a browser with `open /Users/nulll/Documents/RUST/reading/offline-book/index.html` (replace `offline-book` with another link above). In a terminal, run `rustup doc --book`, `rustup doc --std`, or `rustup doc --path`.

## Version and refresh

The offline HTML follows the installed toolchain. Check it with `rustc --version`; refresh using `rustup update`. The checked-out source repositories are snapshots, each with its own Git history metadata; refresh a snapshot from its folder using `git pull --ff-only`. The three repositories were fetched on 2026-10-30. Consult the offline docs for what this installed toolchain documents; online links may show newer content.
