use fs_mcp_rs::protocol::Tool;
use serde_json::{Value, json};

/// Builds the stable MCP tool catalog advertised to clients.
pub(crate) fn tools() -> Vec<Tool> {
    vec![
        tool(
            "get_capabilities",
            "Report sanitized effective server constraints",
            json!({"type":"object","properties":{},"additionalProperties":false}),
        ),
        tool(
            "list_directory",
            "List direct children of a directory",
            path_schema(),
        ),
        tool(
            "read_file",
            "Read a bounded byte range as UTF-8 text",
            json!({"type":"object","properties":{"path":{"type":"string"},"offset":{"type":"integer","minimum":0,"default":0},"length":{"type":"integer","minimum":1}},"required":["path","length"]}),
        ),
        tool(
            "write_file",
            "Atomically create or replace a UTF-8 file. For a new nested path, set createParents=true so missing parent directories are created safely.",
            json!({"type":"object","properties":{"path":{"type":"string","minLength":1,"description":"Destination file path inside an allowed root."},"content":{"type":"string","description":"Complete UTF-8 file content."},"createParents":{"type":"boolean","default":false,"description":"Set true when parent directories may not exist. Missing parents are created after access-policy validation; leave false to require an existing parent."}},"required":["path","content"],"additionalProperties":false}),
        ),
        tool(
            "search_files",
            "Find paths whose file name contains a pattern",
            json!({"type":"object","properties":{"path":{"type":"string"},"pattern":{"type":"string","minLength":1}},"required":["path","pattern"]}),
        ),
        tool(
            "search_content",
            "Search text files using literal text or regex",
            json!({"type":"object","properties":{"path":{"type":"string"},"pattern":{"type":"string","minLength":1},"literal":{"type":"boolean","default":true}},"required":["path","pattern"]}),
        ),
        tool(
            "list_tree",
            "List a bounded recursive directory tree",
            json!({"type":"object","properties":{"path":{"type":"string","minLength":1},"depth":{"type":"integer","minimum":0,"default":2},"include":{"type":"array","items":{"type":"string"}},"exclude":{"type":"array","items":{"type":"string"}},"maxEntries":{"type":"integer","minimum":1},"cursor":{"type":["string","null"]}},"required":["path"],"additionalProperties":false}),
        ),
        tool(
            "file_info",
            "Get rich file or directory metadata",
            json!({"type":"object","properties":{"path":{"type":"string","minLength":1},"includeHash":{"type":"boolean","default":false}},"required":["path"],"additionalProperties":false}),
        ),
        tool(
            "apply_patch",
            "Atomically apply a single-file unified text diff; dryRun is non-mutating",
            json!({"type":"object","properties":{"path":{"type":"string","minLength":1},"patch":{"type":"string"},"expectedBlake3":{"type":["string","null"]},"dryRun":{"type":"boolean","default":false}},"required":["path","patch"],"additionalProperties":false}),
        ),
        tool("create_directory", "Create one directory", path_schema()),
        tool("remove", "Remove a file or empty directory", path_schema()),
        tool("hash_file", "Calculate a BLAKE3 file hash", path_schema()),
        tool(
            "move",
            "Move a file or directory without overwriting",
            json!({"type":"object","properties":{"source":{"type":"string"},"destination":{"type":"string"}},"required":["source","destination"]}),
        ),
        tool(
            "edit_text",
            "Replace an exact number of text matches atomically",
            json!({"type":"object","properties":{"path":{"type":"string"},"old":{"type":"string","minLength":1},"new":{"type":"string"},"expected":{"type":"integer","minimum":1}},"required":["path","old","new","expected"]}),
        ),
        dangerous_tool(
            "terminal_start",
            "Start a persistent terminal command and return a session ID immediately.",
            json!({"type":"object","properties":{"command":{"type":"string","minLength":1},"cwd":{"type":"string"},"timeoutMs":{"type":"integer","minimum":1}},"required":["command"]}),
        ),
        tool(
            "terminal_read",
            "Read new output from a persistent terminal session without restarting it.",
            json!({"type":"object","properties":{"sessionId":{"type":"string","minLength":1},"cursor":{"type":"integer","minimum":0,"default":0},"waitMs":{"type":"integer","minimum":0,"default":0},"maxBytes":{"type":"integer","minimum":1}},"required":["sessionId"]}),
        ),
        dangerous_tool(
            "terminal_write",
            "Write UTF-8 input to a running terminal session.",
            json!({"type":"object","properties":{"sessionId":{"type":"string","minLength":1},"data":{"type":"string"}},"required":["sessionId","data"]}),
        ),
        dangerous_tool(
            "terminal_close_stdin",
            "Close stdin for a running terminal session.",
            json!({"type":"object","properties":{"sessionId":{"type":"string","minLength":1}},"required":["sessionId"]}),
        ),
        dangerous_tool(
            "terminal_kill",
            "Terminate a running terminal session and its process tree.",
            json!({"type":"object","properties":{"sessionId":{"type":"string","minLength":1}},"required":["sessionId"]}),
        ),
        dangerous_tool(
            "terminal_close",
            "Remove a completed terminal session and its retained output.",
            json!({"type":"object","properties":{"sessionId":{"type":"string","minLength":1}},"required":["sessionId"]}),
        ),
        dangerous_tool(
            "run_command",
            "Run an arbitrary terminal command and wait for completion. This may modify files, access the network, or execute other programs.",
            json!({"type":"object","properties":{"command":{"type":"string","minLength":1},"cwd":{"type":"string"},"timeoutMs":{"type":"integer","minimum":1}},"required":["command"]}),
        ),
    ]
}

fn tool(name: &'static str, description: &'static str, input_schema: Value) -> Tool {
    Tool {
        name,
        description,
        input_schema,
        annotations: Some(
            json!({"title": description, "readOnlyHint": matches!(name, "get_capabilities"|"list_directory"|"list_tree"|"read_file"|"search_files"|"search_content"|"file_info"|"hash_file"|"terminal_read"), "destructiveHint": matches!(name, "remove"|"apply_patch"), "idempotentHint": matches!(name, "get_capabilities"|"list_directory"|"list_tree"|"read_file"|"search_files"|"search_content"|"file_info"|"hash_file"|"terminal_read"), "openWorldHint": false}),
        ),
    }
}

fn dangerous_tool(name: &'static str, description: &'static str, input_schema: Value) -> Tool {
    Tool {
        name,
        description,
        input_schema,
        annotations: Some(json!({
            "title": description,
            "readOnlyHint": false,
            "destructiveHint": true,
            "idempotentHint": false,
            "openWorldHint": true
        })),
    }
}

fn path_schema() -> Value {
    json!({"type":"object","properties":{"path":{"type":"string","minLength":1}},"required":["path"]})
}
