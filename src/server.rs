use crate::{app::App, oauth};
use anyhow::Result;
use axum::{
    Json, Router,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response as HttpResponse},
    routing::{get, post},
};
use fs_mcp_rs::protocol::Request as McpRequest;
use serde_json::json;
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

    match crate::handler::handle_request(&app, request).await {
        Some(response) => Json(response).into_response(),
        None => StatusCode::ACCEPTED.into_response(),
    }
}
