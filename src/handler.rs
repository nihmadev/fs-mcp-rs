use crate::{app::App, tools};
use fs_mcp_rs::protocol::{Request as McpRequest, Response as McpResponse, negotiate_protocol};
use serde_json::{Value, json};

pub(crate) async fn handle_request(app: &App, request: McpRequest) -> Option<McpResponse> {
    let id = request.id.clone();
    if request.jsonrpc != "2.0" {
        return Some(McpResponse::error(id, -32600, "invalid JSON-RPC version"));
    }

    // Notifications have no id or method starting with notifications/
    if request.id.is_none() || request.method.starts_with("notifications/") {
        return None;
    }

    let _permit = match app.permits.acquire().await {
        Ok(permit) => permit,
        Err(_) => {
            return Some(McpResponse::error(id, -32603, "server shutting down"));
        }
    };

    let response = match request.method.as_str() {
        "initialize" => {
            let requested = request
                .params
                .get("protocolVersion")
                .and_then(Value::as_str);
            McpResponse::ok(
                id,
                json!({
                    "protocolVersion": negotiate_protocol(requested),
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "fs-mcp-rs", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": server_instructions()
                }),
            )
        }
        "ping" => McpResponse::ok(id, json!({})),
        "tools/list" => McpResponse::ok(id, json!({"tools": tools::tools()})),
        "tools/call" => match tools::call_tool(app, request.params).await {
            Ok(value) => McpResponse::ok(id, value),
            Err(error) => McpResponse::ok(id, tools::tool_error(error)),
        },
        _ => McpResponse::error(id, -32601, format!("unknown method: {}", request.method)),
    };

    Some(response)
}

fn server_instructions() -> String {
    let os = std::env::consts::OS;
    let family = std::env::consts::FAMILY;
    let arch = std::env::consts::ARCH;

    #[cfg(windows)]
    let command_guidance = "The terminal tools execute commands through cmd.exe. Use Windows cmd syntax and Windows paths (for example, C:\\path\\file). Do not use Unix-only commands or /bin/sh syntax unless a Unix compatibility environment is explicitly requested and verified.";
    #[cfg(not(windows))]
    let command_guidance = "The terminal tools execute commands through /bin/sh. Use POSIX shell syntax and paths unless another shell is explicitly requested and verified.";

    format!(
        "Host environment: operating system={os}, OS family={family}, architecture={arch}. {command_guidance} Filesystem access is restricted by operator configuration. The run_command tool can execute arbitrary terminal commands and may modify the system."
    )
}
