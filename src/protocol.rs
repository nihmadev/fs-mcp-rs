//! JSON-RPC 2.0 and Model Context Protocol wire types.
//!
//! This module deliberately contains only transport-neutral data structures and
//! protocol-version negotiation. HTTP routing and tool dispatch live in the
//! binary crate.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The protocol version selected when a client does not request a supported version.
pub const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";
/// MCP protocol versions understood by this server, newest first.
pub const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26"];

#[derive(Debug, Deserialize)]
/// A decoded JSON-RPC 2.0 request or notification.
pub struct Request {
    /// The JSON-RPC version; valid requests use `"2.0"`.
    pub jsonrpc: String,
    /// Correlation identifier, or `None` for a notification.
    pub id: Option<Value>,
    /// The requested RPC method name.
    pub method: String,
    #[serde(default)]
    /// Method parameters; absent parameters deserialize as JSON `null`.
    pub params: Value,
}

#[derive(Debug, Serialize)]
/// A JSON-RPC 2.0 success or error response.
pub struct Response {
    /// The JSON-RPC version, always `"2.0"`.
    pub jsonrpc: &'static str,
    /// Correlation identifier, or `None` for a notification.
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Success payload; omitted from error responses.
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Error payload; omitted from successful responses.
    pub error: Option<RpcError>,
}

#[derive(Debug, Serialize)]
/// A JSON-RPC error object.
pub struct RpcError {
    /// The JSON-RPC numeric error code.
    pub code: i32,
    /// Human-readable error description.
    pub message: String,
}

impl Response {
    /// Constructs a successful response for `id`.
    pub fn ok(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Constructs an error response for `id`.
    pub fn error(id: Option<Value>, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
            }),
        }
    }
}

#[derive(Debug, Serialize)]
/// An MCP tool definition returned by `tools/list`.
pub struct Tool {
    /// Stable tool name used by `tools/call`.
    pub name: &'static str,
    /// Human-readable description presented to MCP clients.
    pub description: &'static str,
    #[serde(rename = "inputSchema")]
    /// JSON Schema describing the tool arguments.
    pub input_schema: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Optional MCP behavioral hints, such as destructive-operation metadata.
    pub annotations: Option<Value>,
}

/// Selects `requested` when supported, otherwise returns [`LATEST_PROTOCOL_VERSION`].
pub fn negotiate_protocol(requested: Option<&str>) -> &'static str {
    requested
        .and_then(|value| {
            SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .copied()
                .find(|v| *v == value)
        })
        .unwrap_or(LATEST_PROTOCOL_VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_success_without_error() {
        let value =
            serde_json::to_value(Response::ok(Some(json!(1)), json!({"pong": true}))).unwrap();
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["result"]["pong"], true);
        assert!(value.get("error").is_none());
    }

    #[test]
    fn negotiates_known_and_unknown_versions() {
        assert_eq!(negotiate_protocol(Some("2025-03-26")), "2025-03-26");
        assert_eq!(
            negotiate_protocol(Some("1900-01-01")),
            LATEST_PROTOCOL_VERSION
        );
    }
}
