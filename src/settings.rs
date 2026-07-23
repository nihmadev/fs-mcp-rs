//! Command-line and TOML configuration.
//!
//! [`Settings::load`] resolves relative filesystem roots against the directory
//! containing the configuration file and rejects unsafe or nonsensical limits.

use anyhow::{Context, Result, bail};
use clap::Parser;
use serde::Deserialize;
use std::{
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
};

#[derive(Debug, Parser)]
#[command(name = "fs-mcp-rs", version, about)]
/// Command-line options accepted by the server binary.
pub struct Cli {
    /// TOML configuration file. May also be set with FS_MCP_CONFIG.
    #[arg(long, env = "FS_MCP_CONFIG", value_name = "FILE")]
    /// Path to the TOML configuration file.
    pub config: PathBuf,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Complete validated server configuration.
pub struct Settings {
    /// HTTP server and request-concurrency settings.
    pub server: Server,
    /// Filesystem roots, permissions, and byte limits.
    pub filesystem: Filesystem,
    /// Search traversal, caching, and concurrency settings.
    pub search: Search,
    /// Child-process execution and session limits.
    pub terminal: Terminal,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// HTTP listener and request scheduling settings.
pub struct Server {
    /// IP address on which the HTTP server listens.
    pub host: IpAddr,
    /// TCP port on which the HTTP server listens.
    pub port: u16,
    /// Maximum number of search tool calls processed concurrently.
    pub max_concurrency: usize,
    /// Maximum concurrent non-search blocking operations.
    pub max_io_concurrency: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Filesystem access and per-operation size limits.
pub struct Filesystem {
    /// Allowed filesystem roots; relative values are resolved from the config directory.
    pub roots: Vec<PathBuf>,
    /// Whether all mutating filesystem operations are disabled.
    pub read_only: bool,
    /// Maximum output bytes returned by one session read.
    pub max_read_bytes: usize,
    /// Maximum bytes accepted by one filesystem write.
    pub max_write_bytes: usize,
    /// Whether validated paths may traverse symbolic links.
    pub follow_links: bool,
    #[serde(default = "default_tree_max_depth")]
    /// Maximum list_tree depth.
    pub tree_max_depth: usize,
    #[serde(default = "default_tree_max_entries")]
    /// Maximum entries returned by list_tree per page.
    pub tree_max_entries: usize,
    #[serde(default = "default_tree_max_warnings")]
    /// Maximum traversal warnings returned.
    pub tree_max_warnings: usize,
    #[serde(default = "default_patch_max_bytes")]
    /// Maximum apply_patch input bytes.
    pub patch_max_bytes: usize,
    #[serde(default = "default_patch_preview_bytes")]
    /// Maximum patch preview bytes.
    pub patch_preview_bytes: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Parallel search limits and traversal behavior.
pub struct Search {
    /// Maximum number of results returned by one search.
    pub max_results: usize,
    /// Maximum number of search tool calls processed concurrently.
    pub max_concurrency: usize,
    /// Number of filesystem-walker threads used by each search.
    pub worker_threads: usize,
    /// Maximum compiled regular expressions retained in the shared cache.
    pub regex_cache_capacity: usize,
    /// Whether traversal includes hidden files and directories.
    pub include_hidden: bool,
    /// Whether traversal honors Git ignore and exclude files.
    pub respect_gitignore: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
/// Command execution, buffering, and session-retention settings.
pub struct Terminal {
    /// Whether command execution tools are available.
    pub enabled: bool,
    /// Maximum number of search tool calls processed concurrently.
    pub max_concurrency: usize,
    /// Default command deadline in milliseconds.
    pub default_timeout_ms: u64,
    /// Largest command deadline accepted from a caller.
    pub max_timeout_ms: u64,
    /// Maximum output bytes retained per session.
    pub max_output_bytes: usize,
    #[serde(default = "default_terminal_max_read_bytes")]
    /// Maximum output bytes returned by one session read.
    pub max_read_bytes: usize,
    #[serde(default = "default_terminal_max_wait_ms")]
    /// Maximum long-poll duration for one session read.
    pub max_wait_ms: u64,
    #[serde(default = "default_terminal_session_retention_ms")]
    /// Duration completed sessions remain queryable.
    pub session_retention_ms: u64,
}

fn default_tree_max_depth() -> usize {
    8
}
fn default_tree_max_entries() -> usize {
    1000
}
fn default_tree_max_warnings() -> usize {
    32
}
fn default_patch_max_bytes() -> usize {
    1048576
}
fn default_patch_preview_bytes() -> usize {
    16384
}

fn default_terminal_max_read_bytes() -> usize {
    262_144
}

fn default_terminal_max_wait_ms() -> u64 {
    30_000
}

fn default_terminal_session_retention_ms() -> u64 {
    300_000
}

impl Settings {
    /// Loads, resolves, and validates a TOML configuration file.
    ///
    /// Relative filesystem roots are interpreted relative to the configuration
    /// file, not the process working directory.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, TOML decoding fails, an
    /// unknown field is present, or a configured limit is invalid.
    pub fn load(path: &Path) -> Result<Self> {
        let config_path = fs::canonicalize(path)
            .with_context(|| format!("cannot resolve configuration file {}", path.display()))?;
        let text = fs::read_to_string(&config_path)
            .with_context(|| format!("cannot read {}", config_path.display()))?;
        let mut settings: Self = toml::from_str(&text).context("invalid configuration")?;
        let base = config_path
            .parent()
            .context("configuration has no parent directory")?;
        for root in &mut settings.filesystem.roots {
            if root.is_relative() {
                *root = base.join(&*root);
            }
        }
        settings.validate()?;
        Ok(settings)
    }

