#!/usr/bin/env bash
# Tier 0 + Tier 1 gate (PLAN.md testing ladder). Run from engine/.
# Tests run under cargo-nextest when it is installed: every test binary side by side, so the step takes about as long
# as its slowest test instead of the sum of all of them (card TSTX: 267 s -> 53 s on the same tree). Without nextest it
# falls back to `cargo test`, which gives the same counts. Install: `cargo install --locked cargo-nextest`.
# The test steps name their targets (card WEARC, SSD wear): left to its default, `cargo test` / nextest links every
# example as a full engine binary (31 of them) that no test runs. Examples stay compile-checked by clippy's
# --all-targets; `arena` is the one example with tests of its own (`test = true` in game/Cargo.toml) and is named.
set -euo pipefail
cd "$(dirname "$0")"
cargo fmt --all -- --check
# A new provisional value must name the measurement that replaces it.
python3 ../scripts/check_provisional.py --selftest
if base="$(git merge-base origin/main HEAD 2>/dev/null)"; then
  python3 ../scripts/check_provisional.py "$base"..HEAD
fi
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
TEST_TARGETS=(--lib --bins --tests --example arena)
if cargo nextest --version >/dev/null 2>&1; then
  cargo nextest run --workspace --profile ci "${TEST_TARGETS[@]}"
  cargo test --workspace --doc
else
  echo "ci.sh: cargo-nextest not installed - falling back to cargo test (slower, same counts)"
  cargo test --workspace "${TEST_TARGETS[@]}"
  cargo test --workspace --doc
fi
echo "CI green (Tier 0 + Tier 1)"
