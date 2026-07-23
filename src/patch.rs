//! Safe, bounded application of a unified diff to one existing UTF-8 file.

use crate::filesystem::{Filesystem, FsError};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
/// Arguments for the `apply_patch` MCP tool.
pub struct ApplyPatchRequest {
    /// Authoritative target path.
    pub path: PathBuf,
    /// Unified diff affecting exactly one existing file.
    pub patch: String,
    /// Optional optimistic-concurrency digest.
    pub expected_blake3: Option<String>,
    /// Validate fully without writing.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Result of a fully validated patch application.
pub struct ApplyPatchOutput {
    /// True when bytes were written.
    pub applied: bool,
    /// Whether this was a non-mutating validation run.
    pub dry_run: bool,
    /// Whether the resulting bytes differ.
    pub changed: bool,
    /// Number of hunks validated.
    pub hunks_applied: usize,
    /// Added line count.
    pub added_lines: usize,
    /// Removed line count.
    pub removed_lines: usize,
    /// Digest before application.
    pub old_blake3: String,
    /// Digest after application.
    pub new_blake3: String,
    /// Resulting byte length.
    pub byte_length: usize,
    /// Bounded normalized patch preview.
    pub preview: String,
}

#[derive(Debug, Error)]
/// Stable patch validation failures.
pub enum PatchError {
    /// Filesystem or policy failure.
    #[error(transparent)]
    Fs(#[from] FsError),
    /// Patch input is too large.
    #[error("patch exceeds configured limit of {limit} bytes")]
    Limit {
        /// Limit in bytes.
        limit: usize,
    },
    /// Unified diff is malformed.
    #[error("invalid unified patch: {0}")]
    Invalid(String),
    /// Patch requests an unsupported operation.
    #[error("unsupported patch operation: {0}")]
    Unsupported(String),
    /// Header does not identify the explicit target.
    #[error("patch header does not match target path")]
    HeaderMismatch,
    /// Context or removal lines do not match.
    #[error("patch context does not match the current file at line {line}")]
    ContextMismatch {
        /// One-based line number.
        line: usize,
    },
    /// Optimistic-concurrency digest differs.
    #[error("expected BLAKE3 does not match current file")]
    HashConflict,
}

#[derive(Debug)]
struct Hunk {
    old_start: usize,
    old_count: usize,
    new_count: usize,
    lines: Vec<HunkLine>,
}

#[derive(Debug)]
enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

/// Applies a single-file patch atomically, or validates it in dry-run mode.
pub fn apply_patch(
    fs: &Filesystem,
    request: &ApplyPatchRequest,
    max_patch_bytes: usize,
    max_preview_bytes: usize,
) -> Result<ApplyPatchOutput, PatchError> {
    if request.patch.len() > max_patch_bytes {
        return Err(PatchError::Limit {
            limit: max_patch_bytes,
        });
    }
    let bytes = fs.read_all(&request.path)?;
    let original = std::str::from_utf8(&bytes).map_err(|_| FsError::Utf8)?;
    let old_blake3 = blake3::hash(&bytes).to_hex().to_string();
    if request
        .expected_blake3
        .as_deref()
        .is_some_and(|expected| !expected.eq_ignore_ascii_case(&old_blake3))
    {
        return Err(PatchError::HashConflict);
    }
    let hunks = parse_patch(&request.patch, &request.path)?;
    let line_ending = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let final_newline = original.ends_with('\n');
    let normalized = original.replace("\r\n", "\n");
    let mut source: Vec<&str> = normalized.split('\n').collect();
    if final_newline {
        source.pop();
    }
    let mut result: Vec<String> = Vec::new();
    let mut source_index = 0usize;
    let mut added = 0usize;
    let mut removed = 0usize;
    for hunk in &hunks {
        let start = hunk.old_start.saturating_sub(1);
        if start < source_index || start > source.len() {
            return Err(PatchError::Invalid(
                "overlapping or out-of-order hunk".into(),
            ));
        }
        result.extend(source[source_index..start].iter().map(|s| (*s).to_owned()));
        source_index = start;
        let mut observed_old = 0usize;
        let mut observed_new = 0usize;
        for line in &hunk.lines {
            match line {
                HunkLine::Context(expected) => {
                    if source.get(source_index).copied() != Some(expected.as_str()) {
                        return Err(PatchError::ContextMismatch {
                            line: source_index + 1,
                        });
                    }
                    result.push(expected.clone());
                    source_index += 1;
                    observed_old += 1;
                    observed_new += 1;
                }
                HunkLine::Remove(expected) => {
                    if source.get(source_index).copied() != Some(expected.as_str()) {
                        return Err(PatchError::ContextMismatch {
                            line: source_index + 1,
                        });
                    }
                    source_index += 1;
                    observed_old += 1;
                    removed += 1;
                }
                HunkLine::Add(value) => {
                    result.push(value.clone());
                    observed_new += 1;
                    added += 1;
                }
            }
        }
        if observed_old != hunk.old_count || observed_new != hunk.new_count {
            return Err(PatchError::Invalid(
                "hunk line counts do not match header".into(),
            ));
        }
    }
    result.extend(source[source_index..].iter().map(|s| (*s).to_owned()));
    let mut output = result.join(line_ending).into_bytes();
    if final_newline {
        output.extend_from_slice(line_ending.as_bytes());
    }
    if output.len() > fs.max_write_bytes() {
        return Err(PatchError::Limit {
            limit: fs.max_write_bytes(),
        });
    }
    let changed = output != bytes;
    let new_blake3 = blake3::hash(&output).to_hex().to_string();
    if !request.dry_run && changed {
        fs.write_existing(&request.path, &output)?;
    } else if !request.dry_run {
        fs.check_write(&request.path)?;
    }
    let preview = bounded_preview(&request.patch, max_preview_bytes);
    Ok(ApplyPatchOutput {
        applied: !request.dry_run && changed,
        dry_run: request.dry_run,
        changed,
        hunks_applied: hunks.len(),
        added_lines: added,
        removed_lines: removed,
        old_blake3,
        new_blake3,
        byte_length: output.len(),
        preview,
    })
}

fn parse_patch(patch: &str, target: &Path) -> Result<Vec<Hunk>, PatchError> {
    if patch.contains("GIT binary patch") || patch.contains("Binary files ") {
        return Err(PatchError::Unsupported("binary patch".into()));
    }
    if patch.contains("rename from ")
        || patch.contains("rename to ")
        || patch.contains("similarity index ")
    {
        return Err(PatchError::Unsupported("rename or move".into()));
    }
    let normalized = patch.replace("\r\n", "\n");
    let lines: Vec<&str> = normalized.lines().collect();
    let old_headers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| l.starts_with("--- ").then_some(i))
        .collect();
    let new_headers: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| l.starts_with("+++ ").then_some(i))
        .collect();
    if old_headers.len() != 1 || new_headers.len() != 1 || new_headers[0] != old_headers[0] + 1 {
        return Err(PatchError::Unsupported(
            "patch must affect exactly one file".into(),
        ));
    }
    let old_name = header_name(lines[old_headers[0]]);
    let new_name = header_name(lines[new_headers[0]]);
    if old_name == "/dev/null" || new_name == "/dev/null" {
        return Err(PatchError::Unsupported("file creation or deletion".into()));
    }
    if !header_matches(old_name, target) || !header_matches(new_name, target) {
        return Err(PatchError::HeaderMismatch);
    }
    let mut hunks = Vec::new();
    let mut i = new_headers[0] + 1;
    while i < lines.len() {
        let line = lines[i];
        if line.starts_with("diff --git ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            return Err(PatchError::Unsupported("multiple-file patch".into()));
        }
        if !line.starts_with("@@ ") {
            if line.is_empty() {
                i += 1;
                continue;
            }
            return Err(PatchError::Invalid(format!("unexpected line: {line}")));
        }
        let (old_start, old_count, new_count) = parse_hunk_header(line)?;
        i += 1;
        let mut hunk_lines = Vec::new();
        while i < lines.len() && !lines[i].starts_with("@@ ") {
            let value = lines[i];
            if value == "\\ No newline at end of file" {
                i += 1;
                continue;
            }
            let (prefix, rest) =
                value.split_at(value.char_indices().nth(1).map_or(value.len(), |(n, _)| n));
            let item = match prefix {
                " " => HunkLine::Context(rest.to_owned()),
                "-" => HunkLine::Remove(rest.to_owned()),
                "+" => HunkLine::Add(rest.to_owned()),
                _ => return Err(PatchError::Invalid("malformed hunk line".into())),
            };
            hunk_lines.push(item);
            i += 1;
        }
        hunks.push(Hunk {
            old_start,
            old_count,
            new_count,
            lines: hunk_lines,
        });
    }
    if hunks.is_empty() {
        return Err(PatchError::Invalid("patch contains no hunks".into()));
    }
    Ok(hunks)
}

