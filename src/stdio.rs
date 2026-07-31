//! STDIO transport loop for line-delimited JSON-RPC 2.0 requests.
//!
//! Reads input line-by-line from `stdin`, processes requests asynchronously using
//! [`handler::handle_request`], and writes newline-delimited JSON-RPC responses to `stdout`.

use crate::{app::App, handler};
use crate::protocol::Request as McpRequest;
use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, stdin, stdout};

/// Runs the STDIO transport loop, reading line-delimited JSON-RPC 2.0 messages from `stdin` and returning responses on `stdout`.
pub async fn serve(app: App) -> Result<()> {
    let stdin = stdin();
    let mut reader = BufReader::new(stdin).lines();
    let mut stdout = stdout();

    tracing::info!("fs-mcp-rs listening on STDIO transport");

    while let Some(line) = reader.next_line().await? {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let request: McpRequest = match serde_json::from_str(line) {
            Ok(req) => req,
            Err(err) => {
                tracing::warn!(error = %err, "Failed to parse JSON-RPC request from stdin");
                let err_response = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Parse error: {err}")
                    }
                });
                let mut out_bytes = serde_json::to_vec(&err_response)?;
                out_bytes.push(b'\n');
                stdout.write_all(&out_bytes).await?;
                stdout.flush().await?;
                continue;
            }
        };

        if let Some(response) = handler::handle_request(&app, request).await {
            let mut out_bytes = serde_json::to_vec(&response)?;
            out_bytes.push(b'\n');
            stdout.write_all(&out_bytes).await?;
            stdout.flush().await?;
        }
    }

    tracing::info!("STDIO transport stream closed");
    Ok(())
}
