//! Tool call dispatching, concurrency control, and tool execution logging.
//!
//! Maps tool calls to blocking implementations, enforces resource permits, and prints
//! concise execution logs (`[OK] tool_name ...` / `[WARN] tool_name ...`) when `server.log_tools` is enabled.

use super::catalog::tools;
use crate::app::App;
use fs_mcp_rs::{
    patch::{ApplyPatchRequest, apply_patch},
    protocol::SUPPORTED_PROTOCOL_VERSIONS,
    tree::ListTreeRequest,
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{path::PathBuf, time::Instant};

#[derive(Deserialize)]
/// Decoded `tools/call` parameters.
struct ToolCall {
    name: String,
    #[serde(default)]
    arguments: Value,
}

pub(crate) fn tool_error(message: String) -> Value {
    let code = error_code(&message);
    json!({"content":[{"type":"text","text":message}],"structuredContent":{"error":{"code":code,"message":message}},"isError":true})
}

fn error_code(message: &str) -> &'static str {
    if message.contains("write operations are disabled") {
        "READ_ONLY"
    } else if message.contains("outside allowed roots") {
        "OUTSIDE_ALLOWED_ROOT"
    } else if message.contains("symbolic links are disabled") {
        "SYMLINK_DISALLOWED"
    } else if message.contains("valid UTF-8") {
        "INVALID_UTF8"
    } else if message.contains("context does not match") {
        "PATCH_CONTEXT_MISMATCH"
    } else if message.contains("BLAKE3 does not match") {
        "HASH_CONFLICT"
    } else if message.contains("unsupported patch") {
        "UNSUPPORTED_PATCH_OPERATION"
    } else if message.contains("patch") {
        "INVALID_PATCH"
    } else if message.contains("limit") || message.contains("exceeds") {
        "LIMIT_EXCEEDED"
    } else if message.contains("cannot be resolved") || message.contains("not found") {
        "PATH_NOT_FOUND"
    } else {
        "TOOL_ERROR"
    }
}

/// Applies per-class concurrency limits and executes blocking tools off-runtime.
pub(crate) async fn call_tool(app: &App, params: Value) -> Result<Value, String> {
    let total_started = Instant::now();
    let call: ToolCall = match serde_json::from_value(params) {
        Ok(c) => c,
        Err(e) => {
            let err_msg = e.to_string();
            if app.settings.server.log_tools {
                println!("[WARN] unknown_tool - INVALID_PARAMS: {} (0 ms)", err_msg);
            }
            return Err(err_msg);
        }
    };

    let tool_name = call.name.clone();
    let arg_summary = summarize_arguments(&call.arguments);

    let permits = if tool_name.starts_with("search_") {
        app.search_permits.clone()
    } else {
        app.io_permits.clone()
    };
    let queue_started = Instant::now();
    let _permit = match permits.acquire_owned().await {
        Ok(p) => p,
        Err(_) => {
            let err_msg = "server is shutting down".to_string();
            if app.settings.server.log_tools {
                let ms = total_started.elapsed().as_millis();
                println!(
                    "[WARN] {} {} - SERVER_SHUTDOWN: {} ({} ms)",
                    tool_name, arg_summary, err_msg, ms
                );
            }
            return Err(err_msg);
        }
    };
    let queue_us = queue_started.elapsed().as_micros() as u64;
    let execution_started = Instant::now();
    let app_clone = app.clone();
    let res = tokio::task::spawn_blocking(move || call_tool_blocking(&app_clone, call)).await;

    let execution_us = execution_started.elapsed().as_micros() as u64;
    let total_us = total_started.elapsed().as_micros() as u64;
    let total_ms = total_started.elapsed().as_millis();

    match res {
        Ok(Ok(mut result)) => {
            result["_meta"] = json!({
                "totalDurationUs": total_us,
                "queueDurationUs": queue_us,
                "executionDurationUs": execution_us
            });
            if app.settings.server.log_tools {
                if arg_summary.is_empty() {
                    println!("[OK] {} ({} ms)", tool_name, total_ms);
                } else {
                    println!("[OK] {} {} ({} ms)", tool_name, arg_summary, total_ms);
                }
            }
            Ok(result)
        }
        Ok(Err(err_msg)) => {
            if app.settings.server.log_tools {
                let code = error_code(&err_msg);
                if arg_summary.is_empty() {
                    println!(
                        "[WARN] {} - {}: {} ({} ms)",
                        tool_name, code, err_msg, total_ms
                    );
                } else {
                    println!(
                        "[WARN] {} {} - {}: {} ({} ms)",
                        tool_name, arg_summary, code, err_msg, total_ms
                    );
                }
            }
            Err(err_msg)
        }
        Err(join_err) => {
            let err_msg = format!("blocking task failed: {join_err}");
            if app.settings.server.log_tools {
                println!(
                    "[WARN] {} {} - INTERNAL_ERROR: {} ({} ms)",
                    tool_name, arg_summary, err_msg, total_ms
                );
            }
            Err(err_msg)
        }
    }
}

