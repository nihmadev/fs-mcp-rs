//! Bounded, deterministic, flat directory-tree traversal.

use crate::security::{AccessError, Policy, display_path};
use globset::{Glob, GlobSet, GlobSetBuilder};
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

fn default_depth() -> usize {
    2
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Arguments for `list_tree`.
pub struct ListTreeRequest {
    /// Root directory.
    pub path: PathBuf,
    /// Maximum descendant depth; zero returns no descendants.
    #[serde(default = "default_depth")]
    pub depth: usize,
    /// Optional inclusion globs matched against slash-normalized relative paths.
    #[serde(default)]
    pub include: Vec<String>,
    /// Optional exclusion globs matched against slash-normalized relative paths.
    #[serde(default)]
    pub exclude: Vec<String>,
    /// Optional page size, capped by configuration.
    pub max_entries: Option<usize>,
    /// Opaque continuation cursor.
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
/// One flat tree entry.
pub struct TreeEntry {
    /// Slash-normalized path relative to the root.
    pub relative_path: String,
    /// Stable display-form absolute path.
    pub display_path: String,
    /// Final path component.
    pub name: String,
    /// `file`, `directory`, `symlink`, or `other`.
    pub kind: &'static str,
    /// File size where meaningful.
    pub size: Option<u64>,
    /// One-based depth below the requested root.
    pub depth: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// A bounded warning emitted for a skipped entry.
pub struct TreeWarning {
    /// Path available at the failure boundary.
    pub path: String,
    /// Human-readable reason.
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Paginated tree result.
pub struct ListTreeOutput {
    /// Canonical requested root.
    pub root: String,
    /// Effective requested depth.
    pub depth: usize,
    /// Flat deterministic entries.
    pub entries: Vec<TreeEntry>,
    /// Cursor for the next page.
    pub next_cursor: Option<String>,
    /// Whether more matching entries exist.
    pub has_more: bool,
    /// Bounded skipped-entry warnings.
    pub warnings: Vec<TreeWarning>,
}

#[derive(Debug, Error)]
/// Tree traversal errors.
pub enum TreeError {
    /// Policy denied the root.
    #[error(transparent)]
    Access(#[from] AccessError),
    /// Root is not a directory.
    #[error("path is not a directory")]
    NotDirectory,
    /// Per-call or configured limit is invalid.
    #[error("requested {name} exceeds configured maximum {max}")]
    Limit {
        /// Limit name.
        name: &'static str,
        /// Configured maximum.
        max: usize,
    },
    /// A glob pattern is invalid.
    #[error("invalid glob pattern: {0}")]
    InvalidGlob(String),
    /// Cursor was created for different arguments or is malformed.
    #[error("invalid or incompatible cursor")]
    InvalidCursor,
}

/// Immutable tree traversal service.
#[derive(Clone)]
pub struct TreeLister {
    policy: Policy,
    max_depth: usize,
    max_entries: usize,
    max_warnings: usize,
    include_hidden: bool,
    respect_gitignore: bool,
    follow_links: bool,
}

impl TreeLister {
    /// Creates a tree service with explicit limits and search-compatible ignore behavior.
    pub fn new(
        policy: Policy,
        max_depth: usize,
        max_entries: usize,
        max_warnings: usize,
        include_hidden: bool,
        respect_gitignore: bool,
        follow_links: bool,
    ) -> Self {
        Self {
            policy,
            max_depth,
            max_entries,
            max_warnings,
            include_hidden,
            respect_gitignore,
            follow_links,
        }
    }

    /// Lists one deterministic page of descendants.
    pub fn list(&self, request: &ListTreeRequest) -> Result<ListTreeOutput, TreeError> {
        if request.depth > self.max_depth {
            return Err(TreeError::Limit {
                name: "depth",
                max: self.max_depth,
            });
        }
        let page_size = request.max_entries.unwrap_or(self.max_entries);
        if page_size == 0 || page_size > self.max_entries {
            return Err(TreeError::Limit {
                name: "maxEntries",
                max: self.max_entries,
            });
        }
        let root = self.policy.read_path(&request.path)?;
        if !root.is_dir() {
            return Err(TreeError::NotDirectory);
        }
        let include = compile_globs(&request.include)?;
        let exclude = compile_globs(&request.exclude)?;
        let fingerprint = fingerprint(&root, request);
        let after = decode_cursor(request.cursor.as_deref(), &fingerprint)?;
        if request.depth == 0 {
            return Ok(ListTreeOutput {
                root: display_path(&root),
                depth: 0,
                entries: Vec::new(),
                next_cursor: None,
                has_more: false,
                warnings: Vec::new(),
            });
        }
        let mut builder = WalkBuilder::new(&root);
        builder
            .hidden(!self.include_hidden)
            .git_ignore(self.respect_gitignore)
            .git_global(self.respect_gitignore)
            .git_exclude(self.respect_gitignore)
            .follow_links(self.follow_links)
            .max_depth(Some(request.depth + 1))
            .threads(1);
        let mut entries = Vec::with_capacity(page_size + 1);
        let mut warnings = Vec::new();
        let mut seen_dirs = HashSet::new();
        for item in builder.build() {
            let entry = match item {
                Ok(value) => value,
                Err(error) => {
                    if warnings.len() < self.max_warnings {
                        warnings.push(TreeWarning {
                            path: String::new(),
                            message: error.to_string(),
                        });
                    }
                    continue;
                }
            };
            if entry.depth() == 0 {
                continue;
            }
            let path = entry.path();
            let relative = match path.strip_prefix(&root) {
                Ok(v) => normalize_relative(v),
                Err(_) => continue,
            };
            if after
                .as_deref()
                .is_some_and(|cursor| relative.as_str() <= cursor)
            {
                continue;
            }
            if exclude.as_ref().is_some_and(|set| set.is_match(&relative)) {
                continue;
            }
            if include.as_ref().is_some_and(|set| !set.is_match(&relative)) {
                continue;
            }
            let file_type = match entry.file_type() {
                Some(value) => value,
                None => continue,
            };
            if self.follow_links && file_type.is_dir() {
                if let Ok(canonical) = fs::canonicalize(path) {
                    if !seen_dirs.insert(canonical) {
                        continue;
                    }
                }
            }
            let metadata = match fs::symlink_metadata(path) {
                Ok(value) => value,
                Err(error) => {
                    if warnings.len() < self.max_warnings {
                        warnings.push(TreeWarning {
                            path: display_path(path),
                            message: error.to_string(),
                        });
                    }
                    continue;
                }
            };
            let kind = if file_type.is_symlink() {
                "symlink"
            } else if metadata.is_file() {
                "file"
            } else if metadata.is_dir() {
                "directory"
            } else {
                "other"
            };
            entries.push(TreeEntry {
                relative_path: relative,
                display_path: display_path(path),
                name: entry.file_name().to_string_lossy().into_owned(),
                kind,
                size: metadata.is_file().then_some(metadata.len()),
                depth: entry.depth(),
            });
            if entries.len() > page_size {
                break;
            }
        }
        entries.sort_unstable_by(|a, b| a.relative_path.cmp(&b.relative_path));
        let has_more = entries.len() > page_size;
        if has_more {
            entries.truncate(page_size);
        }
        let next_cursor = has_more.then(|| {
            encode_cursor(
                &fingerprint,
                &entries.last().expect("non-empty page").relative_path,
            )
        });
        Ok(ListTreeOutput {
            root: display_path(&root),
            depth: request.depth,
            entries,
            next_cursor,
            has_more,
            warnings,
        })
    }
}

fn compile_globs(values: &[String]) -> Result<Option<GlobSet>, TreeError> {
    if values.is_empty() {
        return Ok(None);
    }
    let mut builder = GlobSetBuilder::new();
    for value in values {
        builder.add(Glob::new(value).map_err(|e| TreeError::InvalidGlob(e.to_string()))?);
    }
    builder
        .build()
        .map(Some)
        .map_err(|e| TreeError::InvalidGlob(e.to_string()))
}

fn normalize_relative(path: &Path) -> String {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn fingerprint(root: &Path, request: &ListTreeRequest) -> String {
    let material = format!(
        "{}\0{}\0{:?}\0{:?}",
        display_path(root),
        request.depth,
        request.include,
        request.exclude
    );
    blake3::hash(material.as_bytes()).to_hex()[..16].to_owned()
}

fn encode_cursor(fingerprint: &str, relative: &str) -> String {
    format!(
        "{fingerprint}:{}",
        relative
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn decode_cursor(cursor: Option<&str>, fingerprint: &str) -> Result<Option<String>, TreeError> {
    let Some(cursor) = cursor else {
        return Ok(None);
    };
    let (prefix, encoded) = cursor.split_once(':').ok_or(TreeError::InvalidCursor)?;
    if prefix != fingerprint || encoded.len() % 2 != 0 {
        return Err(TreeError::InvalidCursor);
    }
    let bytes = (0..encoded.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&encoded[i..i + 2], 16).map_err(|_| TreeError::InvalidCursor))
        .collect::<Result<Vec<_>, _>>()?;
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|_| TreeError::InvalidCursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lister(root: &Path) -> TreeLister {
        TreeLister::new(
            Policy::new(vec![root.to_owned()], false, false, false).unwrap(),
            5,
            2,
            4,
            false,
            true,
            false,
        )
    }

    #[test]
    fn depth_order_globs_and_pagination() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("d")).unwrap();
        fs::write(root.path().join("b.txt"), "b").unwrap();
        fs::write(root.path().join("a.txt"), "a").unwrap();
        fs::write(root.path().join("d/c.txt"), "c").unwrap();
        let zero = lister(root.path())
            .list(&ListTreeRequest {
                path: root.path().into(),
                depth: 0,
                include: vec![],
                exclude: vec![],
                max_entries: None,
                cursor: None,
            })
            .unwrap();
        assert!(zero.entries.is_empty());
        let first = lister(root.path())
            .list(&ListTreeRequest {
                path: root.path().into(),
                depth: 2,
                include: vec!["**/*.txt".into(), "*.txt".into()],
                exclude: vec!["b*".into()],
                max_entries: Some(2),
                cursor: None,
            })
            .unwrap();
        assert_eq!(
            first
                .entries
                .iter()
                .map(|e| e.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["a.txt", "d/c.txt"]
        );
    }

    #[test]
    fn cursor_rejects_changed_filters_and_non_directory() {
        let root = tempfile::tempdir().unwrap();
        fs::write(root.path().join("a"), "a").unwrap();
        fs::write(root.path().join("b"), "b").unwrap();
        fs::write(root.path().join("c"), "c").unwrap();
        let request = ListTreeRequest {
            path: root.path().into(),
            depth: 1,
            include: vec![],
            exclude: vec![],
            max_entries: Some(2),
            cursor: None,
        };
        let first = lister(root.path()).list(&request).unwrap();
        assert!(first.has_more);
        let second = lister(root.path())
            .list(&ListTreeRequest {
                cursor: first.next_cursor,
                ..request
            })
            .unwrap();
        assert_eq!(second.entries.len(), 1);
        assert!(matches!(
            lister(root.path()).list(&ListTreeRequest {
                path: root.path().join("a"),
                depth: 1,
                include: vec![],
                exclude: vec![],
                max_entries: None,
                cursor: None
            }),
            Err(TreeError::NotDirectory)
        ));
    }
}
