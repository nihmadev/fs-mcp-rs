<!-- mcp-name: io.github.nihmadev/fs-mcp-rs -->
![fs-mcp-rs](assets/preview.png)
[![Crates.io](https://img.shields.io/crates/v/fs-mcp-rs?logo=rust&label=crates.io)](https://crates.io/crates/fs-mcp-rs)
[![docs.rs](https://img.shields.io/docsrs/fs-mcp-rs?logo=docs.rs&label=docs.rs)](https://docs.rs/fs-mcp-rs)
[![License](https://img.shields.io/crates/l/fs-mcp-rs?label=license)](LICENSE)
[![Rust 1.85+](https://img.shields.io/badge/rust-1.85%2B-orange?logo=rust)](https://www.rust-lang.org)
[![MCP](https://img.shields.io/badge/MCP-Streamable%20HTTP-6f42c1)](https://modelcontextprotocol.io)

[![English](https://img.shields.io/badge/lang-English-lightgrey)](README.md)
[![Русский](https://img.shields.io/badge/lang-Русский-blue)](README_RU.md)

A fast, configurable filesystem server for the Model Context Protocol (MCP), written in Rust.

`fs-mcp-rs` gives an MCP client a small set of filesystem tools while keeping access inside directories selected by the operator. It has no implicit filesystem root, no built-in personal path, and no writable default. The server starts only when you provide a configuration file.

## Why one crate?

The public distribution is one crate and one executable, both named `fs-mcp-rs`. The protocol, policy, filesystem, search, and settings components remain separate Rust modules inside that crate. This keeps installation simple:

```console
cargo install fs-mcp-rs
```

Users do not need to install or understand a graph of internal crates. The benchmark runner remains a development-only workspace member and is not included in the crates.io package.

## Features

- MCP over JSON-RPC HTTP POST and line-delimited STDIO (stdin/stdout).
- Zero-install execution via `npx fs-mcp-rs`.
- Explicit allow-list of one or more filesystem roots (via CLI positional arguments or TOML config).
- Read-only mode for safe deployments.
- Optional write, move, edit, directory creation, and removal operations.
- Bounded reads, writes, search results, and concurrency.
- Literal and regular-expression content search.
- `.gitignore` support and configurable hidden-file handling.
- Symlink traversal disabled by default.
- Atomic file replacement and BLAKE3 hashes.
- Optional arbitrary terminal command execution with time and output limits.
- Persistent terminal sessions with incremental output cursors, long polling, stdin, process-tree termination, retention, and bounded buffers.
- Concise tool execution logging (`[OK]` / `[WARN]`).
- No telemetry and no automatic network discovery.

## Requirements

- Node.js (v18+) for `npx` execution, or Rust 1.85+ when building from source.
- An MCP client that can connect via STDIO JSON-RPC or Streamable HTTP/JSON-RPC endpoint.
- A configuration file or explicit root paths passed to the server.

## Installation

### Via npx (Zero Installation)

The easiest way to run `fs-mcp-rs` with any MCP client (such as Claude Desktop or Cursor) is using `npx`:

```console
npx fs-mcp-rs /path/to/project
```

The npm package automatically downloads the appropriate precompiled binary for your operating system and architecture (Windows x64, Linux x64, macOS x64/arm64).

### From crates.io

```console
cargo install fs-mcp-rs
fs-mcp-rs --version
```

### From source

```console
git clone https://github.com/nihmadev/fs-mcp-rs.git
cd fs-mcp-rs
cargo build --release
```

The executable will be at `target/release/fs-mcp-rs` on Unix-like systems or `target\release\fs-mcp-rs.exe` on Windows.

## Create a configuration

Copy `configs/example.toml` somewhere appropriate and edit it. The server intentionally has no default config path:

```console
cp configs/example.toml fs-mcp-rs.toml
```

Windows PowerShell:

```powershell
Copy-Item configs\example.toml fs-mcp-rs.toml
```

Minimal read-only configuration:

```toml
[server]
host = "127.0.0.1"
port = 8000
max_concurrency = 32
max_io_concurrency = 16

[filesystem]
roots = ["/home/alice/projects"]
read_only = true
max_read_bytes = 8388608
max_write_bytes = 8388608
follow_links = false

[search]
max_results = 1000
max_concurrency = 4
worker_threads = 4
regex_cache_capacity = 64
include_hidden = false
respect_gitignore = true

# WARNING: enables arbitrary commands with the server process permissions.
[terminal]
enabled = true
max_concurrency = 2
default_timeout_ms = 30000
max_timeout_ms = 300000
max_output_bytes = 4194304
```

Windows roots use escaped backslashes or forward slashes:

```toml
roots = ["C:\\Users\\Alice\\Projects", "D:/Shared/Documentation"]
```

Relative roots are resolved relative to the directory containing the configuration file, not relative to an arbitrary process working directory.

### Configuration reference

#### `[server]`

- `host`: IP address to bind. Keep `127.0.0.1` unless remote clients genuinely need access.
- `port`: TCP port for `/health` and `/mcp`.
- `max_concurrency`: maximum number of MCP requests processed at once.
- `max_io_concurrency`: maximum number of simultaneous non-search filesystem jobs.
- `log_tools`: whether concise tool execution logging (`[OK]` / `[WARN]`) is printed (default: `true`).

#### `[filesystem]`

- `roots`: non-empty list of allowed directories. Every requested path must resolve inside one of them.
- `read_only`: when `true`, all mutating tools are rejected.
- `max_write_bytes`: largest permitted `write_file` or resulting `edit_text` content.
- `follow_links`: whether paths containing symbolic links may be followed. Leave this `false` unless you have reviewed the consequences.

#### `[search]`

- `max_results`: maximum matches returned by one search.
- `max_concurrency`: maximum simultaneous search jobs.
- `worker_threads`: filesystem traversal threads used by each search.
- `regex_cache_capacity`: number of compiled search patterns cached in memory; `0` disables the cache.
- `include_hidden`: include hidden files and directories.
- `respect_gitignore`: apply Git ignore files while walking directories.

#### `[terminal]`

- `enabled`: expose command execution. When disabled, `run_command` returns a tool error.
- `max_concurrency`: maximum terminal commands running simultaneously.
- `default_timeout_ms`: timeout used when a call does not provide one.
- `max_timeout_ms`: largest timeout a call may request.
- `max_output_bytes`: maximum combined stdout/stderr bytes retained per terminal session. Old output is evicted and reported as dropped/truncated.
- `max_read_bytes`: maximum output bytes returned by one `terminal_read` call.
- `max_wait_ms`: maximum long-poll duration for one `terminal_read` call.
- `session_retention_ms`: how long completed sessions and their output remain readable before automatic cleanup.

`run_command` executes arbitrary commands through `cmd.exe` on Windows and `/bin/sh` on GNU/Linux and macOS. It is advertised with destructive and open-world MCP annotations so compatible providers can show a warning. Enabling it gives commands all permissions of the operating-system account running the server; filesystem roots and read-only mode do not sandbox terminal commands.

Unknown fields and zero-valued safety limits are rejected. This catches misspelled settings instead of silently ignoring them.

## Start the server

### Positional root paths (Quickstart)

Start directly by specifying allowed filesystem directories on the command line:

```console
fs-mcp-rs /home/alice/project1 /home/alice/project2
```

If no configuration file or positional roots are passed, `fs-mcp-rs` uses default settings scoped to the current working directory.

### With a configuration file

Pass the configuration explicitly:

```console
fs-mcp-rs --config /absolute/path/to/fs-mcp-rs.toml
```

Or use the environment variable:

```console
FS_MCP_CONFIG=/absolute/path/to/fs-mcp-rs.toml fs-mcp-rs
```

PowerShell:

```powershell
$env:FS_MCP_CONFIG = "C:\config\fs-mcp-rs.toml"
fs-mcp-rs
```

### Transport Modes (STDIO vs HTTP)

- **STDIO Mode**: Activated automatically when stdout/stdin are non-interactive pipes (e.g. when spawned as an MCP subprocess) or when passing `--stdio`. Structured logs are written to `stderr` to keep `stdout` clean for line-delimited JSON-RPC messages.
- **HTTP Mode**: Activated by default during interactive execution. Listens on the configured host/port for `/health` and `/mcp` endpoints.

Check HTTP health:

```console
curl http://127.0.0.1:8000/health
```

Expected response:

```text
ok
```

The MCP endpoint is:

```text
http://127.0.0.1:8000/mcp
```

### Start the server and a tunnel together

The `run` subcommand always starts the MCP server in HTTP mode and can manage an ngrok, Cloudflare Quick Tunnel, or zrok public share in the same process lifecycle:

```console
fs-mcp-rs run --ngrok /path/to/allowed/directory
fs-mcp-rs run --ngrok --config ./fs-mcp-rs.toml
fs-mcp-rs run --cloudflared /path/to/allowed/directory
fs-mcp-rs run --zrok /path/to/allowed/directory
```

The providers are mutually exclusive. On Windows, the tunnel opens in a new console by default. Use `--tunnel-window hidden` to avoid a separate visible console while keeping the generated URL in the current terminal, or `--tunnel-window inherit` to attach normally. On Linux and macOS, `new` inherits the current terminal because creating a terminal window is desktop-specific. Each provider accepts repeatable extra arguments and an explicit executable path:

```console
fs-mcp-rs run --ngrok --tunnel-window hidden --ngrok-arg=--region=eu
fs-mcp-rs run --ngrok --ngrok-bin /opt/ngrok/ngrok
fs-mcp-rs run --cloudflared --cloudflared-arg=--no-autoupdate
fs-mcp-rs run --zrok --zrok-bin /opt/zrok/zrok
```

Stopping the MCP server also stops the tunnel process. Existing invocations such as `fs-mcp-rs [PATHS]`, `fs-mcp-rs --config FILE`, and `fs-mcp-rs serve` are unchanged. The earlier `--ngrok-window` spelling remains an alias for `--tunnel-window`.

## Expose the server through a secure tunnel

For testing with a remote MCP client, keep `fs-mcp-rs` bound to loopback and expose it through a tunnel. Start the server first:

```console
fs-mcp-rs --config ./fs-mcp-rs.toml
```

Verify the local endpoint before creating a tunnel:

```console
curl http://127.0.0.1:8000/health
```

> [!CAUTION]
> A public tunnel URL gives remote clients a path to the tools and filesystem roots allowed by your configuration. Start with `read_only = true`, grant only narrowly scoped roots, never expose secrets, and stop the tunnel when it is no longer needed. Quick tunnels are intended for development—not permanent production hosting.

### Cloudflare Tunnel

Install [`cloudflared`](https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/), then create a temporary Quick Tunnel:

```console
cloudflared tunnel --url http://127.0.0.1:8000
```

`cloudflared` prints a temporary URL similar to `https://random-name.trycloudflare.com`. Use the MCP endpoint with `/mcp` appended:

```text
https://random-name.trycloudflare.com/mcp
```

For a stable hostname, create a named Cloudflare Tunnel, map a DNS hostname to it, and route that hostname to `http://127.0.0.1:8000`. Protect long-lived deployments with Cloudflare Access or another authentication layer.

### ngrok

Install [`ngrok`](https://ngrok.com/download), authenticate its CLI if required by your account, and start an HTTP tunnel:

```console
ngrok http 8000
```

Use the HTTPS forwarding URL shown by ngrok and append `/mcp`:

```text
https://example.ngrok-free.app/mcp
```

The local inspection UI is normally available at `http://127.0.0.1:4040`. It can expose request details, so keep it local and do not share it publicly.

### Connect a remote MCP client

Use the public HTTPS URL as an HTTP/Streamable HTTP MCP server:

```json
{
  "mcpServers": {
    "filesystem": {
      "transport": "http",
      "url": "https://your-public-host.example/mcp"
    }
  }
}
```

Test both endpoints after the tunnel starts:

```console
curl https://your-public-host.example/health
curl -i https://your-public-host.example/mcp
```

A non-`200` response from a plain `GET /mcp` can be expected because MCP communication uses protocol requests; the important checks are that `/health` is reachable and that the MCP client can initialize a session.

### Production hosting

For a persistent deployment, prefer a private network/VPN or a named authenticated tunnel. Run `fs-mcp-rs` under a dedicated low-privilege account and a service manager, keep it bound to loopback, terminate TLS at the tunnel or reverse proxy, restrict client access, pin the configuration path, and monitor logs. Do not rely on an unprotected public URL as the only security boundary.

## Configure an MCP client

`fs-mcp-rs` can be configured as a local STDIO subprocess or a remote/local HTTP service in MCP clients like Claude Desktop, Cursor, VS Code, or Windsurf.

### Option A: STDIO Transport with npx (Recommended for Claude / Cursor)

No pre-installation required:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "fs-mcp-rs", "/path/to/allowed/directory"]
    }
  }
}
```

Or using an installed binary with explicit `--stdio`:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "fs-mcp-rs",
      "args": ["--stdio", "/path/to/allowed/directory"]
    }
  }
}
```

### Option B: HTTP Transport

Start `fs-mcp-rs` as a long-running service, then point your client to the `/mcp` HTTP endpoint:

```json
{
  "mcpServers": {
    "filesystem": {
      "transport": "http",
      "url": "http://127.0.0.1:8000/mcp"
    }
  }
}
```

Do not expose HTTP endpoints to untrusted networks without authentication or TLS.

## Available tools

- `list_directory`: list direct children of a directory.
- `read_file`: read a UTF-8 byte range with explicit offset and length.
- `write_file`: atomically create or replace a UTF-8 file. Set `createParents: true` for a new nested path; the server safely creates missing parent directories after validating the allowed root. The default is `false`.
- `search_files`: find paths whose file name contains a string.
- `search_content`: case-insensitive literal or regex search in text files.
- `get_capabilities`: report sanitized effective limits, roots, protocol versions, and advertised tools.
- `list_tree`: return a bounded, deterministic, paginated flat directory tree.
- `apply_patch`: atomically apply a validated single-file unified text diff; `dryRun` never mutates.
- `file_info`: return rich metadata and an optional bounded-streaming BLAKE3 digest.
- `create_directory`: create one directory; parents must already exist.
- `remove`: remove one file or an empty directory.
- `hash_file`: calculate a BLAKE3 file hash.
- `move`: move without overwriting an existing destination.
- `edit_text`: replace an exact expected number of text matches atomically.
- `terminal_start`: start a persistent command and immediately return a session ID.
- `terminal_read`: incrementally read stdout/stderr events using a byte cursor; optionally long-poll for new output.
- `terminal_write`: write UTF-8 data to a running session's stdin.
- `terminal_close_stdin`: send EOF to a running session.
- `terminal_kill`: terminate the session's complete process tree.
- `terminal_close`: explicitly remove a completed session and retained output.
- `run_command`: execute an arbitrary shell command with optional working directory and timeout, waiting for completion for backward compatibility.

Mutating tools remain visible in read-only mode but return a clear tool error. This lets one client configuration work with both read-only and writable deployments.

## Security model

1. Every readable path is canonicalized and checked against the configured roots.
2. Every write destination is checked through its existing parent directory.
3. Symlink traversal is rejected by default.
4. Reads, writes, result counts, and worker concurrency are bounded.
5. Writes use a temporary file in the destination directory before persistence.
6. `edit_text` requires an expected replacement count, preventing accidental broad edits.

Recommended production posture:

- Start with `read_only = true`.
- Grant the smallest possible roots; do not grant a whole home directory or drive without a specific reason.
- Bind to loopback.
- Run under a dedicated low-privilege operating-system account.
- Keep hidden files excluded unless needed.
- If remote access is required, put authentication and TLS in front of the server.
- Back up writable roots and test restore procedures.

Filesystem checks can still be affected by operating-system races if another process can replace path components during an operation. Do not share writable roots with untrusted local users.

## New tool examples

Discover effective constraints without probing paths:

```json
{"name":"get_capabilities","arguments":{}}
```

List a deterministic tree page and continue with the returned cursor:

```json
{"name":"list_tree","arguments":{"path":".","depth":2,"include":["**/*.rs"],"exclude":["target/**"],"maxEntries":100,"cursor":null}}
```

Validate a one-file patch without mutating the target:

```json
{"name":"apply_patch","arguments":{"path":"README.md","patch":"--- a/README.md\n+++ b/README.md\n@@ -1 +1 @@\n-old\n+new\n","expectedBlake3":null,"dryRun":true}}
```

Request rich metadata and a bounded streaming hash:

```json
{"name":"file_info","arguments":{"path":"README.md","includeHash":true}}
```

`apply_patch` rejects binary, multi-file, create/delete, and rename patches. All hunks apply atomically or none do. It preserves the target's existing LF or CRLF convention and final-newline state. `dryRun` performs complete validation but never writes, including in read-only mode.

## Configuration reference for tree and patch limits

The following optional `[filesystem]` fields have validated defaults, so existing configuration files continue to load:

| Field | Default | Constraint |
|---|---:|---|
| `tree_max_depth` | `8` | Positive maximum recursive depth |
| `tree_max_entries` | `1000` | Positive maximum entries returned per page |
| `tree_max_warnings` | `32` | Positive maximum skipped-entry warnings |
| `patch_max_bytes` | `1048576` | Positive maximum unified-diff input bytes |
| `patch_preview_bytes` | `16384` | Positive and no greater than `patch_max_bytes` |

`list_tree` uses the search hidden-file and Git-ignore settings. Cursors are opaque and bound to the canonical root, depth, and filters. Every requested path still passes through `Policy`; these tools do not weaken root isolation, read-only enforcement, or symlink restrictions.

## Migration from stat

`file_info` replaces the former public `stat` tool. `stat` is no longer advertised or dispatched. Clients should migrate to `file_info`; set `includeHash` only when a BLAKE3 digest is needed.

## Logging

### Structured tracing logs

All structured tracing logs are printed to `stderr` to ensure `stdout` remains clean for JSON-RPC 2.0 frames when running in STDIO mode. Set `RUST_LOG` to control verbosity:

```console
RUST_LOG=info fs-mcp-rs --config ./fs-mcp-rs.toml
```

Use `debug` only while troubleshooting because paths may appear in diagnostic output.

### Tool Call Execution Logging

When `log_tools = true` (the default), `fs-mcp-rs` logs a single line summarizing each tool invocation:

- **Success**: `[OK] read_file src/main.rs (3 ms)`
- **Warning/Error**: `[WARN] read_file outside/allowed/path - OUTSIDE_ALLOWED_ROOT: path outside allowed roots (0 ms)`

Tool logging can be disabled in your TOML config under `[server]`: `log_tools = false`.

## Development and release validation

Run the complete local gate before opening a release:

```console
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo publish --dry-run -p fs-mcp-rs
```

On Windows, `scripts\build.cmd` runs the same checks. Continuous integration (`.github/workflows/ci.yml`) runs formatting, linting, and tests on Ubuntu, macOS, and Windows runners. GitHub Actions release workflow (`.github/workflows/release.yml`) builds precompiled binary archives for all major platforms and releases them alongside the npm package.

Benchmark methodology is documented in `BENCHMARKS.md`. Performance claims should be accompanied by raw results, machine details, configuration, and commit hashes.

## License

Copyright (C) 2026 nihmadev (lolz@nihmadev.fun).

Licensed under **GNU GPL version 3 or any later version** (`GPL-3.0-or-later`). See `LICENSE` and the canonical license text at <https://www.gnu.org/licenses/gpl-3.0.html>.
