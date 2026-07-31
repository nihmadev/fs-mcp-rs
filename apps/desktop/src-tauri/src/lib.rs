use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tauri::Manager;
use tokio::sync::{oneshot, watch};

mod profiles;
use profiles::{Profile, ProfileState, ProfileStore};

/// Frontend-facing server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub roots: Vec<String>,
    pub port: u16,
    pub host: String,
    pub max_concurrency: usize,
    pub max_io_concurrency: usize,
    pub read_only: bool,
    pub follow_links: bool,
    pub max_read_mb: usize,
    pub max_write_mb: usize,
    pub tree_max_depth: usize,
    pub tree_max_entries: usize,
    pub tree_max_warnings: usize,
    pub patch_max_kb: usize,
    pub patch_preview_kb: usize,
    pub max_search_results: usize,
    pub search_max_concurrency: usize,
    pub search_worker_threads: usize,
    pub regex_cache_capacity: usize,
    pub include_hidden: bool,
    pub respect_gitignore: bool,
    pub terminal_enabled: bool,
    pub terminal_max_concurrency: usize,
    pub terminal_default_timeout_ms: u64,
    pub terminal_max_timeout_ms: u64,
    pub terminal_max_output_mb: usize,
    pub terminal_max_read_kb: usize,
    pub terminal_max_wait_ms: u64,
    pub terminal_session_retention_ms: u64,
    pub oauth_enabled: bool,
    pub oauth_require_auth: bool,
    pub oauth_issuer: Option<String>,
    pub log_tools: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelConfig {
    pub provider: String,
    pub executable: String,
    pub extra_args: Vec<String>,
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub running: bool,
    pub tunnel_running: bool,
    pub tunnel_provider: Option<String>,
    pub public_url: Option<String>,
    pub tunnel_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolActivity {
    id: String,
    timestamp_ms: u64,
    tool: String,
    target: String,
    duration_us: u64,
    status: &'static str,
    client: String,
    error: Option<String>,
}

static NEXT_ACTIVITY_ID: AtomicU64 = AtomicU64::new(1);

/// Runtime server state shared across Tauri commands.
struct ServerState {
    handle: Option<tokio::task::JoinHandle<()>>,
    shutdown_tx: Option<watch::Sender<bool>>,
    tunnel: Option<fs_mcp_rs::launcher::CompanionProcess>,
    tunnel_provider: Option<String>,
    public_url: Option<String>,
    tunnel_error: Option<String>,
    running: bool,
}

#[tauri::command]
async fn start_server(
    profile_id: String,
    state: tauri::State<'_, Arc<Mutex<ServerState>>>,
    profiles: tauri::State<'_, Arc<ProfileStore>>,
    app_handle: tauri::AppHandle,
) -> Result<String, String> {
    let shared_state = Arc::clone(state.inner());
    {
        let server = state.lock().map_err(|e| e.to_string())?;
        if server.running {
            return Err("Server is already running".into());
        }
    }

    let profile = active_profile(&profiles, &profile_id).map_err(|e| e.to_string())?;
    let config = ServerConfig::from(&profile);
    let settings = build_settings(&config).map_err(|e| e.to_string())?;
    let app = fs_mcp_rs::app::App::new(settings).map_err(|e| e.to_string())?;
    let address = (app.settings.server.host, app.settings.server.port);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .map_err(|e| format!("cannot listen on {}:{}: {e}", address.0, address.1))?;
    let (shutdown_tx, shutdown_rx) = watch::channel(false);

    let mut server = state.lock().map_err(|e| e.to_string())?;
    if server.running {
        return Err("Server is already running".into());
    }

    let handle = tokio::spawn(async move {
        if let Err(e) = run_server(app, app_handle, listener, shutdown_rx).await {
            tracing::error!("Server exited with error: {e}");
        }
        if let Ok(mut server) = shared_state.lock() {
            server.running = false;
            server.shutdown_tx = None;
            server.tunnel = None;
            server.tunnel_provider = None;
            server.public_url = None;
            server.tunnel_error = None;
        }
    });

    server.handle = Some(handle);
    server.shutdown_tx = Some(shutdown_tx);
    server.running = true;
    Ok("Server started".into())
}

/// Wraps `fs_mcp_rs::server::serve` with a custom shutdown signal.
async fn run_server(
    app: fs_mcp_rs::app::App,
    app_handle: tauri::AppHandle,
    listener: tokio::net::TcpListener,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let address = (app.settings.server.host, app.settings.server.port);
    let router = build_router(app.clone(), app_handle)?;
    tracing::info!(?address, "filesystem MCP listening (desktop)");
    axum::serve(listener, router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;
    Ok(())
}

/// Builds the Axum router — mirrors `fs_mcp_rs::server::serve` internals.
fn build_router(app: fs_mcp_rs::app::App, app_handle: tauri::AppHandle) -> Result<axum::Router> {
    use axum::{
        Extension, Json, Router,
        extract::{Request, State},
        http::{HeaderMap, HeaderValue, Method, StatusCode},
        middleware::{self, Next},
        response::{IntoResponse, Response as HttpResponse},
        routing::{get, post},
    };
    use fs_mcp_rs::protocol::Request as McpRequest;
    use serde_json::json;

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

    async fn handle(
        State(app): State<fs_mcp_rs::app::App>,
        Extension(app_handle): Extension<tauri::AppHandle>,
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

        let activity = activity_request(&request, &headers);
        let started = Instant::now();
        match fs_mcp_rs::handler::handle_request(&app, request).await {
            Some(response) => {
                if let Some((tool, target, client)) = activity {
                    let is_error = response.error.is_some()
                        || response
                            .result
                            .as_ref()
                            .and_then(|result| result.get("isError"))
                            .and_then(serde_json::Value::as_bool)
                            .unwrap_or(false);
                    let error = if is_error {
                        activity_error(&response)
                    } else {
                        None
                    };
                    let timestamp_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let event = ToolActivity {
                        id: format!(
                            "{timestamp_ms}-{}",
                            NEXT_ACTIVITY_ID.fetch_add(1, Ordering::Relaxed)
                        ),
                        timestamp_ms,
                        tool,
                        target,
                        duration_us: started.elapsed().as_micros() as u64,
                        status: if is_error { "error" } else { "ok" },
                        client,
                        error,
                    };
                    if let Err(error) = app_handle.emit("tool-activity", event) {
                        tracing::warn!("cannot emit tool activity: {error}");
                    }
                }
                Json(response).into_response()
            }
            None => StatusCode::ACCEPTED.into_response(),
        }
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

    let mut router = Router::new()
        .route("/", get(root_get))
        .route("/health", get(|| async { "ok" }))
        .route("/mcp", get(mcp_get).post(handle));

    if app.settings.oauth.enabled {
        use axum::routing::get as oget;
        router = router
            .route(
                "/.well-known/oauth-authorization-server",
                oget(fs_mcp_rs::oauth::oauth_authorization_server_metadata),
            )
            .route(
                "/.well-known/openid-configuration",
                oget(fs_mcp_rs::oauth::openid_configuration),
            )
            .route(
                "/.well-known/oauth-protected-resource",
                oget(fs_mcp_rs::oauth::oauth_protected_resource_metadata),
            )
            .route(
                "/.well-known/oauth-protected-resource/mcp",
                oget(fs_mcp_rs::oauth::oauth_protected_resource_metadata),
            )
            .route("/register", post(fs_mcp_rs::oauth::register_client))
            .route(
                "/authorize",
                oget(fs_mcp_rs::oauth::authorize_get).post(fs_mcp_rs::oauth::authorize_post),
            )
            .route("/token", post(fs_mcp_rs::oauth::token_endpoint))
            .route("/userinfo", oget(fs_mcp_rs::oauth::userinfo_endpoint))
            .route(
                "/.well-known/jwks.json",
                oget(fs_mcp_rs::oauth::jwks_endpoint),
            );
    }

    let router = router
        .layer(middleware::from_fn(cors_middleware))
        .layer(Extension(app_handle))
        .with_state(app);
    Ok(router)
}

fn activity_request(
    request: &fs_mcp_rs::protocol::Request,
    headers: &axum::http::HeaderMap,
) -> Option<(String, String, String)> {
    if request.method != "tools/call" {
        return None;
    }

    let tool = request
        .params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown_tool")
        .to_owned();
    let arguments = request.params.get("arguments");
    let target = arguments
        .and_then(serde_json::Value::as_object)
        .and_then(|arguments| {
            ["path", "command", "pattern", "source"]
                .iter()
                .find_map(|key| {
                    arguments
                        .get(*key)
                        .and_then(serde_json::Value::as_str)
                        .map(|value| format!("{key}={:?}", truncate(value, 80)))
                })
        })
        .unwrap_or_else(|| "No target details".to_owned());
    let client = headers
        .get(axum::http::header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|value| truncate(value, 80))
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "MCP client".to_owned());
    Some((tool, target, client))
}

fn activity_error(response: &fs_mcp_rs::protocol::Response) -> Option<String> {
    response
        .error
        .as_ref()
        .map(|error| truncate(&error.message, 160))
        .or_else(|| {
            response
                .result
                .as_ref()?
                .get("structuredContent")?
                .get("error")?
                .get("message")?
                .as_str()
                .map(|message| truncate(message, 160))
        })
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let prefix: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

#[cfg(test)]
mod activity_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_safe_tool_activity_summary() {
        let request = fs_mcp_rs::protocol::Request {
            jsonrpc: "2.0".to_owned(),
            id: Some(json!(1)),
            method: "tools/call".to_owned(),
            params: json!({
                "name": "read_file",
                "arguments": { "path": "src/main.rs", "content": "not exposed" }
            }),
        };
        let headers = axum::http::HeaderMap::from_iter([(
            axum::http::header::USER_AGENT,
            axum::http::HeaderValue::from_static("activity-test-client"),
        )]);

        let activity = activity_request(&request, &headers);

        assert_eq!(
            activity,
            Some((
                "read_file".to_owned(),
                "path=\"src/main.rs\"".to_owned(),
                "activity-test-client".to_owned(),
            ))
        );
    }

    #[test]
    fn ignores_non_tool_requests_and_truncates_unicode_safely() {
        let request = fs_mcp_rs::protocol::Request {
            jsonrpc: "2.0".to_owned(),
            id: Some(json!(1)),
            method: "tools/list".to_owned(),
            params: json!(null),
        };

        assert_eq!(
            activity_request(&request, &axum::http::HeaderMap::new()),
            None
        );
        assert_eq!(truncate("абвгд", 3), "абв...");
    }
}

fn build_settings(config: &ServerConfig) -> Result<fs_mcp_rs::settings::Settings> {
    anyhow::ensure!(
        !config.roots.is_empty(),
        "at least one filesystem root is required"
    );
    anyhow::ensure!(
        config.roots.iter().all(|root| !root.trim().is_empty()),
        "filesystem roots must not be empty"
    );
    anyhow::ensure!(
        config.max_concurrency > 0 && config.max_io_concurrency > 0,
        "server concurrency limits must be greater than zero"
    );
    anyhow::ensure!(
        config.max_read_mb > 0 && config.max_write_mb > 0,
        "filesystem size limits must be greater than zero"
    );
    anyhow::ensure!(
        config.tree_max_depth > 0 && config.tree_max_entries > 0 && config.tree_max_warnings > 0,
        "tree limits must be greater than zero"
    );
    anyhow::ensure!(
        config.patch_max_kb > 0
            && config.patch_preview_kb > 0
            && config.patch_preview_kb <= config.patch_max_kb,
        "patch preview must be positive and no larger than the patch limit"
    );
    anyhow::ensure!(
        config.max_search_results > 0
            && config.search_max_concurrency > 0
            && config.search_worker_threads > 0,
        "search limits must be greater than zero"
    );
    anyhow::ensure!(
        config.terminal_max_concurrency > 0,
        "terminal concurrency must be greater than zero"
    );
    anyhow::ensure!(
        config.terminal_default_timeout_ms > 0
            && config.terminal_default_timeout_ms <= config.terminal_max_timeout_ms,
        "terminal default timeout must be positive and no larger than the maximum timeout"
    );
    anyhow::ensure!(
        config.terminal_max_output_mb > 0
            && config.terminal_max_read_kb > 0
            && config.terminal_max_wait_ms > 0
            && config.terminal_session_retention_ms > 0,
        "terminal limits must be greater than zero"
    );
    anyhow::ensure!(
        !config.oauth_require_auth || config.oauth_enabled,
        "OAuth must be enabled when authentication is required"
    );

    let host: std::net::IpAddr = config.host.parse()?;
    anyhow::ensure!(
        !config.oauth_require_auth || host.is_loopback(),
        "the built-in OAuth service may require authentication only on a loopback address"
    );

    let mib = 1024usize * 1024;
    let kib = 1024usize;
    let roots = config.roots.iter().map(PathBuf::from).collect();
    let mut settings = fs_mcp_rs::settings::Settings::default_with_roots(roots)?;
    settings.server.host = host;
    settings.server.port = config.port;
    settings.server.max_concurrency = config.max_concurrency;
    settings.server.max_io_concurrency = config.max_io_concurrency;
    settings.filesystem.read_only = config.read_only;
    settings.filesystem.follow_links = config.follow_links;
    settings.filesystem.max_read_bytes = config
        .max_read_mb
        .checked_mul(mib)
        .ok_or_else(|| anyhow::anyhow!("max read size is too large"))?;
    settings.filesystem.max_write_bytes = config
        .max_write_mb
        .checked_mul(mib)
        .ok_or_else(|| anyhow::anyhow!("max write size is too large"))?;
    settings.filesystem.tree_max_depth = config.tree_max_depth;
    settings.filesystem.tree_max_entries = config.tree_max_entries;
    settings.filesystem.tree_max_warnings = config.tree_max_warnings;
    settings.filesystem.patch_max_bytes = config
        .patch_max_kb
        .checked_mul(kib)
        .ok_or_else(|| anyhow::anyhow!("patch size is too large"))?;
    settings.filesystem.patch_preview_bytes = config
        .patch_preview_kb
        .checked_mul(kib)
        .ok_or_else(|| anyhow::anyhow!("patch preview size is too large"))?;
    settings.search.max_results = config.max_search_results;
    settings.search.max_concurrency = config.search_max_concurrency;
    settings.search.worker_threads = config.search_worker_threads;
    settings.search.regex_cache_capacity = config.regex_cache_capacity;
    settings.search.include_hidden = config.include_hidden;
    settings.search.respect_gitignore = config.respect_gitignore;
    settings.terminal.enabled = config.terminal_enabled;
    settings.terminal.max_concurrency = config.terminal_max_concurrency;
    settings.terminal.default_timeout_ms = config.terminal_default_timeout_ms;
    settings.terminal.max_timeout_ms = config.terminal_max_timeout_ms;
    settings.terminal.max_output_bytes = config
        .terminal_max_output_mb
        .checked_mul(mib)
        .ok_or_else(|| anyhow::anyhow!("terminal output size is too large"))?;
    settings.terminal.max_read_bytes = config
        .terminal_max_read_kb
        .checked_mul(kib)
        .ok_or_else(|| anyhow::anyhow!("terminal read size is too large"))?;
    settings.terminal.max_wait_ms = config.terminal_max_wait_ms;
    settings.terminal.session_retention_ms = config.terminal_session_retention_ms;
    settings.oauth.enabled = config.oauth_enabled;
    settings.oauth.require_auth = config.oauth_require_auth;
    settings.oauth.issuer = config
        .oauth_issuer
        .clone()
        .filter(|value| !value.trim().is_empty());
    settings.server.log_tools = config.log_tools;
    Ok(settings)
}

impl From<&Profile> for ServerConfig {
    fn from(profile: &Profile) -> Self {
        Self {
            roots: profile.roots.clone(),
            port: profile.port,
            host: profile.host.clone(),
            max_concurrency: profile.max_concurrency,
            max_io_concurrency: profile.max_io_concurrency,
            read_only: profile.read_only,
            follow_links: profile.follow_links,
            max_read_mb: profile.max_read_mb,
            max_write_mb: profile.max_write_mb,
            tree_max_depth: profile.tree_max_depth,
            tree_max_entries: profile.tree_max_entries,
            tree_max_warnings: profile.tree_max_warnings,
            patch_max_kb: profile.patch_max_kb,
            patch_preview_kb: profile.patch_preview_kb,
            max_search_results: profile.max_search_results,
            search_max_concurrency: profile.search_max_concurrency,
            search_worker_threads: profile.search_worker_threads,
            regex_cache_capacity: profile.regex_cache_capacity,
            include_hidden: profile.include_hidden,
            respect_gitignore: profile.respect_gitignore,
            terminal_enabled: profile.terminal_enabled,
            terminal_max_concurrency: profile.terminal_max_concurrency,
            terminal_default_timeout_ms: profile.terminal_default_timeout_ms,
            terminal_max_timeout_ms: profile.terminal_max_timeout_ms,
            terminal_max_output_mb: profile.terminal_max_output_mb,
            terminal_max_read_kb: profile.terminal_max_read_kb,
            terminal_max_wait_ms: profile.terminal_max_wait_ms,
            terminal_session_retention_ms: profile.terminal_session_retention_ms,
            oauth_enabled: profile.oauth_enabled,
            oauth_require_auth: profile.oauth_require_auth,
            oauth_issuer: profile.oauth_issuer.clone(),
            log_tools: profile.log_tools,
        }
    }
}

fn active_profile(store: &ProfileStore, requested_id: &str) -> Result<Profile> {
    let state = store.load_or_initialize()?;
    anyhow::ensure!(
        state.active_profile_id == requested_id,
        "profile is not active; select it before starting the server"
    );
    state
        .profiles
        .into_iter()
        .find(|profile| profile.id == requested_id)
        .ok_or_else(|| anyhow::anyhow!("active profile does not exist"))
}

fn profile_toml(profile: &Profile) -> Result<String> {
    let settings = build_settings(&ServerConfig::from(profile))?;
    toml::to_string_pretty(&settings).context("cannot serialize server configuration")
}

#[tauri::command]
fn load_profiles(store: tauri::State<'_, Arc<ProfileStore>>) -> Result<ProfileState, String> {
    store
        .load_or_initialize()
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn reset_profiles(store: tauri::State<'_, Arc<ProfileStore>>) -> Result<ProfileState, String> {
    store.reset().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_profile(
    profile: Profile,
    store: tauri::State<'_, Arc<ProfileStore>>,
) -> Result<ProfileState, String> {
    build_settings(&ServerConfig::from(&profile)).map_err(|error| error.to_string())?;
    store
        .save_profile(profile)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_profile(
    name: String,
    duplicate_id: Option<String>,
    store: tauri::State<'_, Arc<ProfileStore>>,
) -> Result<ProfileState, String> {
    store
        .create_profile(name, duplicate_id.as_deref())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_profile(
    id: String,
    name: String,
    store: tauri::State<'_, Arc<ProfileStore>>,
) -> Result<ProfileState, String> {
    store
        .rename_profile(&id, name)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_profile(
    id: String,
    store: tauri::State<'_, Arc<ProfileStore>>,
) -> Result<ProfileState, String> {
    store.delete_profile(&id).map_err(|error| error.to_string())
}

#[tauri::command]
fn set_active_profile(
    id: String,
    store: tauri::State<'_, Arc<ProfileStore>>,
) -> Result<ProfileState, String> {
    store.set_active(&id).map_err(|error| error.to_string())
}

#[tauri::command]
fn export_profile_toml(
    profile_id: String,
    path: String,
    overwrite: bool,
    store: tauri::State<'_, Arc<ProfileStore>>,
) -> Result<String, String> {
    let state = store
        .load_or_initialize()
        .map_err(|error| error.to_string())?;
    let profile = state
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "profile does not exist".to_owned())?;
    let destination = PathBuf::from(path);
    if destination.exists() && !overwrite {
        return Err(
            "The selected file already exists. Confirm overwrite to replace it.".to_owned(),
        );
    }
    let text = profile_toml(profile).map_err(|error| error.to_string())?;
    std::fs::write(&destination, text)
        .map_err(|error| format!("cannot save {}: {error}", destination.display()))?;
    Ok(destination.display().to_string())
}

#[tauri::command]
fn get_client_snippets(
    profile_id: String,
    store: tauri::State<'_, Arc<ProfileStore>>,
) -> Result<Vec<fs_mcp_rs::cli_format::ClientSnippet>, String> {
    let state = store
        .load_or_initialize()
        .map_err(|error| error.to_string())?;
    let profile = state
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| "profile does not exist".to_owned())?;
    let config_path = store.config_path(&profile.id);
    std::fs::write(
        &config_path,
        profile_toml(profile).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("cannot write snippet configuration: {error}"))?;
    Ok(fs_mcp_rs::cli_format::client_snippets(
        &config_path,
        "fs-mcp-rs",
        &profile.display_name,
        &profile.host,
        profile.port,
    ))
}

#[tauri::command]
fn save_snippet(path: String, content: String, overwrite: bool) -> Result<String, String> {
    let destination = PathBuf::from(path);
    if destination.exists() && !overwrite {
        return Err(
            "The selected file already exists. Confirm overwrite to replace it.".to_owned(),
        );
    }
    serde_json::from_str::<serde_json::Value>(&content)
        .map_err(|error| format!("invalid snippet JSON: {error}"))?;
    std::fs::write(&destination, content)
        .map_err(|error| format!("cannot save {}: {error}", destination.display()))?;
    Ok(destination.display().to_string())
}

#[tauri::command]
async fn stop_server(state: tauri::State<'_, Arc<Mutex<ServerState>>>) -> Result<String, String> {
    let handle = {
        let mut server = state.lock().map_err(|e| e.to_string())?;
        if !server.running {
            return Err("Server is not running".into());
        }
        if let Some(tx) = server.shutdown_tx.take() {
            let _ = tx.send(true);
        }
        server.tunnel = None;
        server.tunnel_provider = None;
        server.public_url = None;
        server.tunnel_error = None;
        server.handle.take()
    };
    if let Some(h) = handle {
        let _ = h.await;
    }
    {
        let mut server = state.lock().map_err(|e| e.to_string())?;
        server.running = false;
    }
    Ok("Server stopped".into())
}

#[tauri::command]
async fn get_server_status(
    state: tauri::State<'_, Arc<Mutex<ServerState>>>,
) -> Result<bool, String> {
    let server = state.lock().map_err(|e| e.to_string())?;
    Ok(server.running)
}

#[tauri::command]
async fn get_runtime_status(
    state: tauri::State<'_, Arc<Mutex<ServerState>>>,
) -> Result<RuntimeStatus, String> {
    let mut server = state.lock().map_err(|e| e.to_string())?;
    let tunnel_running = match server.tunnel.as_mut() {
        Some(tunnel) => tunnel.is_running().map_err(|e| e.to_string())?,
        None => false,
    };
    let tunnel_starting = server.tunnel.is_none() && server.tunnel_provider.is_some();
    let tunnel_error = if !tunnel_running && !tunnel_starting {
        let error = server
            .tunnel
            .as_ref()
            .and_then(|tunnel| tunnel.failure().map(str::to_owned));
        server.tunnel = None;
        server.tunnel_provider = None;
        server.public_url = None;
        if error.is_some() {
            server.tunnel_error = error;
        }
        server.tunnel_error.clone()
    } else {
        server.tunnel_error.clone()
    };
    if !tunnel_running && !tunnel_starting {
        server.tunnel = None;
    }
    Ok(RuntimeStatus {
        running: server.running,
        tunnel_running: tunnel_running || tunnel_starting,
        tunnel_provider: server.tunnel_provider.clone(),
        public_url: server.public_url.clone(),
        tunnel_error,
    })
}

#[tauri::command]
async fn start_tunnel(
    config: TunnelConfig,
    state: tauri::State<'_, Arc<Mutex<ServerState>>>,
) -> Result<String, String> {
    let host = config
        .host
        .parse::<std::net::IpAddr>()
        .map_err(|error| error.to_string())?;
    {
        let mut server = state.lock().map_err(|e| e.to_string())?;
        if !server.running {
            return Err("Start the MCP server before connecting remote access".into());
        }
        if server.tunnel.is_some() {
            return Err("A tunnel is already running".into());
        }
        server.tunnel_provider = Some(config.provider.clone());
        server.public_url = None;
        server.tunnel_error = None;
    }

    let executable_input = config.executable.trim();
    let executable_input = executable_input
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(executable_input);
    let executable = if executable_input.is_empty() {
        config.provider.as_str()
    } else {
        executable_input
    };
    let extra_args = &config.extra_args;
    let program = std::path::Path::new(executable);
    let tunnel = match config.provider.as_str() {
        "ngrok" => fs_mcp_rs::launcher::Tunnel::Ngrok {
            program,
            extra_args,
        },
        "cloudflared" => fs_mcp_rs::launcher::Tunnel::Cloudflared {
            program,
            extra_args,
        },
        "zrok" => fs_mcp_rs::launcher::Tunnel::Zrok {
            program,
            extra_args,
        },
        _ => return Err("unsupported tunnel provider".into()),
    };
    let shared_state = Arc::clone(state.inner());
    let callback_provider = config.provider.clone();
    let companion = fs_mcp_rs::launcher::CompanionProcess::start_tunnel_with_url_callback(
        tunnel,
        host,
        config.port,
        fs_mcp_rs::settings::CompanionWindow::Hidden,
        move |url| {
            if let Ok(mut server) = shared_state.lock() {
                if server.tunnel_provider.as_deref() == Some(callback_provider.as_str()) {
                    server.public_url = Some(url);
                }
            }
        },
    )
    .map_err(|error| {
        if let Ok(mut server) = state.lock() {
            server.tunnel_provider = None;
            server.tunnel_error = Some(error.to_string());
        }
        error.to_string()
    })?;

    let mut server = state.lock().map_err(|e| e.to_string())?;
    if !server.running {
        return Err("The MCP server stopped while the tunnel was connecting".into());
    }
    let public_url = server.public_url.clone().ok_or_else(|| {
        "Tunnel provider reported ready, but did not provide a public URL".to_owned()
    })?;
    server.tunnel = Some(companion);
    Ok(public_url)
}

#[tauri::command]
async fn stop_tunnel(state: tauri::State<'_, Arc<Mutex<ServerState>>>) -> Result<String, String> {
    let mut server = state.lock().map_err(|e| e.to_string())?;
    if server.tunnel.take().is_none() {
        return Err("Tunnel is not running".into());
    }
    server.tunnel_provider = None;
    server.public_url = None;
    Ok("Tunnel stopped".into())
}

#[tauri::command]
async fn pick_folder(app: tauri::AppHandle<tauri::Wry>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = oneshot::channel();
    app.dialog().file().pick_folder(move |path| {
        let _ = tx.send(path.map(|p| p.to_string()));
    });
    rx.await.map_err(|e| e.to_string())
}

/// Starts the Tauri desktop application.
pub fn run() {
    tracing_subscriber::fmt().with_env_filter("info").init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let directory = app.path().app_data_dir()?;
            app.manage(Arc::new(ProfileStore::new(directory)));
            Ok(())
        })
        .manage(Arc::new(Mutex::new(ServerState {
            handle: None,
            shutdown_tx: None,
            tunnel: None,
            tunnel_provider: None,
            public_url: None,
            tunnel_error: None,
            running: false,
        })))
        .invoke_handler(tauri::generate_handler![
            load_profiles,
            reset_profiles,
            save_profile,
            create_profile,
            rename_profile,
            delete_profile,
            set_active_profile,
            export_profile_toml,
            get_client_snippets,
            save_snippet,
            start_server,
            stop_server,
            get_server_status,
            get_runtime_status,
            start_tunnel,
            stop_tunnel,
            pick_folder,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(
                event,
                tauri::RunEvent::ExitRequested { .. } | tauri::RunEvent::Exit
            ) {
                if let Some(state) = app.try_state::<Arc<Mutex<ServerState>>>() {
                    if let Ok(mut server) = state.lock() {
                        if let Some(tx) = server.shutdown_tx.take() {
                            let _ = tx.send(true);
                        }
                        server.tunnel = None;
                        server.tunnel_provider = None;
                        server.public_url = None;
                        server.running = false;
                    }
                }
            }
        });
}

#[cfg(test)]
mod configuration_tests {
    use super::*;

    fn profile_with_roots(roots: &[&str]) -> Profile {
        let mut profile = Profile::default();
        profile.roots = roots.iter().map(|root| (*root).to_owned()).collect();
        profile
    }

    #[test]
    fn converts_multiple_roots_and_rejects_empty_roots() {
        let profile = profile_with_roots(&["C:/workspace-a", "D:/workspace-b"]);
        let settings = build_settings(&ServerConfig::from(&profile)).unwrap();
        assert_eq!(settings.filesystem.roots.len(), 2);
        assert!(build_settings(&ServerConfig::from(&Profile::default())).is_err());
    }

    #[test]
    fn exported_toml_round_trips_through_settings_load() {
        let temp = tempfile::tempdir().unwrap();
        let profile = profile_with_roots(&[
            temp.path().to_str().unwrap(),
            temp.path().join("other").to_str().unwrap(),
        ]);
        let text = profile_toml(&profile).unwrap();
        assert!(text.contains("roots = ["));
        let path = temp.path().join("profile.toml");
        std::fs::write(&path, text).unwrap();
        let loaded = fs_mcp_rs::settings::Settings::load(&path).unwrap();
        assert_eq!(loaded.filesystem.roots.len(), 2);
    }

    #[test]
    fn snippets_reference_one_config_and_contain_no_secrets() {
        let profile = profile_with_roots(&["C:/one", "D:/two"]);
        let snippets = fs_mcp_rs::cli_format::client_snippets(
            PathBuf::from("profile.toml").as_path(),
            "fs-mcp-rs",
            &profile.display_name,
            &profile.host,
            profile.port,
        );
        assert_eq!(snippets.len(), 2);
        let all = snippets
            .iter()
            .map(|item| item.content.as_str())
            .collect::<String>();
        assert!(all.contains("profile.toml"));
        assert!(!all.to_ascii_lowercase().contains("secret"));
        assert!(!all.contains("C:/one"));
        assert!(!all.contains("D:/two"));
    }
}
