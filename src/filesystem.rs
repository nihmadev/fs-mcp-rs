//! Bounded filesystem operations guarded by an access policy.
//!
//! [`Filesystem`] is the high-level I/O facade used by the server. Every path is
//! validated by [`Policy`] before it reaches `std::fs`; writes use a temporary
//! file followed by an atomic persist operation in the destination directory.

use crate::security::{AccessError, Policy, display_path};
use serde::Serialize;
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

#[derive(Debug, Error)]
/// An error produced by a bounded filesystem operation.
pub enum FsError {
    #[error(transparent)]
    /// The access policy rejected the path or operation.
    Access(#[from] AccessError),
    #[error(transparent)]
    /// The operating system reported an I/O error.
    Io(#[from] std::io::Error),
    #[error("operation exceeds configured limit of {limit} bytes")]
    /// The requested read or write exceeded the configured byte limit.
    Limit {
        /// Maximum number of bytes permitted by the operation.
        limit: usize,
    },
    #[error("content is not valid UTF-8")]
    /// A text-edit operation encountered non-UTF-8 file content.
    Utf8,
    #[error("search text must not be empty")]
    /// A text edit was requested with an empty search string.
    EmptyPattern,
    #[error("expected {expected} replacements, found {actual}")]
    /// The observed replacement count differed from the caller's expectation.
    Conflict {
        /// Number of replacements required by the caller.
        expected: usize,
        /// Number of replacements found in the current file content.
        actual: usize,
    },
}

#[derive(Debug, Serialize)]
/// Serializable metadata for one filesystem entry.
pub struct Entry {
    /// The final path component, decoded lossily when necessary.
    pub name: String,
    /// The display-form absolute path.
    pub path: String,
    /// The entry kind: `file`, `directory`, or `symlink`.
    pub kind: &'static str,
    /// The metadata length in bytes.
    pub size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
/// Rich cross-platform metadata returned by `file_info`.
pub struct FileInfo {
    /// Final path component.
    pub name: String,
    /// Stable canonical display path.
    pub display_path: String,
    /// Entry kind.
    pub kind: &'static str,
    /// File size where meaningful.
    pub size: Option<u64>,
    /// Filesystem read-only attribute.
    pub read_only: bool,
    /// Creation time as Unix milliseconds.
    pub created_ms: Option<i64>,
    /// Modification time as Unix milliseconds.
    pub modified_ms: Option<i64>,
    /// Access time as Unix milliseconds.
    pub accessed_ms: Option<i64>,
    /// Safely reportable symlink target, if any.
    pub symlink_target: Option<String>,
    /// Optional BLAKE3 digest for regular files.
    pub blake3: Option<String>,
}

fn system_time_ms(value: Option<SystemTime>) -> Option<i64> {
    value
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| i64::try_from(d.as_millis()).ok())
}

#[derive(Clone)]
/// A cloneable facade for policy-checked, bounded filesystem I/O.
pub struct Filesystem {
    policy: Policy,
    max_read: usize,
    max_write: usize,
}

impl Filesystem {
    /// Creates a filesystem facade with per-operation byte limits.
    pub fn new(policy: Policy, max_read: usize, max_write: usize) -> Self {
        Self {
            policy,
            max_read,
            max_write,
        }
    }

    /// Lists the direct children of `path`, sorted by name.
    ///
    /// # Errors
    ///
    /// Returns an error if policy validation fails or directory metadata cannot be read.
    pub fn list(&self, path: &Path) -> Result<Vec<Entry>, FsError> {
        let path = self.policy.read_path(path)?;
        let mut result = Vec::new();
        for item in fs::read_dir(path)? {
            let item = item?;
            let meta = item.metadata()?;
            let file_type = item.file_type()?;
            result.push(Entry {
                name: item.file_name().to_string_lossy().into_owned(),
                path: display_path(&item.path()),
                kind: if file_type.is_symlink() {
                    "symlink"
                } else if meta.is_dir() {
                    "directory"
                } else {
                    "file"
                },
                size: meta.len(),
            });
        }
        result.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    /// Reads at most `length` bytes beginning at `offset`.
    ///
    /// Reaching end-of-file is not an error and may return fewer bytes.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::Limit`] when `length` exceeds the configured maximum.
    pub fn read(&self, path: &Path, offset: u64, length: usize) -> Result<Vec<u8>, FsError> {
        if length > self.max_read {
            return Err(FsError::Limit {
                limit: self.max_read,
            });
        }
        let path = self.policy.read_path(path)?;
        let mut file = fs::File::open(path)?;
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(offset))?;
        let mut data = Vec::with_capacity(length);
        file.take(length as u64).read_to_end(&mut data)?;
        Ok(data)
    }

    /// Atomically replaces `path` with `data` and returns its BLAKE3 digest.
    ///
    /// The destination parent must already exist. Data is synced to a temporary
    /// file in that directory before the temporary file is persisted.
    ///
    /// # Errors
    ///
    /// Returns an error when writes are disabled, the path is outside the policy,
    /// `data` exceeds the configured maximum, or the atomic replacement fails.
    pub fn write(&self, path: &Path, data: &[u8]) -> Result<String, FsError> {
        let (hash, _) = self.write_with_parents(path, data, false)?;
        Ok(hash)
    }

    /// Atomically replaces `path`, optionally creating missing parent directories.
    ///
    /// Returns the content hash and the display paths of directories created by
    /// this call. Directory creation is policy-checked before any mutation.
    pub fn write_with_parents(
        &self,
        path: &Path,
        data: &[u8],
        create_parents: bool,
    ) -> Result<(String, Vec<String>), FsError> {
        if data.len() > self.max_write {
            return Err(FsError::Limit {
                limit: self.max_write,
            });
        }
        let (path, created) = self.policy.write_path_with_parents(path, create_parents)?;
        let parent = path
            .parent()
            .ok_or_else(|| AccessError::Invalid(path.display().to_string()))?;
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        file.write_all(data)?;
        file.as_file().sync_all()?;
        file.persist(&path).map_err(|error| error.error)?;
        Ok((
            blake3::hash(data).to_hex().to_string(),
            created.iter().map(|path| display_path(path)).collect(),
        ))
    }

    /// Reads an entire existing file while enforcing the configured bound.
    pub fn read_all(&self, path: &Path) -> Result<Vec<u8>, FsError> {
        let path = self.policy.read_path(path)?;
        let meta = fs::metadata(&path)?;
        if meta.len() > self.max_read as u64 {
            return Err(FsError::Limit {
                limit: self.max_read,
            });
        }
        let file = fs::File::open(path)?;
        let mut data = Vec::with_capacity(meta.len() as usize);
        file.take(self.max_read as u64 + 1).read_to_end(&mut data)?;
        if data.len() > self.max_read {
            return Err(FsError::Limit {
                limit: self.max_read,
            });
        }
        Ok(data)
    }

    /// Maximum permitted replacement size.
    pub fn max_write_bytes(&self) -> usize {
        self.max_write
    }

    /// Validates that a real write is allowed without mutating the target.
    pub fn check_write(&self, path: &Path) -> Result<(), FsError> {
        self.policy.write_path(path)?;
        Ok(())
    }

    /// Atomically replaces an existing file only.
    pub fn write_existing(&self, path: &Path, data: &[u8]) -> Result<String, FsError> {
        self.policy.read_path(path)?;
        self.write(path, data)
    }

    /// Returns rich, policy-checked metadata with an optional bounded streaming hash.
    pub fn file_info(&self, path: &Path, include_hash: bool) -> Result<FileInfo, FsError> {
        let path = self.policy.read_path(path)?;
        let metadata = fs::metadata(&path)?;
        let kind = if metadata.is_file() {
            "file"
        } else if metadata.is_dir() {
            "directory"
        } else {
            "other"
        };
        let blake3 = if include_hash && metadata.is_file() {
            Some(self.hash(&path)?)
        } else {
            None
        };
        Ok(FileInfo {
            name: path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
            display_path: display_path(&path),
            kind,
            size: metadata.is_file().then_some(metadata.len()),
            read_only: metadata.permissions().readonly(),
            created_ms: system_time_ms(metadata.created().ok()),
            modified_ms: system_time_ms(metadata.modified().ok()),
            accessed_ms: system_time_ms(metadata.accessed().ok()),
            symlink_target: None,
            blake3,
        })
    }

    /// Creates exactly one directory.
    ///
    /// Parent directories are not created automatically.
    pub fn create_directory(&self, path: &Path) -> Result<(), FsError> {
        fs::create_dir(self.policy.write_path(path)?)?;
        Ok(())
    }

    /// Moves `source` to `destination` without overwriting an existing entry.
    ///
    /// Both paths must satisfy the access policy.
    pub fn move_path(&self, source: &Path, destination: &Path) -> Result<(), FsError> {
        let source = self.policy.read_path(source)?;
        let destination = self.policy.write_path(destination)?;
        if destination.exists() {
            return Err(FsError::Io(std::io::ErrorKind::AlreadyExists.into()));
        }
        fs::rename(source, destination)?;
        Ok(())
    }

    /// Removes one file or one empty directory.
    ///
    /// Recursive directory removal is intentionally unsupported.
    pub fn remove(&self, path: &Path) -> Result<(), FsError> {
        let path = self.policy.read_path(path)?;
        let checked = self.policy.write_path(&path)?;
        if checked.is_dir() {
            fs::remove_dir(checked)?;
        } else {
            fs::remove_file(checked)?;
        }
        Ok(())
    }

    /// Streams a file into BLAKE3 and returns the lowercase hexadecimal digest.
    pub fn hash(&self, path: &Path) -> Result<String, FsError> {
        let path = self.policy.read_path(path)?;
        let mut file = fs::File::open(path)?;
        let mut hasher = blake3::Hasher::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(hasher.finalize().to_hex().to_string())
    }

    /// Replaces exactly `expected` non-overlapping occurrences of `old`.
    ///
    /// The operation reads the entire bounded file as UTF-8, verifies the match
    /// count, and then delegates to [`Self::write`] for atomic replacement.
    ///
    /// # Errors
    ///
    /// Returns [`FsError::Conflict`] without writing when the count differs.
    pub fn edit(
        &self,
        path: &Path,
        old: &str,
        new: &str,
        expected: usize,
    ) -> Result<String, FsError> {
        if old.is_empty() {
            return Err(FsError::EmptyPattern);
        }
        let bytes = self.read(path, 0, self.max_read)?;
        let text = String::from_utf8(bytes).map_err(|_| FsError::Utf8)?;
        let actual = text.matches(old).count();
        if actual != expected {
            return Err(FsError::Conflict { expected, actual });
        }
        self.write(path, text.replacen(old, new, expected).as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_reads_and_hashes() {
        let root = tempfile::tempdir().unwrap();
        let policy = Policy::new(vec![root.path().to_owned()], false, false, false).unwrap();
        let filesystem = Filesystem::new(policy, 1024, 1024);
        let path = root.path().join("sample.txt");
        let hash = filesystem.write(&path, b"hello").unwrap();
        assert_eq!(filesystem.read(&path, 0, 5).unwrap(), b"hello");
        assert_eq!(hash, blake3::hash(b"hello").to_hex().to_string());
    }

    #[test]
    fn rejects_oversized_reads_and_writes() {
        let root = tempfile::tempdir().unwrap();
        let policy = Policy::new(vec![root.path().to_owned()], false, false, false).unwrap();
        let filesystem = Filesystem::new(policy, 4, 4);
        let path = root.path().join("sample");
        assert!(matches!(
            filesystem.write(&path, b"12345"),
            Err(FsError::Limit { .. })
        ));
        filesystem.write(&path, b"1234").unwrap();
        assert!(matches!(
            filesystem.read(&path, 0, 5),
            Err(FsError::Limit { .. })
        ));
    }

    #[test]
    fn creates_missing_parent_directories_when_requested() {
        let root = tempfile::tempdir().unwrap();
        let policy = Policy::new(vec![root.path().to_owned()], false, false, false).unwrap();
        let filesystem = Filesystem::new(policy, 1024, 1024);
        let path = root.path().join("reports").join("2026").join("result.txt");

        let (_, created) = filesystem
            .write_with_parents(&path, b"ready", true)
            .unwrap();

        assert_eq!(created.len(), 2);
        assert_eq!(filesystem.read(&path, 0, 5).unwrap(), b"ready");
    }

    #[test]
    fn requires_existing_parent_by_default() {
        let root = tempfile::tempdir().unwrap();
        let policy = Policy::new(vec![root.path().to_owned()], false, false, false).unwrap();
        let filesystem = Filesystem::new(policy, 1024, 1024);
        let path = root.path().join("missing").join("result.txt");

        assert!(filesystem.write(&path, b"ready").is_err());
        assert!(!root.path().join("missing").exists());
    }

    #[test]
    fn read_only_mode_never_creates_parent_directories() {
        let root = tempfile::tempdir().unwrap();
        let policy = Policy::new(vec![root.path().to_owned()], false, true, false).unwrap();
        let filesystem = Filesystem::new(policy, 1024, 1024);
        let path = root.path().join("missing").join("result.txt");

        assert!(matches!(
            filesystem.write_with_parents(&path, b"ready", true),
            Err(FsError::Access(AccessError::ReadOnly))
        ));
        assert!(!root.path().join("missing").exists());
    }
}
