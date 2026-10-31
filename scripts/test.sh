#!/usr/bin/env bash
# Runs formatting, checks, clippy, and tests for every Rust crate in this
# repo, reporting all results instead of stopping at the first failure.
set -uo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
crates=("$root_dir" "$root_dir/ringen-van-patronage-origins")

failures=0

run_step() {
    local crate="$1" label="$2"
    shift 2
    echo "==> $(basename "$crate"): $label"
    if ! (cd "$crate" && "$@"); then
        echo "    FAILED: $label"
        failures=$((failures + 1))
    fi
}

for crate in "${crates[@]}"; do
    run_step "$crate" "cargo fmt --check" cargo fmt --all -- --check
    run_step "$crate" "cargo check" cargo check
    run_step "$crate" "cargo clippy" cargo clippy -- -D warnings
    run_step "$crate" "cargo test" cargo test
done

run_step "$root_dir" "studio export and page smoke test" ./scripts/check-studio.sh

echo
if [[ "$failures" -eq 0 ]]; then
    echo "All crates passed."
else
    echo "$failures check(s) failed."
fi
exit "$failures"
