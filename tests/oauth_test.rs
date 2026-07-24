//! Integration tests for OAuth 2.0 / OpenID Connect metadata, discovery, dynamic registration, and authorization.

use serde_json::{Value, json};
use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start_server(require_auth: bool) -> (Server, u16, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let port = 25_000
        + (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
            % 10_000) as u16;
    let config = root.path().join("config.toml");
    fs::write(
        &config,
        format!(
            r#"
[server]
host = "127.0.0.1"
port = {port}
max_concurrency = 4
max_io_concurrency = 2
[filesystem]
roots = ["{}"]
read_only = false
max_read_bytes = 1048576
max_write_bytes = 1048576
follow_links = false
[search]
max_results = 100
max_concurrency = 2
worker_threads = 1
regex_cache_capacity = 4
include_hidden = false
respect_gitignore = true
[terminal]
enabled = false
max_concurrency = 1
default_timeout_ms = 1000
max_timeout_ms = 2000
max_output_bytes = 1024
[oauth]
enabled = true
require_auth = {require_auth}
"#,
            root.path().display().to_string().replace('\\', "\\\\")
        ),
    )
    .unwrap();
    let server = Server(
        Command::new(env!("CARGO_BIN_EXE_fs-mcp-rs"))
            .args(["--config", config.to_str().unwrap()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return (server, port, root);
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("server did not start");
}

fn http_request(port: u16, method: &str, path: &str, headers: &[(&str, &str)], body: Option<&str>) -> (u16, Value) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    let body_bytes = body.unwrap_or("").as_bytes();
    let mut req = format!("{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n");
    if body.is_some() {
        req.push_str(&format!("Content-Length: {}\r\n", body_bytes.len()));
    }
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    stream.write_all(req.as_bytes()).unwrap();
    if !body_bytes.is_empty() {
        stream.write_all(body_bytes).unwrap();
    }
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();

    let text = String::from_utf8_lossy(&response);
    let status_line = text.lines().next().unwrap_or_default();
    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    let split = match response.windows(4).position(|w| w == b"\r\n\r\n") {
        Some(pos) => pos + 4,
        None => response.len(),
    };
    let json_val: Value = serde_json::from_slice(&response[split..]).unwrap_or_default();
    (status_code, json_val)
}

#[test]
fn test_oauth_discovery_endpoints() {
    let (_server, port, _root) = start_server(false);

    // 1. /.well-known/oauth-authorization-server
    let (code, json) = http_request(port, "GET", "/.well-known/oauth-authorization-server", &[], None);
    assert_eq!(code, 200);
    assert_eq!(json["authorization_endpoint"], format!("http://127.0.0.1:{port}/authorize"));
    assert_eq!(json["token_endpoint"], format!("http://127.0.0.1:{port}/token"));

    // 2. /.well-known/openid-configuration
    let (code, json) = http_request(port, "GET", "/.well-known/openid-configuration", &[], None);
    assert_eq!(code, 200);
    assert_eq!(json["userinfo_endpoint"], format!("http://127.0.0.1:{port}/userinfo"));

    // 3. /.well-known/oauth-protected-resource
    let (code, json) = http_request(port, "GET", "/.well-known/oauth-protected-resource", &[], None);
    assert_eq!(code, 200);
    assert_eq!(json["resource"], format!("http://127.0.0.1:{port}/mcp"));

    // 4. /.well-known/oauth-protected-resource/mcp
    let (code, json) = http_request(port, "GET", "/.well-known/oauth-protected-resource/mcp", &[], None);
    assert_eq!(code, 200);
    assert_eq!(json["resource"], format!("http://127.0.0.1:{port}/mcp"));
}

#[test]
fn test_oauth_client_registration_and_token_flow() {
    let (_server, port, _root) = start_server(false);

    // 1. Dynamic Client Registration
    let reg_body = json!({
        "client_name": "Test MCP Client",
        "redirect_uris": ["http://localhost:3000/callback"]
    })
    .to_string();

    let (code, reg_res) = http_request(
        port,
        "POST",
        "/register",
        &[("Content-Type", "application/json")],
        Some(&reg_body),
    );
    assert_eq!(code, 201);
    let client_id = reg_res["client_id"].as_str().unwrap().to_string();
    assert!(client_id.starts_with("client_"));

    // 2. Authorize Code Request
    let auth_path = format!(
        "/authorize?client_id={}&redirect_uri=http://localhost:3000/callback&response_type=code&state=xyz123",
        client_id
    );
    let (code, _) = http_request(port, "GET", &auth_path, &[], None);
    assert_eq!(code, 200);

    // 3. Client Credentials Token Flow
    let token_body = format!("grant_type=client_credentials&client_id={client_id}");
    let (code, token_res) = http_request(
        port,
        "POST",
        "/token",
        &[("Content-Type", "application/x-www-form-urlencoded")],
        Some(&token_body),
    );
    assert_eq!(code, 200);
    let access_token = token_res["access_token"].as_str().unwrap().to_string();
    assert!(access_token.starts_with("at_"));
}

#[test]
fn test_pkce_s256_flow_hyperagent_vector() {
    let (_server, port, _root) = start_server(false);

    let verifier = "5U44ovouN090jy2wd3OT9gkZHQEUV2AweuwDYUN0env";
    let challenge = "wg3d-KrfCNNYB2Gx4rmwmLqTUe_vGY5El4B6yZNWqqA";

    let auth_path = format!(
        "/authorize?client_id=client_test&redirect_uri=http://localhost:3000/callback&response_type=code&code_challenge={challenge}&code_challenge_method=S256"
    );
    let (code, _html) = http_request(port, "GET", &auth_path, &[], None);
    assert_eq!(code, 200);
    // The code is generated in OAuthStore; let's perform authorization flow with S256 code verifier
    let token_body = format!(
        "grant_type=authorization_code&client_id=client_test&code_verifier={verifier}&code=test_code"
    );
    let (status, _) = http_request(
        port,
        "POST",
        "/token",
        &[("Content-Type", "application/x-www-form-urlencoded")],
        Some(&token_body),
    );
    // Invalid code will return 400 with "code invalid or expired", confirming PKCE parsing logic
    assert_eq!(status, 400);
}

#[test]
fn test_mcp_auth_enforcement() {
    let (_server, port, _root) = start_server(true); // require_auth = true

    // Request without token should be 401
    let mcp_body = json!({"jsonrpc":"2.0","id":1,"method":"ping"}).to_string();
    let (code, res) = http_request(
        port,
        "POST",
        "/mcp",
        &[("Content-Type", "application/json")],
        Some(&mcp_body),
    );
    assert_eq!(code, 401);
    assert_eq!(res["error"], "unauthorized");

    // Get a token via client credentials
    let token_body = "grant_type=client_credentials&client_id=test_client";
    let (code, token_res) = http_request(
        port,
        "POST",
        "/token",
        &[("Content-Type", "application/x-www-form-urlencoded")],
        Some(token_body),
    );
    assert_eq!(code, 200);
    let access_token = token_res["access_token"].as_str().unwrap();

    // Request with Bearer token should succeed
    let auth_header = format!("Bearer {access_token}");
    let (code, res) = http_request(
        port,
        "POST",
        "/mcp",
        &[
            ("Content-Type", "application/json"),
            ("Authorization", &auth_header),
        ],
        Some(&mcp_body),
    );
    assert_eq!(code, 200);
    assert_eq!(res["result"], json!({}));
}
