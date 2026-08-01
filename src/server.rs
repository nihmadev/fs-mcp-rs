//! HTTP transport server using Axum framework.
//!
//! Exposes `/mcp` HTTP POST endpoint, `/health` check, CORS middleware, and optional OAuth 2.0 / OIDC endpoints.

use crate::{app::App, handler, oauth};
use crate::protocol::{
    LATEST_PROTOCOL_VERSION, Request as McpRequest, SUPPORTED_PROTOCOL_VERSIONS,
    negotiate_protocol,
};
use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response as HttpResponse},
    routing::{get, post},
};
use serde_json::json;
use std::path::Path;
use tokio::net::TcpListener;

/// Starts the HTTP server listener with configured routes and graceful shutdown handler.
pub async fn serve(app: App, config_path: &Path) -> Result<()> {
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
    // Streamable HTTP GET is reserved for an SSE stream. This server has no
    // server-initiated messages, so explicitly advertise that it is unsupported
    // instead of returning a non-MCP JSON document to clients probing the endpoint.
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [("allow", "POST")],
        Json(json!({"error": "GET is not supported; use POST for MCP messages"})),
    )
}

/// Validates and dispatches one JSON-RPC request.
async fn handle(
    State(app): State<App>,
    headers: HeaderMap,
    Json(request): Json<McpRequest>,
) -> HttpResponse {
    if let Some(version) = headers
        .get("mcp-protocol-version")
        .and_then(|value| value.to_str().ok())
        && !SUPPORTED_PROTOCOL_VERSIONS.contains(&version)
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "error": format!("unsupported MCP protocol version: {version}"),
                "supportedVersions": SUPPORTED_PROTOCOL_VERSIONS,
            })),
        )
            .into_response();
    }

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

    let requested_header_version = headers
        .get("mcp-protocol-version")
        .and_then(|value| value.to_str().ok());
    let initialize = request.method == "initialize";
    let negotiated = request
        .params
        .get("protocolVersion")
        .and_then(serde_json::Value::as_str)
        .map(|version| negotiate_protocol(Some(version)))
        .or_else(|| requested_header_version.map(|version| negotiate_protocol(Some(version))))
        .unwrap_or(LATEST_PROTOCOL_VERSION);

    match handler::handle_request(&app, request).await {
        Some(response) => {
            let mut response = Json(response).into_response();
            let response_version = if initialize {
                negotiated
            } else {
                requested_header_version.unwrap_or(negotiated)
            };
            if let Ok(value) = HeaderValue::from_str(response_version) {
                response.headers_mut().insert("mcp-protocol-version", value);
            }
            response
        }
        None => StatusCode::ACCEPTED.into_response(),
    }
}
