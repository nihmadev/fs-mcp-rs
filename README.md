# fs-mcp-rs

High-performance filesystem MCP server written in Rust.

## Workspace

- `protocol` — JSON-RPC and MCP wire models.
- `settings` — CLI and TOML configuration.
- `security` — allowed-root and write policies.
- `filesystem` — bounded reads and atomic writes.
- `search` — file-name and content search.
- `server` — HTTP MCP server.
- `benchmarks` — black-box latency runner.

All crate names are single words. The default server address is `127.0.0.1:8000`.

## Run

```bat
cargo run --release -p server -- --config configs\default.toml
```

Health endpoint:

```text
GET http://127.0.0.1:8000/health
```

MCP endpoint:

```text
POST http://127.0.0.1:8000/mcp
```"# fs-mcp-rs" 
"# fs-mcp-rs" 
