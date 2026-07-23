//! Filesystem access-policy enforcement.
//!
//! [`Policy`] canonicalizes configured roots and validates every path before an
//! operation. This prevents lexical `..` traversal from escaping a root and,
//! when configured, rejects paths that traverse symbolic links.

use std::{
    fs,
    path::{Component, Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Error)]
/// A path-policy validation failure.
pub enum AccessError {
    #[error("path is outside allowed roots: {0}")]
    /// The canonical path is not below any configured root.
    OutsideRoot(String),
    #[error("write operations are disabled")]
    /// A write was requested while the policy is read-only.
    ReadOnly,
    #[error("path cannot be resolved: {0}")]
    /// A path could not be resolved or violated link policy.
    Invalid(String),
    #[error(transparent)]
    /// The operating system rejected path preparation.
    Io(#[from] std::io::Error),
}

#[derive(Clone, Debug)]
/// An immutable, cloneable filesystem access policy.
///
/// Configured roots are canonicalized during construction. Path checks compare
/// canonical paths, rather than untrusted lexical path components.
pub struct Policy {
    roots: Vec<PathBuf>,
    read_only: bool,
    follow_links: bool,
}

impl Policy {
    /// Creates a policy from allowed roots and operation flags.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError::Invalid`] when no roots are supplied or a root
    /// cannot be canonicalized.
    pub fn new(
        roots: Vec<PathBuf>,
        read_only: bool,
        follow_links: bool,
    ) -> Result<Self, AccessError> {
        if roots.is_empty() {
            return Err(AccessError::Invalid("no allowed roots configured".into()));
        }
        let roots = roots
            .into_iter()
            .map(|path| {
                fs::canonicalize(&path)
                    .map_err(|_| AccessError::Invalid(path.display().to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            roots,
            read_only,
            follow_links,
        })
    }

    /// Resolves and authorizes an existing path for reading.
    ///
    /// # Errors
    ///
    /// Returns an error if the path cannot be canonicalized, escapes all roots,
    /// or traverses a symbolic link while link following is disabled.
    pub fn read_path(&self, path: impl AsRef<Path>) -> Result<PathBuf, AccessError> {
        let path = path.as_ref();
        if !self.follow_links && contains_link(path) {
            return Err(AccessError::Invalid("symbolic links are disabled".into()));
        }
        let resolved =
            fs::canonicalize(path).map_err(|_| AccessError::Invalid(path.display().to_string()))?;
        self.ensure_root(&resolved)?;
        Ok(resolved)
    }

    /// Resolves and authorizes a destination path for writing.
    ///
    /// The destination itself may not exist, but its parent must exist and pass
    /// root and symbolic-link validation.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError::ReadOnly`] when writes are disabled.
    pub fn write_path(&self, path: impl AsRef<Path>) -> Result<PathBuf, AccessError> {
        if self.read_only {
            return Err(AccessError::ReadOnly);
        }
        let path = path.as_ref();
        let parent = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        if !self.follow_links && contains_link(parent) {
            return Err(AccessError::Invalid("symbolic links are disabled".into()));
        }
        let parent = fs::canonicalize(parent)
            .map_err(|_| AccessError::Invalid(parent.display().to_string()))?;
        self.ensure_root(&parent)?;
        let name = path
            .file_name()
            .ok_or_else(|| AccessError::Invalid(path.display().to_string()))?;
        Ok(parent.join(name))
    }

    /// Resolves a write destination and optionally creates missing parent directories.
    ///
    /// Missing directories are created only after the nearest existing ancestor has
    /// passed root and symbolic-link validation. Parent traversal (`..`) is rejected
    /// when directory creation is requested so authorization stays unambiguous.
    pub fn write_path_with_parents(
        &self,
        path: impl AsRef<Path>,
        create_parents: bool,
    ) -> Result<(PathBuf, Vec<PathBuf>), AccessError> {
        if !create_parents {
            return Ok((self.write_path(path)?, Vec::new()));
        }
        if self.read_only {
            return Err(AccessError::ReadOnly);
        }

        let path = path.as_ref();
        if path
            .components()
            .any(|component| matches!(component, Component::ParentDir))
        {
            return Err(AccessError::Invalid(
                "parent traversal is not allowed when createParents is enabled".into(),
            ));
        }
        let name = path
            .file_name()
            .ok_or_else(|| AccessError::Invalid(path.display().to_string()))?;
        let parent = path
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut ancestor = if parent.is_absolute() {
            parent.to_owned()
        } else {
            std::env::current_dir()?.join(parent)
        };
        let mut missing = Vec::new();
        while !ancestor.exists() {
            let component = ancestor
                .file_name()
                .ok_or_else(|| AccessError::Invalid(parent.display().to_string()))?;
            missing.push(component.to_os_string());
            ancestor = ancestor
                .parent()
                .ok_or_else(|| AccessError::Invalid(parent.display().to_string()))?
                .to_owned();
        }

        if !self.follow_links && contains_link(&ancestor) {
            return Err(AccessError::Invalid("symbolic links are disabled".into()));
        }
        let mut resolved = fs::canonicalize(&ancestor)?;
        self.ensure_root(&resolved)?;
        if !resolved.is_dir() {
            return Err(std::io::Error::from(std::io::ErrorKind::NotADirectory).into());
        }

        let mut created = Vec::new();
        for component in missing.into_iter().rev() {
            resolved.push(component);
            match fs::create_dir(&resolved) {
                Ok(()) => created.push(resolved.clone()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error.into()),
            }
            let metadata = fs::symlink_metadata(&resolved)?;
            if (!self.follow_links && metadata.file_type().is_symlink()) || !metadata.is_dir() {
                return Err(AccessError::Invalid(resolved.display().to_string()));
            }
        }

        let resolved_parent = fs::canonicalize(&resolved)?;
        self.ensure_root(&resolved_parent)?;
        Ok((resolved_parent.join(name), created))
    }

    fn ensure_root(&self, path: &Path) -> Result<(), AccessError> {
        self.roots
            .iter()
            .any(|root| path.starts_with(root))
            .then_some(())
            .ok_or_else(|| AccessError::OutsideRoot(path.display().to_string()))
    }
}

/// Converts a path to a stable display string.
///
/// On Windows, verbatim path prefixes introduced by canonicalization are
/// removed while UNC paths retain their conventional leading backslashes.
pub fn display_path(path: &Path) -> String {
    let value = path.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = value.strip_prefix("\\\\?\\UNC\\") {
            return format!("\\\\{rest}");
        }
        if let Some(rest) = value.strip_prefix("\\\\?\\") {
            return rest.to_owned();
        }
    }
    value.into_owned()
}

fn contains_link(path: &Path) -> bool {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        if fs::symlink_metadata(&current)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_paths_outside_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let policy = Policy::new(vec![root.path().to_owned()], false, false).unwrap();
        assert!(matches!(
            policy.read_path(outside.path()),
            Err(AccessError::OutsideRoot(_))
        ));
    }

    #[test]
    fn read_only_rejects_writes() {
        let root = tempfile::tempdir().unwrap();
        let policy = Policy::new(vec![root.path().to_owned()], true, false).unwrap();
        assert!(matches!(
            policy.write_path(root.path().join("file")),
            Err(AccessError::ReadOnly)
        ));
    }

    #[test]
    fn accepts_single_component_write_path() {
        let root = std::env::current_dir().unwrap();
        let policy = Policy::new(vec![root], false, false).unwrap();
        assert!(policy.write_path("new-file").is_ok());
    }
}
