use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WorkspaceError {
    #[error("the selected shared root does not exist or is not a directory")]
    InvalidRoot,
    #[error("workspace paths must be relative and cannot contain traversal components")]
    InvalidRelativePath,
    #[error("the requested path does not exist")]
    NotFound,
    #[error("the requested path resolves outside the authorized root")]
    OutsideRoot,
    #[error("filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
}

/// A canonicalized, explicit grant to one directory. It never grants a drive/root.
#[derive(Clone, Debug)]
pub struct AuthorizedRoot {
    canonical_root: PathBuf,
}

impl AuthorizedRoot {
    pub fn new(user_approved_path: impl AsRef<Path>) -> Result<Self, WorkspaceError> {
        let canonical_root =
            fs::canonicalize(user_approved_path).map_err(|_| WorkspaceError::InvalidRoot)?;
        if !canonical_root.is_dir() || canonical_root.file_name().is_none() {
            return Err(WorkspaceError::InvalidRoot);
        }
        Ok(Self { canonical_root })
    }

    pub fn root(&self) -> &Path {
        &self.canonical_root
    }

    /// Resolve an existing user-relative path and reject symlinks/reparse targets outside root.
    pub fn resolve_existing(&self, relative: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        let relative = validate_relative(relative.as_ref())?;
        let joined = self.canonical_root.join(relative);
        let resolved = fs::canonicalize(joined).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                WorkspaceError::NotFound
            } else {
                WorkspaceError::Io(error)
            }
        })?;
        if !resolved.starts_with(&self.canonical_root) {
            return Err(WorkspaceError::OutsideRoot);
        }
        Ok(resolved)
    }

    /// Resolve a not-yet-created file by canonicalizing its existing parent directory.
    pub fn resolve_new_file(&self, relative: impl AsRef<Path>) -> Result<PathBuf, WorkspaceError> {
        let relative = validate_relative(relative.as_ref())?;
        let name = relative
            .file_name()
            .filter(|name| !name.is_empty())
            .ok_or(WorkspaceError::InvalidRelativePath)?;
        let parent = relative
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let parent = match parent {
            Some(parent) => self.resolve_existing(parent)?,
            None => self.canonical_root.clone(),
        };
        if !parent.is_dir() {
            return Err(WorkspaceError::InvalidRelativePath);
        }
        let destination = parent.join(name);
        match fs::symlink_metadata(&destination) {
            Ok(_) => return self.resolve_existing(relative),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(WorkspaceError::Io(error)),
        }
        if !destination.starts_with(&self.canonical_root) {
            return Err(WorkspaceError::OutsideRoot);
        }
        Ok(destination)
    }
}

fn validate_relative(path: &Path) -> Result<&Path, WorkspaceError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(WorkspaceError::InvalidRelativePath);
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("unified-workspace-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn root_grant_resolves_files_inside_the_selected_directory() {
        let tree = TempTree::new();
        fs::write(tree.0.join("notes.txt"), "test").unwrap();
        let grant = AuthorizedRoot::new(&tree.0).unwrap();
        assert_eq!(
            grant.resolve_existing("notes.txt").unwrap(),
            fs::canonicalize(tree.0.join("notes.txt")).unwrap()
        );
    }

    #[test]
    fn parent_and_absolute_paths_are_rejected() {
        let tree = TempTree::new();
        let grant = AuthorizedRoot::new(&tree.0).unwrap();
        assert!(matches!(
            grant.resolve_existing("../outside.txt"),
            Err(WorkspaceError::InvalidRelativePath)
        ));
        assert!(matches!(
            grant.resolve_existing(tree.0.join("absolute.txt")),
            Err(WorkspaceError::InvalidRelativePath)
        ));
    }

    #[test]
    fn new_file_target_stays_under_an_existing_authorized_parent() {
        let tree = TempTree::new();
        fs::create_dir(tree.0.join("docs")).unwrap();
        let grant = AuthorizedRoot::new(&tree.0).unwrap();
        assert_eq!(
            grant.resolve_new_file("docs/new.txt").unwrap(),
            fs::canonicalize(tree.0.join("docs"))
                .unwrap()
                .join("new.txt")
        );
    }

    #[test]
    fn nonexistent_parent_is_not_created_as_a_side_effect() {
        let tree = TempTree::new();
        let grant = AuthorizedRoot::new(&tree.0).unwrap();
        assert!(matches!(
            grant.resolve_new_file("missing/new.txt"),
            Err(WorkspaceError::NotFound)
        ));
        assert!(!tree.0.join("missing").exists());
    }
}
