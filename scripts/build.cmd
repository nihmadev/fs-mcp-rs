@echo off
setlocal
cd /d "%~dp0\.."
echo [1/5] Formatting check
cargo fmt --all -- --check || exit /b 1
echo [2/5] Compiling workspace
cargo check --workspace --all-targets || exit /b 1
echo [3/5] Running Clippy
cargo clippy --workspace --all-targets -- -D warnings || exit /b 1
echo [4/5] Running tests
cargo test --workspace || exit /b 1
echo [5/5] Checking release package
cargo publish --dry-run -p fs-mcp-rs || exit /b 1
echo Release validation completed.
