//! Formatting utilities for CLI guidance, tools catalog, client snippets, and config validation summaries.

use crate::protocol::Tool;
use crate::settings::Settings;
use serde::Serialize;
use std::path::Path;

/// A client-ready MCP configuration snippet.
#[derive(Clone, Debug, Serialize)]
pub struct ClientSnippet {
    /// Stable identifier used by graphical clients.
    pub id: &'static str,
    /// Human-readable client or transport name.
    pub title: &'static str,
    /// JSON configuration text.
    pub content: String,
}

fn normalized_path(path: &Path) -> String {
    let abs = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let mut value = abs.display().to_string().replace('\\', "/");
    if let Some(stripped) = value.strip_prefix("//?/") {
        value = stripped.to_owned();
    }
    value
}

/// Builds the STDIO and HTTP snippets shared by the CLI and desktop app.
pub fn client_snippets(
    config_path: &Path,
    exe_name: &str,
    server_name: &str,
    host: &str,
    port: u16,
) -> Vec<ClientSnippet> {
    let path = normalized_path(config_path);
    let display_host = match host {
        "0.0.0.0" | "::" => "127.0.0.1",
        value => value,
    };
    let url_host = if display_host.contains(':') {
        format!("[{display_host}]")
    } else {
        display_host.to_owned()
    };
    let stdio = serde_json::json!({
        "mcpServers": {
            server_name: {
                "command": exe_name,
                "args": ["serve", "--stdio", "--config", path]
            }
        }
    });
    let http = serde_json::json!({
        "mcpServers": {
            server_name: {
                "url": format!("http://{url_host}:{port}/mcp")
            }
        }
    });
    vec![
        ClientSnippet {
            id: "claude-desktop-stdio",
            title: "Claude Desktop (STDIO)",
            content: serde_json::to_string_pretty(&stdio).expect("JSON value is serializable"),
        },
        ClientSnippet {
            id: "mcp-http",
            title: "Cursor / MCP HTTP",
            content: serde_json::to_string_pretty(&http).expect("JSON value is serializable"),
        },
    ]
}

/// Generates annotated default TOML configuration string.
pub fn default_config_toml() -> &'static str {
    include_str!("../configs/default.toml")
}

/// Formats and prints a list of all available MCP tools provided by the server.
pub fn print_tools_catalog(tools: &[Tool]) {
    println!("\n================================================================================");
    println!(
        "                    fs-mcp-rs Available MCP Tools Catalog                            "
    );
    println!("================================================================================");
    println!("{:<22} {:<14} DESCRIPTION", "TOOL NAME", "SAFETY LEVEL");
    println!("{:-<22} {:-<14} {:-<45}", "", "", "");

    for tool in tools {
        let badge = if let Some(ref ann) = tool.annotations {
            if ann
                .get("readOnlyHint")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                "[READ-ONLY]"
            } else if ann
                .get("openWorldHint")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                "[DANGEROUS]"
            } else if ann
                .get("destructiveHint")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
            {
                "[DESTRUCTIVE]"
            } else {
                "[MUTATING]"
            }
        } else {
            "[STANDARD]"
        };

        println!("{:<22} {:<14} {}", tool.name, badge, tool.description);
    }

    println!("\nTotal tools available: {}", tools.len());
    println!("For detailed schema parameters, connect an MCP client or read documentation.\n");
}

/// Generates and prints JSON configuration snippets for AI clients (Claude Desktop, Cursor, etc.).
pub fn print_client_snippets(config_path: &Path, exe_name: &str) {
    let snippets = client_snippets(config_path, exe_name, "fs-mcp-rs", "127.0.0.1", 8000);

    println!("\n================================================================================");
    println!("                MCP Client Integration Snippets for fs-mcp-rs                   ");
    println!("================================================================================");

    println!("\n[1] Claude Desktop (stdio / command execution mode)");
    println!("Add to your `claude_desktop_config.json`:");
    println!("--------------------------------------------------------------------------------");
    println!("{}", snippets[0].content);
    println!("--------------------------------------------------------------------------------");

    println!("\n[2] Cursor / VS Code / Roo Code / Windsurf (HTTP SSE mode)");
    println!("Add to your `mcp.json` or client settings:");
    println!("--------------------------------------------------------------------------------");
    println!("{}", snippets[1].content);
    println!("--------------------------------------------------------------------------------");
    println!(
        "\nTip: Make sure `fs-mcp-rs` is in your PATH or specify the full path to the executable.\n"
    );
}

/// Prints a human-readable summary of settings and security boundaries.
pub fn print_config_summary(settings: &Settings, config_path: &Path) {
    println!("\n================================================================================");
    println!("                    fs-mcp-rs Configuration Summary                             ");
    println!("================================================================================");
    println!("Config File Path : {}", config_path.display());
    println!(
        "Server Address   : http://{}:{}",
        settings.server.host, settings.server.port
    );
    println!(
        "Read-Only Mode   : {}",
        if settings.filesystem.read_only {
            "YES (Safe)"
        } else {
            "NO (Mutating operations allowed)"
        }
    );
    println!(
        "Terminal Exec    : {}",
        if settings.terminal.enabled {
            "ENABLED (Arbitrary command execution allowed)"
        } else {
            "DISABLED (Safe)"
        }
    );
    println!(
        "Tool Call Logging: {}",
        if settings.server.log_tools {
            "ENABLED ([OK]/[WARN] per tool execution)"
        } else {
            "DISABLED (Quiet mode)"
        }
    );
    println!(
        "OAuth Enabled    : {}",
        if settings.oauth.enabled { "YES" } else { "NO" }
    );
    println!(
        "OAuth Require Auth: {}",
        if settings.oauth.require_auth {
            "YES (Bearer token required)"
        } else {
            "NO"
        }
    );

    println!("\nAllowed Filesystem Roots:");
    for (idx, root) in settings.filesystem.roots.iter().enumerate() {
        println!("  [{}] {}", idx + 1, root.display());
    }

    println!("\nResource Limits:");
    println!(
        "  • Server Max Concurrency: {}",
        settings.server.max_concurrency
    );
    println!(
        "  • Max Read Bytes        : {} MB",
        settings.filesystem.max_read_bytes / (1024 * 1024)
    );
    println!(
        "  • Max Write Bytes       : {} MB",
        settings.filesystem.max_write_bytes / (1024 * 1024)
    );
    println!(
        "  • Search Worker Threads : {}",
        settings.search.worker_threads
    );
    println!(
        "  • Search Max Results    : {}",
        settings.search.max_results
    );
    println!("--------------------------------------------------------------------------------\n");
}

/// Prints a friendly error banner when no configuration file is found and non-interactive.
pub fn print_no_config_banner() {
    eprintln!("\n================================================================================");
    eprintln!("                  fs-mcp-rs Configuration Needed                                ");
    eprintln!("================================================================================");
    eprintln!("No configuration file was found or specified.");
    eprintln!();
    eprintln!("Quick Steps to Get Started:");
    eprintln!("  1. Run the interactive setup wizard:");
    eprintln!("     $ fs-mcp-rs init");
    eprintln!();
    eprintln!("  2. Or generate a default configuration file:");
    eprintln!("     $ fs-mcp-rs config print-example > config.toml");
    eprintln!();
    eprintln!("  3. Or start the server specifying an existing config file:");
    eprintln!("     $ fs-mcp-rs serve --config path/to/config.toml");
    eprintln!();
    eprintln!("  4. To inspect available MCP tools:");
    eprintln!("     $ fs-mcp-rs tools");
    eprintln!("================================================================================");
}
