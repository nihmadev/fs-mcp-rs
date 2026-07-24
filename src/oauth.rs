//! OAuth 2.0 and OpenID Connect implementation for fs-mcp-rs.
//!
//! Provides discovery metadata (RFC 8414, RFC 9728, OIDC), dynamic client registration (RFC 7591),
//! PKCE authorization code grant flow, token exchange, and bearer token validation.

use crate::app::App;
use axum::{
    Form, Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

fn random_hex(bytes: usize) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut out = String::with_capacity(bytes * 2);
    for i in 0..bytes {
        let val = ((nanos.wrapping_add(i as u128 * 0x9e3779b97f4a7c15)) >> ((i % 8) * 4)) as u8;
        out.push_str(&format!("{:02x}", val));
    }
    out
}

use sha2::{Digest, Sha256};

fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hasher.finalize().into()
}

fn base64_url_nopad(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut i = 0;
    while i < data.len() {
        let b0 = data[i] as usize;
        let b1 = if i + 1 < data.len() {
            data[i + 1] as usize
        } else {
            0
        };
        let b2 = if i + 2 < data.len() {
            data[i + 2] as usize
        } else {
            0
        };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(triple >> 18) & 63] as char);
        out.push(ALPHABET[(triple >> 12) & 63] as char);
        if i + 1 < data.len() {
            out.push(ALPHABET[(triple >> 6) & 63] as char);
        }
        if i + 2 < data.len() {
            out.push(ALPHABET[triple & 63] as char);
        }
        i += 3;
    }
    out
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RegisteredClient {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uris: Vec<String>,
    pub client_name: Option<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub scope: Option<String>,
    pub issued_at: u64,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AuthCode {
    pub code: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub expires_at: Instant,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct TokenInfo {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub client_id: String,
    pub scope: String,
    pub expires_at: Instant,
}

#[derive(Default)]
struct OAuthStateInner {
    clients: HashMap<String, RegisteredClient>,
    codes: HashMap<String, AuthCode>,
    tokens: HashMap<String, TokenInfo>,
    refresh_tokens: HashMap<String, TokenInfo>,
}

#[derive(Clone, Default)]
pub struct OAuthStore {
    inner: Arc<RwLock<OAuthStateInner>>,
}

impl OAuthStore {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(OAuthStateInner::default())),
        }
    }

    pub fn register_client(&self, mut client: RegisteredClient) -> RegisteredClient {
        if client.client_id.is_empty() {
            client.client_id = format!("client_{}", random_hex(12));
        }
        if client.client_secret.is_none() {
            client.client_secret = Some(format!("secret_{}", random_hex(16)));
        }
        let mut guard = self.inner.write().expect("lock poisoned");
        guard
            .clients
            .insert(client.client_id.clone(), client.clone());
        client
    }

    #[allow(dead_code)]
    pub fn get_client(&self, client_id: &str) -> Option<RegisteredClient> {
        let guard = self.inner.read().expect("lock poisoned");
        guard.clients.get(client_id).cloned()
    }

    pub fn create_code(
        &self,
        client_id: String,
        redirect_uri: String,
        scope: String,
        code_challenge: Option<String>,
        code_challenge_method: Option<String>,
    ) -> String {
        let code = format!("code_{}", random_hex(16));
        let auth_code = AuthCode {
            code: code.clone(),
            client_id,
            redirect_uri,
            scope,
            code_challenge,
            code_challenge_method,
            expires_at: Instant::now() + Duration::from_secs(600),
        };
        let mut guard = self.inner.write().expect("lock poisoned");
        guard.codes.insert(code.clone(), auth_code);
        code
    }

    pub fn consume_code(&self, code: &str) -> Option<AuthCode> {
        let mut guard = self.inner.write().expect("lock poisoned");
        if let Some(auth_code) = guard.codes.remove(code) {
            if auth_code.expires_at > Instant::now() {
                return Some(auth_code);
            }
        }
        None
    }

    pub fn issue_token(&self, client_id: String, scope: String) -> TokenInfo {
        let access_token = format!("at_{}", random_hex(20));
        let refresh_token = format!("rt_{}", random_hex(20));
        let token_info = TokenInfo {
            access_token: access_token.clone(),
            refresh_token: Some(refresh_token.clone()),
            client_id,
            scope,
            expires_at: Instant::now() + Duration::from_secs(86400),
        };
        let mut guard = self.inner.write().expect("lock poisoned");
        guard.tokens.insert(access_token, token_info.clone());
        guard
            .refresh_tokens
            .insert(refresh_token, token_info.clone());
        token_info
    }

    pub fn validate_token(&self, token: &str) -> bool {
        let guard = self.inner.read().expect("lock poisoned");
        if let Some(t) = guard.tokens.get(token) {
            return t.expires_at > Instant::now();
        }
        false
    }
}

