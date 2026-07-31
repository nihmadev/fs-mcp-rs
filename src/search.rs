//! Parallel, bounded filename and file-content search.
//!
//! Searches honor the same [`Policy`] as ordinary filesystem operations. The
//! walker can respect ignore files and hidden-file settings, while an atomic
//! reservation counter enforces the global result limit across worker threads.

use crate::security::{AccessError, Policy, display_path};
use ignore::{WalkBuilder, WalkState};
use regex::{Regex, RegexBuilder};
use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    fs::File,
    io::{BufRead, BufReader},
    path::Path,
    sync::{
        Arc, Mutex, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
};
use thiserror::Error;

#[derive(Debug, Error)]
/// An error produced while preparing or running a search.
pub enum SearchError {
    #[error(transparent)]
    /// The access policy rejected the search root.
    Access(#[from] AccessError),
    #[error(transparent)]
    /// An I/O operation required to start the search failed.
    Io(#[from] std::io::Error),
    #[error(transparent)]
    /// A regular-expression pattern failed to compile.
    Regex(#[from] regex::Error),
    #[error("search pattern must not be empty")]
    /// The caller supplied an empty search pattern.
    EmptyPattern,
}

#[derive(Debug, Serialize)]
/// One matching line returned by a content search.
pub struct Match {
    /// Display-form path of the matching file.
    pub path: String,
    /// One-based line number within the file.
    pub line: usize,
    /// Matched line text without its trailing line ending.
    pub text: String,
}

#[derive(Default)]
struct RegexCache {
    entries: HashMap<String, Regex>,
    insertion_order: VecDeque<String>,
}

#[derive(Clone)]
/// A cloneable parallel search service with a shared regular-expression cache.
pub struct Searcher {
    policy: Policy,
    max_results: usize,
    worker_threads: usize,
    regex_cache_capacity: usize,
    regex_cache: Arc<RwLock<RegexCache>>,
    hidden: bool,
    gitignore: bool,
}

enum Matcher {
    LiteralAscii(Vec<u8>),
    LiteralUnicode(String),
    Regex(Regex),
}

impl Matcher {
    fn new(searcher: &Searcher, pattern: &str, literal: bool) -> Result<Self, SearchError> {
        if pattern.is_empty() {
            return Err(SearchError::EmptyPattern);
        }
        if literal {
            if pattern.is_ascii() {
                Ok(Self::LiteralAscii(pattern.as_bytes().to_ascii_lowercase()))
            } else {
                Ok(Self::LiteralUnicode(pattern.to_lowercase()))
            }
        } else {
            Ok(Self::Regex(searcher.cached_regex(pattern)?))
        }
    }

    #[inline]
    fn is_match(&self, line: &str) -> bool {
        match self {
            Self::LiteralAscii(needle) => line
                .as_bytes()
                .windows(needle.len())
                .any(|window| window.eq_ignore_ascii_case(needle)),
            Self::LiteralUnicode(needle) => line.to_lowercase().contains(needle),
            Self::Regex(regex) => regex.is_match(line),
        }
    }
}

impl Searcher {
    /// Creates a search service.
    ///
    /// `max_results` is enforced globally per search, including across worker
    /// threads. A zero cache capacity disables regular-expression caching.
    pub fn new(
        policy: Policy,
        max_results: usize,
        worker_threads: usize,
        regex_cache_capacity: usize,
        hidden: bool,
        gitignore: bool,
    ) -> Self {
        Self {
            policy,
            max_results,
            worker_threads,
            regex_cache_capacity,
            regex_cache: Arc::new(RwLock::new(RegexCache::default())),
            hidden,
            gitignore,
        }
    }

    fn cached_regex(&self, pattern: &str) -> Result<Regex, SearchError> {
        if pattern.is_empty() {
            return Err(SearchError::EmptyPattern);
        }
        if self.regex_cache_capacity == 0 {
            return Ok(RegexBuilder::new(pattern).case_insensitive(true).build()?);
        }
        if let Some(regex) = self
            .regex_cache
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .entries
            .get(pattern)
        {
            return Ok(regex.clone());
        }

        // Compile outside the write lock so unrelated cache hits are never blocked.
        let compiled = RegexBuilder::new(pattern).case_insensitive(true).build()?;
        let mut cache = self.regex_cache.write().unwrap_or_else(|p| p.into_inner());
        if let Some(regex) = cache.entries.get(pattern) {
            return Ok(regex.clone());
        }
        while cache.entries.len() >= self.regex_cache_capacity {
            if let Some(evicted) = cache.insertion_order.pop_front() {
                cache.entries.remove(&evicted);
            } else {
                break;
            }
        }
        cache.insertion_order.push_back(pattern.to_owned());
        cache.entries.insert(pattern.to_owned(), compiled.clone());
        Ok(compiled)
    }

    /// Searches readable text lines below `root`.
    ///
    /// Literal searches are case-insensitive. When `literal` is `false`,
    /// `pattern` is compiled as a case-insensitive regular expression. Files
    /// that cannot be opened or decoded line-by-line are skipped.
    ///
    /// # Errors
    ///
    /// Returns an error for a denied root, empty pattern, or invalid regex.
    pub fn content(
        &self,
        root: &Path,
        pattern: &str,
        literal: bool,
    ) -> Result<Vec<Match>, SearchError> {
        let root = self.policy.read_path(root)?;
        let matcher = Arc::new(Matcher::new(self, pattern, literal)?);
        let max_results = self.max_results;
        if max_results == 0 {
            return Ok(Vec::new());
        }
        // Reserve result slots atomically before buffering matches locally. This
        // keeps the limit exact without taking the result mutex for every line.
        let reserved = Arc::new(AtomicUsize::new(0));
        let matches = Arc::new(Mutex::new(Vec::with_capacity(max_results.min(256))));
        let mut walker = WalkBuilder::new(root);
        walker
            .hidden(!self.hidden)
            .git_ignore(self.gitignore)
            .git_global(self.gitignore)
            .git_exclude(self.gitignore)
            .threads(self.worker_threads.max(1));
        walker.build_parallel().run(|| {
            let matches = Arc::clone(&matches);
            let matcher = Arc::clone(&matcher);
            let reserved = Arc::clone(&reserved);
            Box::new(move |entry| {
                let Ok(entry) = entry else {
                    return WalkState::Continue;
                };
                if !entry.file_type().is_some_and(|kind| kind.is_file()) {
                    return WalkState::Continue;
                }
                if reserved.load(Ordering::Relaxed) >= max_results {
                    return WalkState::Quit;
                }
                let file = match File::open(entry.path()) {
                    Ok(file) => file,
                    Err(_) => return WalkState::Continue,
                };
                let mut reader = BufReader::with_capacity(64 * 1024, file);
                let mut line = String::new();
                let mut local = Vec::new();
                let mut line_number = 0;
                let path = display_path(entry.path());
                loop {
                    line.clear();
                    match reader.read_line(&mut line) {
                        Ok(0) => break,
                        Ok(_) => line_number += 1,
                        Err(_) => break,
                    }
                    if !matcher.is_match(&line) {
                        continue;
                    }
                    if reserved
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                            (count < max_results).then_some(count + 1)
                        })
                        .is_err()
                    {
                        break;
                    }
                    local.push(Match {
                        path: path.clone(),
                        line: line_number,
                        text: line.trim_end_matches(['\r', '\n']).to_owned(),
                    });
                }
                if !local.is_empty() {
                    matches
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .extend(local);
                }
                if reserved.load(Ordering::Relaxed) >= max_results {
                    WalkState::Quit
                } else {
                    WalkState::Continue
                }
            })
        });
        let mut guard = matches.lock().unwrap_or_else(|p| p.into_inner());
        let mut output = std::mem::take(&mut *guard);
        output.sort_unstable_by(|a, b| a.path.cmp(&b.path).then(a.line.cmp(&b.line)));
        Ok(output)
    }

    /// Finds entries whose file name contains `pattern`, case-insensitively.
    ///
    /// Results are sorted by display path and capped at the configured maximum.
    pub fn files(&self, root: &Path, pattern: &str) -> Result<Vec<String>, SearchError> {
        if pattern.is_empty() {
            return Err(SearchError::EmptyPattern);
        }
        let root = self.policy.read_path(root)?;
        let matcher = Arc::new(Matcher::new(self, pattern, true)?);
        let max_results = self.max_results;
        if max_results == 0 {
            return Ok(Vec::new());
        }
        let results = Arc::new(Mutex::new(Vec::with_capacity(max_results.min(256))));
        let mut walker = WalkBuilder::new(root);
        walker
            .hidden(!self.hidden)
            .git_ignore(self.gitignore)
            .git_global(self.gitignore)
            .git_exclude(self.gitignore)
            .threads(self.worker_threads.max(1));
        walker.build_parallel().run(|| {
            let results = Arc::clone(&results);
            let matcher = Arc::clone(&matcher);
            Box::new(move |entry| {
                let Ok(entry) = entry else {
                    return WalkState::Continue;
                };
                if !matcher.is_match(&entry.file_name().to_string_lossy()) {
                    return WalkState::Continue;
                }
                let mut output = results.lock().unwrap_or_else(|p| p.into_inner());
                if output.len() >= max_results {
                    return WalkState::Quit;
                }
                output.push(display_path(entry.path()));
                WalkState::Continue
            })
        });
        let mut guard = results.lock().unwrap_or_else(|p| p.into_inner());
        let mut output = std::mem::take(&mut *guard);
        output.sort_unstable();
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn searcher(root: &Path) -> Searcher {
        Searcher::new(
            Policy::new(vec![root.to_owned()], false, false).unwrap(),
            100,
            2,
            2,
            true,
            false,
        )
    }

    #[test]
    fn literal_search_uses_case_insensitive_fast_path() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("sample.txt"), "Alpha\nbEtA\n").unwrap();
        let found = searcher(root.path())
            .content(root.path(), "BETA", true)
            .unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line, 2);
        assert_eq!(found[0].text, "bEtA");
    }

    #[test]
    fn regex_cache_evicts_one_entry_instead_of_clearing() {
        let root = tempfile::tempdir().unwrap();
        let searcher = searcher(root.path());
        searcher.cached_regex("one").unwrap();
        searcher.cached_regex("two").unwrap();
        searcher.cached_regex("three").unwrap();
        let cache = searcher.regex_cache.read().unwrap();
        assert_eq!(cache.entries.len(), 2);
        assert!(!cache.entries.contains_key("one"));
        assert!(cache.entries.contains_key("two"));
        assert!(cache.entries.contains_key("three"));
    }
}
