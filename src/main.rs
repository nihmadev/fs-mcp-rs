use anyhow::{Result, bail};
use clap::Parser;
use fs_mcp_rs::cli_format::{
    default_config_toml, print_client_snippets, print_config_summary, print_no_config_banner,
    print_tools_catalog,
};
use fs_mcp_rs::protocol::Tool;
use fs_mcp_rs::settings::{Cli, Commands, ConfigCommands, Settings, resolve_config_path};
use fs_mcp_rs::wizard::run_wizard;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

mod app;
mod oauth;
mod server;
mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

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
        Some(Commands::Serve { config }) => {
            let config_path = get_or_prompt_config(config.as_deref())?;
            run_server(config_path).await
        }
        None => {
            let config_path = get_or_prompt_config(cli.config.as_deref())?;
            run_server(config_path).await
        }
    }
}

fn get_or_prompt_config(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(resolved) = resolve_config_path(explicit) {
        println!("[INFO] Using configuration: {}", resolved.display());
        return Ok(resolved);
    }

    if std::io::stdout().is_terminal() && std::io::stdin().is_terminal() {
        print_no_config_banner();
        let prompt_wizard =
            inquire::Confirm::new("Would you like to run the interactive setup wizard now?")
                .with_default(true)
                .prompt()
                .unwrap_or(false);

        if prompt_wizard {
            return run_wizard(Path::new("config.toml"));
        }
    } else {
        print_no_config_banner();
    }

    bail!("No configuration file specified or found.")
}

async fn run_server(config_path: PathBuf) -> Result<()> {
    let settings = Settings::load(&config_path)?;
    let app = app::App::new(settings)?;
    server::serve(app, &config_path).await
}