pub fn get_issuer_url(app: &App, headers: &HeaderMap) -> String {
    if let Some(ref issuer) = app.settings.oauth.issuer {
        return issuer.trim_end_matches('/').to_string();
    }
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if !host.is_empty() {
        let scheme = if headers
            .get("x-forwarded-proto")
            .and_then(|v| v.to_str().ok())
            == Some("https")
        {
            "https"
        } else {
            "http"
        };
        return format!("{scheme}://{host}");
    }
    format!(
        "http://{}:{}",
        app.settings.server.host, app.settings.server.port
    )
}

pub async fn oauth_authorization_server_metadata(
    State(app): State<App>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let issuer = get_issuer_url(&app, &headers);
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "registration_endpoint": format!("{issuer}/register"),
        "userinfo_endpoint": format!("{issuer}/userinfo"),
        "jwks_uri": format!("{issuer}/.well-known/jwks.json"),
        "scopes_supported": ["mcp", "openid", "profile"],
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token", "client_credentials"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "code_challenge_methods_supported": ["S256", "plain"]
    }))
}

pub async fn openid_configuration(State(app): State<App>, headers: HeaderMap) -> impl IntoResponse {
    let issuer = get_issuer_url(&app, &headers);
    Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/authorize"),
        "token_endpoint": format!("{issuer}/token"),
        "registration_endpoint": format!("{issuer}/register"),
        "userinfo_endpoint": format!("{issuer}/userinfo"),
        "jwks_uri": format!("{issuer}/.well-known/jwks.json"),
        "scopes_supported": ["mcp", "openid", "profile"],
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token", "client_credentials"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "code_challenge_methods_supported": ["S256", "plain"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256", "HS256"]
    }))
}

pub async fn oauth_protected_resource_metadata(
    State(app): State<App>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let issuer = get_issuer_url(&app, &headers);
    Json(json!({
        "resource": format!("{issuer}/mcp"),
        "authorization_servers": [issuer],
        "scopes_supported": ["mcp"],
        "bearer_methods_supported": ["header"]
    }))
}

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub client_name: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
    pub grant_types: Option<Vec<String>>,
    pub response_types: Option<Vec<String>>,
    pub scope: Option<String>,
}

pub async fn register_client(
    State(app): State<App>,
    Json(req): Json<RegisterRequest>,
) -> impl IntoResponse {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let registered = app.oauth.register_client(RegisteredClient {
        client_id: String::new(),
        client_secret: None,
        redirect_uris: req.redirect_uris.unwrap_or_default(),
        client_name: req.client_name.clone(),
        grant_types: req
            .grant_types
            .unwrap_or_else(|| vec!["authorization_code".to_string()]),
        response_types: req
            .response_types
            .unwrap_or_else(|| vec!["code".to_string()]),
        scope: req.scope,
        issued_at: now,
    });

    (
        StatusCode::CREATED,
        Json(json!({
            "client_id": registered.client_id,
            "client_secret": registered.client_secret,
            "client_id_issued_at": registered.issued_at,
            "client_secret_expires_at": 0,
            "redirect_uris": registered.redirect_uris,
            "grant_types": registered.grant_types,
            "response_types": registered.response_types,
            "client_name": registered.client_name,
            "token_endpoint_auth_method": "client_secret_post"
        })),
    )
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct AuthorizeParams {
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub response_type: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
}

pub async fn authorize_get(
    State(app): State<App>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    let client_id = params.client_id.clone().unwrap_or_default();
    let redirect_uri = params.redirect_uri.clone().unwrap_or_default();
    let state = params.state.clone().unwrap_or_default();

    if client_id.is_empty() || redirect_uri.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_request", "error_description": "missing client_id or redirect_uri"})),
        )
            .into_response();
    }

    let code = app.oauth.create_code(
        client_id.clone(),
        redirect_uri.clone(),
        params.scope.unwrap_or_else(|| "mcp openid".to_string()),
        params.code_challenge,
        params.code_challenge_method,
    );

    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head><title>Authorize MCP Access</title></head>
<body style="font-family: sans-serif; max-width: 400px; margin: 50px auto; text-align: center;">
    <h2>Authorize Access</h2>
    <p>Application <strong>{}</strong> is requesting access to fs-mcp-rs.</p>
    <form method="POST" action="/authorize">
        <input type="hidden" name="code" value="{}" />
        <input type="hidden" name="redirect_uri" value="{}" />
        <input type="hidden" name="state" value="{}" />
        <button type="submit" style="padding: 10px 20px; font-size: 16px; background: #0066cc; color: white; border: none; border-radius: 4px; cursor: pointer;">Approve Access</button>
    </form>
</body>
</html>"#,
        client_id, code, redirect_uri, state
    );

    Html(html).into_response()
}

#[derive(Debug, Deserialize)]
pub struct AuthorizeForm {
    pub code: String,
    pub redirect_uri: String,
    pub state: Option<String>,
}

pub async fn authorize_post(Form(form): Form<AuthorizeForm>) -> impl IntoResponse {
    let mut location = format!("{}?code={}", form.redirect_uri, form.code);
    if let Some(state) = form.state {
        if !state.is_empty() {
            location.push_str(&format!("&state={state}"));
        }
    }
    Redirect::to(&location)
}

fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let clean = input.trim_end_matches('=');
    let mut out = Vec::new();
    let bytes = clean.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let pos = |b: u8| -> Option<u8> {
            if b == b'-' {
                return Some(62);
            }
            if b == b'_' {
                return Some(63);
            }
            ALPHABET.iter().position(|&c| c == b).map(|p| p as u8)
        };
        let b0 = pos(bytes[i])?;
        let b1 = if i + 1 < bytes.len() {
            pos(bytes[i + 1])?
        } else {
            0
        };
        let b2 = if i + 2 < bytes.len() {
            pos(bytes[i + 2])?
        } else {
            0
        };
        let b3 = if i + 3 < bytes.len() {
            pos(bytes[i + 3])?
        } else {
            0
        };

        let triple = ((b0 as u32) << 18) | ((b1 as u32) << 12) | ((b2 as u32) << 6) | (b3 as u32);
        out.push(((triple >> 16) & 0xFF) as u8);
        if i + 2 < bytes.len() {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if i + 3 < bytes.len() {
            out.push((triple & 0xFF) as u8);
        }
        i += 4;
    }
    Some(out)
}

pub async fn token_endpoint(
    State(app): State<App>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    let content_type = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let mut map: HashMap<String, String> = if content_type.contains("application/json") {
        serde_json::from_slice(&body).unwrap_or_default()
    } else {
        serde_urlencoded::from_bytes(&body)
            .or_else(|_| serde_json::from_slice(&body))
            .unwrap_or_default()
    };

    if let Some(auth) = headers.get("authorization").and_then(|v| v.to_str().ok()) {
        if let Some(basic) = auth.strip_prefix("Basic ") {
            if let Some(decoded) = base64_decode(basic.trim()) {
                if let Ok(str_val) = String::from_utf8(decoded) {
                    if let Some((id, secret)) = str_val.split_once(':') {
                        map.entry("client_id".to_string())
                            .or_insert_with(|| id.to_string());
                        map.entry("client_secret".to_string())
                            .or_insert_with(|| secret.to_string());
                    }
                }
            }
        }
    }

    let grant_type = map
        .get("grant_type")
        .map(|s| s.as_str())
        .unwrap_or("authorization_code");

    if grant_type == "client_credentials" {
        let client_id = map
            .get("client_id")
            .cloned()
            .unwrap_or_else(|| "client_credentials".to_string());
        let token = app.oauth.issue_token(client_id, "mcp openid".to_string());
        return (
            StatusCode::OK,
            Json(json!({
                "access_token": token.access_token,
                "token_type": "Bearer",
                "expires_in": 86400,
                "refresh_token": token.refresh_token,
                "scope": token.scope
            })),
        )
            .into_response();
    }

    if grant_type == "authorization_code" {
        let code_str = match map.get("code") {
            Some(c) if !c.is_empty() => c.as_str(),
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "invalid_request", "error_description": "missing code"})),
                )
                    .into_response();
            }
        };

        let auth_code = match app.oauth.consume_code(code_str) {
            Some(ac) => ac,
            None => {
                tracing::warn!(code = code_str, "Code lookup failed in token_endpoint");
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "invalid_grant", "error_description": "code invalid or expired"})),
                )
                    .into_response();
            }
        };

        if let Some(ref challenge) = auth_code.code_challenge {
            let verifier = map.get("code_verifier").map(|s| s.as_str()).unwrap_or("");
            let method = auth_code
                .code_challenge_method
                .as_deref()
                .unwrap_or("plain");

            let valid = match method {
                "S256" => {
                    let hashed = sha256(verifier.as_bytes());
                    let encoded = base64_url_nopad(&hashed);
                    encoded == *challenge
                }
                _ => verifier == challenge,
            };

            if !valid {
                tracing::warn!(
                    verifier,
                    challenge,
                    method,
                    "PKCE validation failed in token_endpoint"
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error": "invalid_grant", "error_description": "PKCE verification failed"})),
                )
                    .into_response();
            }
        }

        let token = app.oauth.issue_token(auth_code.client_id, auth_code.scope);
        return (
            StatusCode::OK,
            Json(json!({
                "access_token": token.access_token,
                "token_type": "Bearer",
                "expires_in": 86400,
                "refresh_token": token.refresh_token,
                "scope": token.scope
            })),
        )
            .into_response();
    }

    (
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "unsupported_grant_type"})),
    )
        .into_response()
}

pub async fn userinfo_endpoint(State(_app): State<App>, headers: HeaderMap) -> impl IntoResponse {
    let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
    if auth_header.is_none() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "unauthorized"})),
        )
            .into_response();
    }

    Json(json!({
        "sub": "fs-mcp-user",
        "name": "FS MCP User",
        "preferred_username": "mcp-user"
    }))
    .into_response()
}

pub async fn jwks_endpoint() -> impl IntoResponse {
    Json(json!({ "keys": [] }))
}
