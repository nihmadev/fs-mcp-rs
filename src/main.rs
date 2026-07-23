use anyhow::Result;
use clap::Parser;
use fs_mcp_rs::settings::{Cli, Settings};

mod app;
mod server;
mod tools;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let cli = Cli::parse();
    let settings = Settings::load(&cli.config)?;
    let app = app::App::new(settings)?;
    server::serve(app, &cli.config).await
}
