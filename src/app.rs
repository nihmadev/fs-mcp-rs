use crate::oauth::OAuthStore;
use anyhow::Result;
use fs_mcp_rs::{
    filesystem::Filesystem, search::Searcher, security::Policy,
    settings::Settings, terminal::Terminal, tree::TreeLister,
};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[derive(Clone)]
/// Shared services and concurrency gates for HTTP request handlers.
pub(crate) struct App {
    pub(crate) fs: Filesystem,
    pub(crate) search: Searcher,
    pub(crate) tree: TreeLister,
    pub(crate) settings: Arc<Settings>,
    pub(crate) terminal: Terminal,
    pub(crate) oauth: OAuthStore,
    pub(crate) permits: Arc<Semaphore>,
    pub(crate) io_permits: Arc<Semaphore>,
    pub(crate) search_permits: Arc<Semaphore>,
}

impl App {
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        let policy = Policy::new(
            settings.filesystem.roots.clone(),
            settings.filesystem.read_only,
            settings.filesystem.follow_links,
        )?;
        Ok(Self {
            tree: TreeLister::new(
                policy.clone(),
                settings.filesystem.tree_max_depth,
                settings.filesystem.tree_max_entries,
                settings.filesystem.tree_max_warnings,
                settings.search.include_hidden,
                settings.search.respect_gitignore,
                settings.filesystem.follow_links,
            ),
            fs: Filesystem::new(
                policy.clone(),
                settings.filesystem.max_read_bytes,
                settings.filesystem.max_write_bytes,
            ),
            search: Searcher::new(
                policy,
                settings.search.max_results,
                settings.search.worker_threads,
                settings.search.regex_cache_capacity,
                settings.search.include_hidden,
                settings.search.respect_gitignore,
            ),
            terminal: Terminal::new(
                settings.terminal.enabled,
                settings.terminal.max_concurrency,
                settings.terminal.default_timeout_ms,
                settings.terminal.max_timeout_ms,
                settings.terminal.max_output_bytes,
                settings.terminal.max_read_bytes,
                settings.terminal.max_wait_ms,
                settings.terminal.session_retention_ms,
            ),
            oauth: OAuthStore::new(),
            permits: Arc::new(Semaphore::new(settings.server.max_concurrency)),
            io_permits: Arc::new(Semaphore::new(settings.server.max_io_concurrency)),
            search_permits: Arc::new(Semaphore::new(settings.search.max_concurrency)),
            settings: Arc::new(settings),
        })
    }
}
