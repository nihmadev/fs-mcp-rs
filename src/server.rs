use crate::{app::App, tools};
use anyhow::Result;
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response as HttpResponse},
    routing::get,
};
use fs_mcp_rs::protocol::{Request, Response, negotiate_protocol};
use serde_json::{Value, json};
use std::path::Path;
use tokio::net::TcpListener;

pub(crate) async fn serve(app: App, config_path: &Path) -> Result<()> {
    let address = (app.settings.server.host, app.settings.server.port);
    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/mcp", get(mcp_get).post(handle))
        .with_state(app);
    let listener = TcpListener::bind(address).await?;
    tracing::info!(?address, config = %config_path.display(), "filesystem MCP listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn mcp_get() -> impl IntoResponse {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [("allow", "POST")],
        "MCP endpoint accepts JSON-RPC POST requests",
    )
}

/// Validates and dispatches one JSON-RPC request.
async fn handle(State(app): State<App>, Json(request): Json<Request>) -> HttpResponse {
    let id = request.id.clone();
    if request.jsonrpc != "2.0" {
        return Json(Response::error(id, -32600, "invalid JSON-RPC version")).into_response();
    }
    if request.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let _permit = match app.permits.acquire().await {
        Ok(permit) => permit,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(Response::error(id, -32603, "server shutting down")),
            )
                .into_response();
        }
    };
    let response = match request.method.as_str() {
        "initialize" => {
            let requested = request
                .params
                .get("protocolVersion")
                .and_then(Value::as_str);
            Response::ok(
                id,
                json!({
                    "protocolVersion": negotiate_protocol(requested),
                    "capabilities": {"tools": {"listChanged": false}},
                    "serverInfo": {"name": "fs-mcp-rs", "version": env!("CARGO_PKG_VERSION")},
                    "instructions": server_instructions()
                }),
            )
        }
        "ping" => Response::ok(id, json!({})),
        "tools/list" => Response::ok(id, json!({"tools": tools::tools()})),
        "tools/call" => match tools::call_tool(&app, request.params).await {
            Ok(value) => Response::ok(id, value),
            Err(error) => Response::ok(id, tools::tool_error(error)),
        },
        _ => Response::error(id, -32601, format!("unknown method: {}", request.method)),
    };
    Json(response).into_response()
}

/// Builds host-specific guidance returned during the MCP initialize handshake.
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
