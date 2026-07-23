# fs-mcp-rs

A fast, configurable filesystem server for the Model Context Protocol (MCP), written in Rust.

`fs-mcp-rs` gives an MCP client a small set of filesystem tools while keeping access inside directories selected by the operator. It has no implicit filesystem root, no built-in personal path, and no writable default. The server starts only when you provide a configuration file.

## Why one crate?

The public distribution is one crate and one executable, both named `fs-mcp-rs`. The protocol, policy, filesystem, search, and settings components remain separate Rust modules inside that crate. This keeps installation simple:

```console
cargo install fs-mcp-rs
```

Users do not need to install or understand a graph of internal crates. The benchmark runner remains a development-only workspace member and is not included in the crates.io package.

## Features

- MCP over JSON-RPC HTTP POST.
- Explicit allow-list of one or more filesystem roots.
- Read-only mode for safe deployments.
- Optional write, move, edit, directory creation, and removal operations.
- Bounded reads, writes, search results, and concurrency.
- Literal and regular-expression content search.
- `.gitignore` support and configurable hidden-file handling.
- Symlink traversal disabled by default.
- Atomic file replacement and BLAKE3 hashes.
- Optional arbitrary terminal command execution with time and output limits.
- Persistent terminal sessions with incremental output cursors, long polling, stdin, process-tree termination, retention, and bounded buffers.
- No telemetry and no automatic network discovery.

## Requirements

- Rust 1.85 or newer when building from source.
- An MCP client that can connect to a Streamable HTTP/JSON-RPC endpoint.
- A configuration file created and reviewed by the person running the server.

## Installation

### From crates.io

```console
cargo install fs-mcp-rs
fs-mcp-rs --version
```

### From source

```console
git clone YOUR_REPOSITORY_URL
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

#### `[filesystem]`

- `roots`: non-empty list of allowed directories. Every requested path must resolve inside one of them.
- `read_only`: when `true`, all mutating tools are rejected.
- `max_read_bytes`: largest permitted `read_file` response.
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

Check that it is alive:

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

## Configure an MCP client

Start `fs-mcp-rs` as a long-running service, then add an HTTP MCP server in your client using the `/mcp` URL. Product configuration formats differ, so use the client documentation for the exact field names.

Generic example:

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

Do not expose the endpoint to an untrusted network. The server controls filesystem scope, but it does not replace host authentication, TLS, a firewall, or an authenticated reverse proxy.

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

Set `RUST_LOG` to control structured logs:

```console
RUST_LOG=info fs-mcp-rs --config ./fs-mcp-rs.toml
```

Use `debug` only while troubleshooting because paths may appear in diagnostic output.

## Development and release validation

Run the complete local gate before opening a release:

```console
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo publish --dry-run -p fs-mcp-rs
```

On Windows, `scripts\build.cmd` runs the same checks. A real crates.io release should also pass CI on Windows, Linux, and macOS and should be tested against at least one actual MCP client.

Benchmark methodology is documented in `BENCHMARKS.md`. Performance claims should be accompanied by raw results, machine details, configuration, and commit hashes.

## License

Copyright (C) 2026 nihmadev (lolz@nihmadev.fun).

Licensed under **GNU GPL version 3 or any later version** (`GPL-3.0-or-later`). See `LICENSE` and the canonical license text at <https://www.gnu.org/licenses/gpl-3.0.html>.
