use crate::{app::App, oauth, tools};
use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response as HttpResponse},
    routing::{get, post},
};
use fs_mcp_rs::protocol::{Request as McpRequest, Response as McpResponse, negotiate_protocol};
use serde_json::{Value, json};
use std::path::Path;
use tokio::net::TcpListener;

pub(crate) async fn serve(app: App, config_path: &Path) -> Result<()> {
    let address = (app.settings.server.host, app.settings.server.port);
    let mut router = Router::new()
        .route("/", get(root_get))
        .route("/health", get(|| async { "ok" }))
        .route("/mcp", get(mcp_get).post(handle));

    if app.settings.oauth.enabled {
        router = router
            .route(
                "/.well-known/oauth-authorization-server",
                get(oauth::oauth_authorization_server_metadata),
            )
            .route(
                "/.well-known/openid-configuration",
                get(oauth::openid_configuration),
            )
            .route(
                "/.well-known/oauth-protected-resource",
                get(oauth::oauth_protected_resource_metadata),
            )
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                get(oauth::oauth_protected_resource_metadata),
            )
            .route("/register", post(oauth::register_client))
            .route(
                "/authorize",
                get(oauth::authorize_get).post(oauth::authorize_post),
            )
            .route("/token", post(oauth::token_endpoint))
            .route("/userinfo", get(oauth::userinfo_endpoint))
            .route("/.well-known/jwks.json", get(oauth::jwks_endpoint));
    }

    let router = router
        .layer(middleware::from_fn(cors_middleware))
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

async fn cors_middleware(req: Request, next: Next) -> HttpResponse {
    if req.method() == Method::OPTIONS {
        let mut res = HttpResponse::builder()
            .status(StatusCode::OK)
            .body(axum::body::Body::empty())
            .unwrap();
        let headers = res.headers_mut();
        headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
        headers.insert(
            "access-control-allow-methods",
            HeaderValue::from_static("GET, POST, OPTIONS, PUT, DELETE"),
        );
        headers.insert(
            "access-control-allow-headers",
            HeaderValue::from_static("*"),
        );
        headers.insert("access-control-max-age", HeaderValue::from_static("86400"));
        return res;
    }

    let mut res = next.run(req).await;
    let headers = res.headers_mut();
    headers.insert("access-control-allow-origin", HeaderValue::from_static("*"));
    headers.insert(
        "access-control-allow-methods",
        HeaderValue::from_static("GET, POST, OPTIONS, PUT, DELETE"),
    );
    headers.insert(
        "access-control-allow-headers",
        HeaderValue::from_static("*"),
    );
    res
}

async fn root_get() -> impl IntoResponse {
    Json(json!({
        "name": "fs-mcp-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "ok",
        "mcp": "/mcp"
    }))
}

async fn mcp_get() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "server": "fs-mcp-rs",
        "version": env!("CARGO_PKG_VERSION"),
        "transport": "http-post",
        "endpoint": "/mcp"
    }))
}

/// Validates and dispatches one JSON-RPC request.
async fn handle(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<McpRequest>,
) -> HttpResponse {
    if app.settings.oauth.require_auth {
        let auth_valid = headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|t| app.oauth.validate_token(t))
            .unwrap_or(false);

        if !auth_valid {
            return (
                StatusCode::UNAUTHORIZED,
                [("www-authenticate", "Bearer realm=\"mcp\"")],
                Json(json!({"error": "unauthorized", "error_description": "valid Bearer token required"})),
            )
                .into_response();
        }
    }
    let id = request.id.clone();
    if request.jsonrpc != "2.0" {
        return Json(McpResponse::error(id, -32600, "invalid JSON-RPC version")).into_response();
    }
    if request.id.is_none() {
        return StatusCode::ACCEPTED.into_response();
    }
    let _permit = match app.permits.acquire().await {
        Ok(permit) => permit,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(McpResponse::error(id, -32603, "server shutting down")),
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
        "tools/call" => match tools::call_tool(&app, request.params).await {
            Ok(value) => McpResponse::ok(id, value),
            Err(error) => McpResponse::ok(id, tools::tool_error(error)),
        },
        _ => McpResponse::error(id, -32601, format!("unknown method: {}", request.method)),
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
