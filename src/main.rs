use anyhow::Result;
use clap::Parser;
use fs_mcp_rs::cli_format::{
    default_config_toml, print_client_snippets, print_config_summary, print_tools_catalog,
};
use fs_mcp_rs::protocol::Tool;
use fs_mcp_rs::settings::{Cli, Commands, ConfigCommands, Settings, resolve_config_path};
use fs_mcp_rs::wizard::run_wizard;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

mod app;
mod handler;
mod oauth;
mod server;
mod stdio;
mod tools;

/// Binary entry point for `fs-mcp-rs`.
///
/// Directs structured tracing logs to `stderr` to keep `stdout` clear for JSON-RPC 2.0
/// when running in STDIO transport mode. Automatically detects interactive terminal sessions
/// vs non-interactive pipe execution to select STDIO or HTTP transport modes accordingly.
#[tokio::main]
async fn main() -> Result<()> {
    // Direct all tracing logs to stderr so stdout remains clean for JSON-RPC in STDIO mode
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("info")
        .init();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { output }) => {
            run_wizard(&output)?;
            Ok(())
        }
        Some(Commands::Tools) => {
            let catalog_tools: Vec<Tool> = tools::tools();
            print_tools_catalog(&catalog_tools);
            Ok(())
        }
        Some(Commands::Config { command }) => match command {
            ConfigCommands::PrintExample => {
                println!("{}", default_config_toml());
                Ok(())
            }
            ConfigCommands::Snippet { config } => {
                let path = resolve_config_path(config.as_deref())
                    .unwrap_or_else(|| PathBuf::from("config.toml"));
                print_client_snippets(&path, "fs-mcp-rs");
                Ok(())
            }
            ConfigCommands::Check { config } => {
                let resolved = resolve_config_path(config.as_deref()).ok_or_else(|| {
                    anyhow::anyhow!("No configuration file found to check. Pass --config <FILE> or run `fs-mcp-rs init`.")
                })?;
                let settings = Settings::load(&resolved)?;
                print_config_summary(&settings, &resolved);
                println!("[OK] Configuration file structure and limits are valid.");
                Ok(())
            }
        },
        Some(Commands::Serve { config, stdio }) => {
            let (settings, config_path) = load_settings(config.as_deref().or(cli.config.as_deref()), cli.root_paths)?;
            let app = app::App::new(settings)?;
            if stdio || cli.stdio {
                stdio::serve(app).await
            } else {
                let path_display = config_path.unwrap_or_else(|| PathBuf::from("default"));
                server::serve(app, &path_display).await
            }
        }
        None => {
            let is_interactive = std::io::stdout().is_terminal() && std::io::stdin().is_terminal();
            let force_stdio = cli.stdio || !is_interactive;

            let (settings, config_path) = load_settings(cli.config.as_deref(), cli.root_paths)?;
            let app = app::App::new(settings)?;

            if force_stdio {
                stdio::serve(app).await
            } else {
                let path_display = config_path.unwrap_or_else(|| PathBuf::from("default"));
                server::serve(app, &path_display).await
            }
        }
    }
}

/// Resolves configuration settings either from an explicit TOML file, auto-discovered TOML file,
/// or positional CLI root path arguments falling back to default settings template.
fn load_settings(explicit: Option<&Path>, root_paths: Vec<PathBuf>) -> Result<(Settings, Option<PathBuf>)> {
    if let Some(resolved) = resolve_config_path(explicit) {
        let settings = Settings::load(&resolved)?;
        return Ok((settings, Some(resolved)));
    }

    let settings = Settings::default_with_roots(root_paths)?;
    Ok((settings, None))
}