fn summarize_arguments(args: &Value) -> String {
    if let Some(obj) = args.as_object() {
        for key in &["path", "command", "pattern", "source"] {
            if let Some(val) = obj.get(*key).and_then(Value::as_str) {
                let truncated = if val.len() > 40 {
                    format!("{}...", &val[..37])
                } else {
                    val.to_string()
                };
                return format!("{}={:?}", key, truncated);
            }
        }
    }
    String::new()
}

/// Dispatches one validated tool call on a blocking worker thread.
fn call_tool_blocking(app: &App, call: ToolCall) -> Result<Value, String> {
    let arguments = call.arguments;
    let text = |key: &str| {
        arguments
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("missing string argument: {key}"))
    };
    let output = match call.name.as_str() {
        "get_capabilities" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields)]
            struct Empty {}
            let _: Empty = serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
            let cfg = &app.settings;
            serde_json::to_string(&json!({"server":{"name":"fs-mcp-rs","version":env!("CARGO_PKG_VERSION")},"protocolVersions":SUPPORTED_PROTOCOL_VERSIONS,"osFamily":std::env::consts::FAMILY,"roots":cfg.filesystem.roots.iter().map(|p| fs_mcp_rs::security::display_path(p)).collect::<Vec<_>>(),"filesystem":{"readOnly":cfg.filesystem.read_only,"followLinks":cfg.filesystem.follow_links,"maxReadBytes":cfg.filesystem.max_read_bytes,"maxWriteBytes":cfg.filesystem.max_write_bytes},"search":{"maxResults":cfg.search.max_results,"maxConcurrency":cfg.search.max_concurrency},"tree":{"maxDepth":cfg.filesystem.tree_max_depth,"maxEntries":cfg.filesystem.tree_max_entries,"maxWarnings":cfg.filesystem.tree_max_warnings},"terminal":{"enabled":cfg.terminal.enabled,"defaultTimeoutMs":cfg.terminal.default_timeout_ms,"maxTimeoutMs":cfg.terminal.max_timeout_ms,"maxOutputBytes":cfg.terminal.max_output_bytes,"maxReadBytes":cfg.terminal.max_read_bytes,"maxWaitMs":cfg.terminal.max_wait_ms,"sessionRetentionMs":cfg.terminal.session_retention_ms,"maxConcurrency":cfg.terminal.max_concurrency},"tools":tools().iter().map(|t|t.name).collect::<Vec<_>>() })).map_err(|e| e.to_string())?
        }
        "list_tree" => {
            let req: ListTreeRequest =
                serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
            serde_json::to_string(&app.tree.list(&req).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?
        }
        "file_info" => {
            #[derive(Deserialize)]
            #[serde(deny_unknown_fields, rename_all = "camelCase")]
            struct A {
                path: PathBuf,
                #[serde(default)]
                include_hash: bool,
            }
            let req: A = serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
            serde_json::to_string(
                &app.fs
                    .file_info(&req.path, req.include_hash)
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?
        }
        "apply_patch" => {
            let req: ApplyPatchRequest =
                serde_json::from_value(arguments.clone()).map_err(|e| e.to_string())?;
            serde_json::to_string(
                &apply_patch(
                    &app.fs,
                    &req,
                    app.settings.filesystem.patch_max_bytes,
                    app.settings.filesystem.patch_preview_bytes,
                )
                .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?
        }
        "list_directory" => serde_json::to_string(&json!({
            "entries": app.fs
                .list(&PathBuf::from(text("path")?))
                .map_err(|e| e.to_string())?
        }))
        .map_err(|e| e.to_string())?,
        "read_file" => {
            let length = arguments
                .get("length")
                .and_then(Value::as_u64)
                .ok_or_else(|| "missing integer argument: length".to_owned())?
                as usize;
            let bytes = app
                .fs
                .read(
                    &PathBuf::from(text("path")?),
                    arguments.get("offset").and_then(Value::as_u64).unwrap_or(0),
                    length,
                )
                .map_err(|e| e.to_string())?;
            match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
            }
        }
        "write_file" => {
            let content = text("content")?;
            let create_parents = arguments
                .get("createParents")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let (hash, created_directories) = app
                .fs
                .write_with_parents(
                    &PathBuf::from(text("path")?),
                    content.as_bytes(),
                    create_parents,
                )
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&json!({
                "written": content.len(),
                "blake3": hash,
                "createdDirectories": created_directories
            }))
            .map_err(|e| e.to_string())?
        }
        "search_files" => serde_json::to_string(&json!({
            "paths": app.search
                .files(&PathBuf::from(text("path")?), &text("pattern")?)
                .map_err(|e| e.to_string())?
        }))
        .map_err(|e| e.to_string())?,
        "search_content" => serde_json::to_string(&json!({
            "matches": app.search
                .content(
                    &PathBuf::from(text("path")?),
                    &text("pattern")?,
                    arguments
                        .get("literal")
                        .and_then(Value::as_bool)
                        .unwrap_or(true),
                )
                .map_err(|e| e.to_string())?
        }))
        .map_err(|e| e.to_string())?,

        "create_directory" => {
            app.fs
                .create_directory(&PathBuf::from(text("path")?))
                .map_err(|e| e.to_string())?;
            "{\"created\":true}".into()
        }
        "remove" => {
            app.fs
                .remove(&PathBuf::from(text("path")?))
                .map_err(|e| e.to_string())?;
            "{\"removed\":true}".into()
        }
        "hash_file" => app
            .fs
            .hash(&PathBuf::from(text("path")?))
            .map_err(|e| e.to_string())?,
        "move" => {
            app.fs
                .move_path(
                    &PathBuf::from(text("source")?),
                    &PathBuf::from(text("destination")?),
                )
                .map_err(|e| e.to_string())?;
            "{\"moved\":true}".into()
        }
        "edit_text" => app
            .fs
            .edit(
                &PathBuf::from(text("path")?),
                &text("old")?,
                &text("new")?,
                arguments
                    .get("expected")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| "missing integer argument: expected".to_owned())?
                    as usize,
            )
            .map_err(|e| e.to_string())?,
        "terminal_start" => {
            let command = text("command")?;
            let cwd = arguments
                .get("cwd")
                .and_then(Value::as_str)
                .map(PathBuf::from);
            let result = app
                .terminal
                .start(
                    &command,
                    cwd.as_deref(),
                    arguments.get("timeoutMs").and_then(Value::as_u64),
                )
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        }
        "terminal_read" => {
            let result = app
                .terminal
                .read(
                    &text("sessionId")?,
                    arguments.get("cursor").and_then(Value::as_u64).unwrap_or(0),
                    arguments.get("waitMs").and_then(Value::as_u64),
                    arguments
                        .get("maxBytes")
                        .and_then(Value::as_u64)
                        .map(|value| value as usize),
                )
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        }
        "terminal_write" => {
            let result = app
                .terminal
                .write(&text("sessionId")?, text("data")?.as_bytes())
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        }
        "terminal_close_stdin" => {
            app.terminal
                .close_stdin(&text("sessionId")?)
                .map_err(|e| e.to_string())?;
            "{\"closed\":true}".into()
        }
        "terminal_kill" => {
            let result = app
                .terminal
                .kill(&text("sessionId")?)
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        }
        "terminal_close" => {
            app.terminal
                .close(&text("sessionId")?)
                .map_err(|e| e.to_string())?;
            "{\"closed\":true}".into()
        }
        "run_command" => {
            let command = text("command")?;
            let cwd = arguments
                .get("cwd")
                .and_then(Value::as_str)
                .map(PathBuf::from);
            let result = app
                .terminal
                .run(
                    &command,
                    cwd.as_deref(),
                    arguments.get("timeoutMs").and_then(Value::as_u64),
                )
                .map_err(|e| e.to_string())?;
            serde_json::to_string(&result).map_err(|e| e.to_string())?
        }
        _ => return Err(format!("unknown tool: {}", call.name)),
    };
    let mut result = json!({
        "content": [{"type": "text", "text": output}],
        "isError": false
    });
    if let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&output) {
        result["structuredContent"] = Value::Object(object);
    }
    Ok(result)
}