fn parse_hunk_header(line: &str) -> Result<(usize, usize, usize), PatchError> {
    let end = line[3..]
        .find(" @@")
        .ok_or_else(|| PatchError::Invalid("malformed hunk header".into()))?
        + 3;
    let mut parts = line[3..end].split_whitespace();
    let old = parts
        .next()
        .ok_or_else(|| PatchError::Invalid("missing old range".into()))?;
    let new = parts
        .next()
        .ok_or_else(|| PatchError::Invalid("missing new range".into()))?;
    if parts.next().is_some() || !old.starts_with('-') || !new.starts_with('+') {
        return Err(PatchError::Invalid("malformed hunk range".into()));
    }
    let (old_start, old_count) = parse_range(&old[1..])?;
    let (_, new_count) = parse_range(&new[1..])?;
    if old_start == 0 {
        return Err(PatchError::Invalid(
            "old hunk start must be positive".into(),
        ));
    }
    Ok((old_start, old_count, new_count))
}

fn parse_range(value: &str) -> Result<(usize, usize), PatchError> {
    let mut parts = value.split(',');
    let start = parts
        .next()
        .and_then(|v| v.parse().ok())
        .ok_or_else(|| PatchError::Invalid("invalid hunk range".into()))?;
    let count = parts.next().map_or(Ok(1), |v| {
        v.parse()
            .map_err(|_| PatchError::Invalid("invalid hunk count".into()))
    })?;
    if parts.next().is_some() {
        return Err(PatchError::Invalid("invalid hunk range".into()));
    }
    Ok((start, count))
}

