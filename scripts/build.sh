#!/usr/bin/env sh
set -eu

cd "$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

echo "[1/5] Formatting check"
cargo fmt --all -- --check

echo "[2/5] Compiling workspace"
cargo check --workspace --all-targets

echo "[3/5] Running Clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "[4/5] Running tests"
cargo test --workspace

echo "[5/5] Checking release package"
cargo publish --dry-run -p fs-mcp-rs

echo "Release validation completed."
