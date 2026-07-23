//! End-to-end MCP HTTP catalog tests.

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

fn start() -> (Server, u16, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("sample.txt"), "alpha\nbeta\n").unwrap();
    let port = 20_000
        + (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
            % 20_000) as u16;
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

fn rpc(port: u16, body: Value) -> Value {
    let bytes = serde_json::to_vec(&body).unwrap();
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    write!(stream, "POST /mcp HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", bytes.len()).unwrap();
    stream.write_all(&bytes).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap() + 4;
    serde_json::from_slice(&response[split..]).unwrap()
}

fn call(port: u16, id: u64, name: &str, arguments: Value) -> Value {
    rpc(port, json!({"jsonrpc":"2.0","id":id,"method":"tools/call","params":{"name":name,"arguments":arguments}}))["result"].clone()
}

#[test]
fn initialize_catalog_and_calls_stay_consistent() {
    let (_server, port, root) = start();
    let initialized = rpc(
        port,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}),
    );
    assert_eq!(initialized["result"]["protocolVersion"], "2025-06-18");
    let listed = rpc(
        port,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );
    let names: Vec<&str> = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert!(!names.contains(&"stat"));
    for expected in ["get_capabilities", "list_tree", "apply_patch", "file_info"] {
        assert!(names.contains(&expected));
    }
    let capabilities = call(port, 3, "get_capabilities", json!({}));
    let reported: Vec<&str> = capabilities["structuredContent"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert_eq!(reported, names);
    let serialized = serde_json::to_string(&capabilities["structuredContent"])
        .unwrap()
        .to_lowercase();
    assert!(!serialized.contains("environment"));
    assert!(!serialized.contains("127.0.0.1"));
    let tree = call(port, 4, "list_tree", json!({"path":root.path(),"depth":1}));
    assert_eq!(tree["isError"], false);
    let info = call(
        port,
        5,
        "file_info",
        json!({"path":root.path().join("sample.txt"),"includeHash":true}),
    );
    assert_eq!(info["structuredContent"]["kind"], "file");
    let nested = root
        .path()
        .join("generated")
        .join("reports")
        .join("result.txt");
    let written = call(
        port,
        8,
        "write_file",
        json!({"path":nested,"content":"ready","createParents":true}),
    );
    assert_eq!(written["isError"], false);
    assert_eq!(written["structuredContent"]["written"], 5);
    assert_eq!(
        written["structuredContent"]["createdDirectories"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(fs::read_to_string(&nested).unwrap(), "ready");
    let patch = "--- a/sample.txt\n+++ b/sample.txt\n@@ -1,1 +1,1 @@\n-alpha\n+ALPHA\n";
    let dry = call(
        port,
        6,
        "apply_patch",
        json!({"path":root.path().join("sample.txt"),"patch":patch,"dryRun":true}),
    );
    assert_eq!(dry["structuredContent"]["dryRun"], true);
    assert_eq!(
        fs::read_to_string(root.path().join("sample.txt")).unwrap(),
        "alpha\nbeta\n"
    );
    let bad = call(
        port,
        7,
        "file_info",
        json!({"path":root.path().join("missing")}),
    );
    assert_eq!(bad["isError"], true);
    assert!(bad["structuredContent"]["error"]["code"].is_string());
}