fn header_name(line: &str) -> &str {
    line[4..].split('\t').next().unwrap_or("").trim()
}

fn header_matches(header: &str, target: &Path) -> bool {
    let normalized = header
        .strip_prefix("a/")
        .or_else(|| header.strip_prefix("b/"))
        .unwrap_or(header)
        .replace('\\', "/");
    let target_display = target.to_string_lossy().replace('\\', "/");
    target_display == normalized
        || target_display.ends_with(&format!("/{normalized}"))
        || target
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == normalized)
}

fn bounded_preview(patch: &str, limit: usize) -> String {
    let normalized = patch.replace("\r\n", "\n");
    if normalized.len() <= limit {
        return normalized;
    }
    let mut end = limit.min(normalized.len());
    while !normalized.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}\n… preview truncated …", &normalized[..end])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::security::Policy;

    fn fs(root: &Path, read_only: bool) -> Filesystem {
        Filesystem::new(
            Policy::new(vec![root.to_owned()], read_only, false).unwrap(),
            4096,
            4096,
        )
    }

    #[test]
    fn applies_one_and_multiple_hunks_and_dry_run() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.txt");
        std::fs::write(&path, "one\ntwo\nthree\nfour\n").unwrap();
        let request = ApplyPatchRequest { path: path.clone(), patch: "--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n one\n-two\n+TWO\n@@ -4,1 +4,2 @@\n four\n+five\n".into(), expected_blake3: None, dry_run: true };
        let out = apply_patch(&fs(root.path(), true), &request, 4096, 1024).unwrap();
        assert!(out.changed);
        assert!(!out.applied);
        assert_eq!(out.hunks_applied, 2);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "one\ntwo\nthree\nfour\n"
        );
        let real = ApplyPatchRequest {
            dry_run: false,
            ..request
        };
        apply_patch(&fs(root.path(), false), &real, 4096, 1024).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "one\nTWO\nthree\nfour\nfive\n"
        );
    }

    #[test]
    fn preserves_crlf_and_rejects_context_without_writing() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("a.txt");
        std::fs::write(&path, b"a\r\nb\r\n").unwrap();
        let bad = ApplyPatchRequest {
            path: path.clone(),
            patch: "--- a.txt\n+++ a.txt\n@@ -1,1 +1,1 @@\n-x\n+y\n".into(),
            expected_blake3: None,
            dry_run: false,
        };
        assert!(matches!(
            apply_patch(&fs(root.path(), false), &bad, 4096, 1024),
            Err(PatchError::ContextMismatch { .. })
        ));
        assert_eq!(std::fs::read(&path).unwrap(), b"a\r\nb\r\n");
        let good = ApplyPatchRequest {
            patch: "--- a.txt\n+++ a.txt\n@@ -2,1 +2,1 @@\n-b\n+B\n".into(),
            ..bad
        };
        apply_patch(&fs(root.path(), false), &good, 4096, 1024).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"a\r\nB\r\n");
    }
}