    fn validate(&self) -> Result<()> {
        if self.filesystem.roots.is_empty() {
            bail!("filesystem.roots must contain at least one directory");
        }
        if self.server.max_concurrency == 0 || self.server.max_io_concurrency == 0 {
            bail!("server concurrency limits must be greater than zero");
        }
        if self.search.max_concurrency == 0 || self.search.worker_threads == 0 {
            bail!("search concurrency and worker_threads must be greater than zero");
        }
        if self.filesystem.tree_max_depth == 0
            || self.filesystem.tree_max_entries == 0
            || self.filesystem.tree_max_warnings == 0
            || self.filesystem.patch_max_bytes == 0
            || self.filesystem.patch_preview_bytes == 0
            || self.filesystem.patch_preview_bytes > self.filesystem.patch_max_bytes
        {
            bail!(
                "filesystem tree and patch limits must be positive and patch_preview_bytes must not exceed patch_max_bytes"
            );
        }
        if self.filesystem.max_read_bytes == 0 || self.filesystem.max_write_bytes == 0 {
            bail!("filesystem byte limits must be greater than zero");
        }
        if self.search.max_results == 0 {
            bail!("search.max_results must be greater than zero");
        }
        if self.terminal.max_concurrency == 0 {
            bail!("terminal.max_concurrency must be greater than zero");
        }
        if self.terminal.default_timeout_ms == 0
            || self.terminal.max_timeout_ms == 0
            || self.terminal.default_timeout_ms > self.terminal.max_timeout_ms
        {
            bail!(
                "terminal timeouts must be greater than zero and default_timeout_ms must not exceed max_timeout_ms"
            );
        }
        if self.terminal.max_output_bytes == 0 || self.terminal.max_read_bytes == 0 {
            bail!("terminal output and read byte limits must be greater than zero");
        }
        if self.terminal.max_wait_ms == 0 || self.terminal.session_retention_ms == 0 {
            bail!("terminal wait and session retention limits must be greater than zero");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_fields() {
        let input = r#"
[server]
host = "127.0.0.1"
port = 8000
max_concurrency = 1
max_io_concurrency = 1
typo = true
[filesystem]
roots = ["."]
read_only = true
max_read_bytes = 1
max_write_bytes = 1
follow_links = false
[search]
max_results = 1
max_concurrency = 1
worker_threads = 1
regex_cache_capacity = 1
include_hidden = false
respect_gitignore = true
[terminal]
enabled = true
max_concurrency = 1
default_timeout_ms = 1000
max_timeout_ms = 2000
max_output_bytes = 1024
"#;
        assert!(toml::from_str::<Settings>(input).is_err());
    }
}
