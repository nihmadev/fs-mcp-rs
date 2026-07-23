@echo off
setlocal
cd /d "%~dp0\.."
if not exist benchmark-results mkdir benchmark-results
echo The server must already be listening on 127.0.0.1:8000.
cargo run --release -p benchmarks -- 1000 > benchmark-results\rust-ping.json
if errorlevel 1 exit /b 1
echo Saved benchmark-results\rust-ping.json