//! Capability-rooted Shared Workspace operations.
//!
//! Every operation is relative to an open `cap_std::fs::Dir`; code never resolves
//! an untrusted peer path into a host-absolute path. Uploads are committed by an
//! atomic same-directory hard link, so an existing file is never overwritten.

use cap_std::ambient_authority;
use cap_std::fs::{Dir, File, OpenOptions};
use cap_std::time::SystemClock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const MAX_READ_RANGE: usize = 8 * 1024 * 1024;
pub const MAX_UPLOAD_CHUNK: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum SharedFilesError {
    #[error("the selected Shared Workspace root is invalid")]
    InvalidRoot,
    #[error("workspace paths must be relative and cannot contain traversal or stream components")]
    InvalidPath,
    #[error("requested entry was not found")]
    NotFound,
    #[error("requested entry has the wrong type")]
    WrongEntryType,
    #[error("a destructive or overwrite operation requires explicit confirmation")]
    ConfirmationRequired,
    #[error("read range exceeds the maximum allowed size")]
    ReadRangeTooLarge,
    #[error("upload chunk is too large or is not the next sequential chunk")]
    InvalidUploadChunk,
    #[error("upload length does not match the declared file length")]
    UploadLengthMismatch,
    #[error("upload checksum does not match the declared checksum")]
    ChecksumMismatch,
    #[error("a file or directory already exists at the destination")]
    AlreadyExists,
    #[error("filesystem operation failed: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedEntryKind {
    File,
    Directory,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SharedEntry {
    pub name: String,
    pub relative_path: String,
    pub kind: SharedEntryKind,
    pub size: u64,
    pub modified_unix_ms: Option<u128>,
}

/// A user-selected storage root held as an open directory capability.
pub struct SharedWorkspace {
    root_path: PathBuf,
    root: Dir,
}

impl SharedWorkspace {
    pub fn open(user_approved_path: impl AsRef<Path>) -> Result<Self, SharedFilesError> {
        let root_path =
            fs::canonicalize(user_approved_path).map_err(|_| SharedFilesError::InvalidRoot)?;
        if !root_path.is_dir() || root_path.file_name().is_none() {
            return Err(SharedFilesError::InvalidRoot);
        }
        let root = Dir::open_ambient_dir(&root_path, ambient_authority())
            .map_err(|_| SharedFilesError::InvalidRoot)?;
        Ok(Self { root_path, root })
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Enumerate one directory, skipping reparse/symlink entries so clients never
    /// mistake a path that leaves the grant for a workspace-owned child.
    pub fn list_directory(
        &self,
        relative_directory: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SharedEntry>, SharedFilesError> {
        let directory = match relative_directory {
            None | Some("") => self.root.open_dir(".")?,
            Some(path) => self.root.open_dir(validate_relative(Path::new(path))?)?,
        };

        let mut entries = Vec::new();
        for entry in directory.read_dir(".")? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_symlink() {
                continue;
            }
            let kind = if file_type.is_dir() {
                SharedEntryKind::Directory
            } else if file_type.is_file() {
                SharedEntryKind::File
            } else {
                continue;
            };
            let name = entry.file_name();
            let name = name.to_string_lossy().into_owned();
            if name.starts_with(".uw-upload-") && name.ends_with(".part") {
                continue;
            }
            let relative_path = join_relative(relative_directory, &name);
            // Read metadata from the opened entry handle, not by re-resolving a path.
            // If an entry is concurrently replaced with a reparse point, capability
            // open remains rooted and fails instead of observing an outside target.
            let metadata = match kind {
                SharedEntryKind::Directory => entry.open_dir()?.metadata(".")?,
                SharedEntryKind::File => entry.open()?.metadata()?,
            };
            let modified_unix_ms = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(SystemClock::UNIX_EPOCH).ok())
                .map(|duration| duration.as_millis());
            entries.push(SharedEntry {
                name,
                relative_path,
                kind,
                size: metadata.len(),
                modified_unix_ms,
            });
        }
        entries.sort_by(|left, right| {
            (left.kind != SharedEntryKind::Directory)
                .cmp(&(right.kind != SharedEntryKind::Directory))
                .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
        });
        entries.truncate(limit.min(10_000));
        Ok(entries)
    }

    /// Read a bounded range from an already-open file handle.
    pub fn read_file_range(
        &self,
        relative_file: &str,
        offset: u64,
        length: usize,
    ) -> Result<Vec<u8>, SharedFilesError> {
        if length > MAX_READ_RANGE {
            return Err(SharedFilesError::ReadRangeTooLarge);
        }
        validate_relative(Path::new(relative_file))?;
        let mut file = self.root.open(relative_file).map_err(map_not_found)?;
        if !file.metadata()?.is_file() {
            return Err(SharedFilesError::WrongEntryType);
        }
        file.seek(SeekFrom::Start(offset))?;
        let mut bytes = Vec::with_capacity(length);
        file.take(length as u64).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    pub fn create_directory(&self, relative_path: &str) -> Result<(), SharedFilesError> {
        validate_relative(Path::new(relative_path))?;
        self.root
            .create_dir(relative_path)
            .map_err(map_already_exists)
    }

    /// Delete a regular file only after the caller passes an explicit confirmation.
    pub fn delete_file(
        &self,
        relative_path: &str,
        user_confirmed: bool,
    ) -> Result<(), SharedFilesError> {
        if !user_confirmed {
            return Err(SharedFilesError::ConfirmationRequired);
        }
        let (parent, name) = self.open_parent(relative_path)?;
        let metadata = parent.symlink_metadata(&name).map_err(map_not_found)?;
        if !metadata.is_file() {
            return Err(SharedFilesError::WrongEntryType);
        }
        parent.remove_file(name).map_err(map_not_found)
    }

    /// Rename within the grant. Destination replacement is not allowed.
    pub fn rename_entry(
        &self,
        source: &str,
        destination: &str,
        user_confirmed: bool,
    ) -> Result<(), SharedFilesError> {
        if !user_confirmed {
            return Err(SharedFilesError::ConfirmationRequired);
        }
        let (source_parent, source_name) = self.open_parent(source)?;
        let (destination_parent, destination_name) = self.open_parent(destination)?;
        if destination_parent
            .symlink_metadata(&destination_name)
            .is_ok()
        {
            return Err(SharedFilesError::AlreadyExists);
        }
        source_parent
            .rename(source_name, &destination_parent, destination_name)
            .map_err(map_already_exists)
    }

    /// Start an upload to a new name. The existing destination is never replaced.
    pub fn begin_upload(
        &self,
        relative_destination: &str,
        declared_length: u64,
    ) -> Result<WorkspaceUpload, SharedFilesError> {
        let (parent, destination_name) = self.open_parent(relative_destination)?;
        let temp_name = OsString::from(format!(".uw-upload-{}.part", Uuid::new_v4()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        let file = parent.open_with(&temp_name, &options)?;
        Ok(WorkspaceUpload {
            parent,
            temp_name,
            destination_name,
            file: Some(file),
            declared_length,
            written: 0,
            digest: Sha256::new(),
            committed: false,
        })
    }

    fn open_parent(&self, relative_path: &str) -> Result<(Dir, OsString), SharedFilesError> {
        let path = validate_relative(Path::new(relative_path))?;
        let name = path
            .file_name()
            .ok_or(SharedFilesError::InvalidPath)?
            .to_os_string();
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty());
        let directory = match parent {
            Some(parent) => self.root.open_dir(parent)?,
            None => self.root.open_dir(".")?,
        };
        Ok((directory, name))
    }
}

/// Sequential, bounded upload transaction. The destination name appears atomically
/// only after length and SHA-256 verification; abandoning it removes the temp file.
pub struct WorkspaceUpload {
    parent: Dir,
    temp_name: OsString,
    destination_name: OsString,
    file: Option<File>,
    declared_length: u64,
    written: u64,
    digest: Sha256,
    committed: bool,
}

impl WorkspaceUpload {
    pub fn written_bytes(&self) -> u64 {
        self.written
    }

    pub fn write_chunk(&mut self, offset: u64, bytes: &[u8]) -> Result<(), SharedFilesError> {
        if bytes.len() > MAX_UPLOAD_CHUNK
            || offset != self.written
            || self.written.saturating_add(bytes.len() as u64) > self.declared_length
        {
            return Err(SharedFilesError::InvalidUploadChunk);
        }
        let file = self
            .file
            .as_mut()
            .ok_or(SharedFilesError::InvalidUploadChunk)?;
        file.write_all(bytes)?;
        self.digest.update(bytes);
        self.written += bytes.len() as u64;
        Ok(())
    }

    pub fn commit(mut self, expected_sha256: [u8; 32]) -> Result<[u8; 32], SharedFilesError> {
        if self.written != self.declared_length {
            return Err(SharedFilesError::UploadLengthMismatch);
        }
        let actual: [u8; 32] = self.digest.clone().finalize().into();
        if actual != expected_sha256 {
            return Err(SharedFilesError::ChecksumMismatch);
        }
        let file = self
            .file
            .take()
            .ok_or(SharedFilesError::InvalidUploadChunk)?;
        file.sync_all()?;
        drop(file);

        // Hard-link creation is atomic and fails if the destination already exists.
        self.parent
            .hard_link(&self.temp_name, &self.parent, &self.destination_name)
            .map_err(map_already_exists)?;
        self.committed = true;
        let _ = self.parent.remove_file(&self.temp_name);
        Ok(actual)
    }
}

impl Drop for WorkspaceUpload {
    fn drop(&mut self) {
        self.file.take();
        if !self.committed {
            let _ = self.parent.remove_file(&self.temp_name);
        }
    }
}

fn validate_relative(path: &Path) -> Result<&Path, SharedFilesError> {
    if path.as_os_str().is_empty()
        || path.components().any(|component| match component {
            Component::Normal(name) => name.to_string_lossy().contains(':'),
            _ => true,
        })
    {
        return Err(SharedFilesError::InvalidPath);
    }
    Ok(path)
}

fn join_relative(parent: Option<&str>, name: &str) -> String {
    match parent.filter(|parent| !parent.is_empty()) {
        Some(parent) => format!("{parent}/{name}"),
        None => name.to_owned(),
    }
}

fn map_not_found(error: std::io::Error) -> SharedFilesError {
    if error.kind() == std::io::ErrorKind::NotFound {
        SharedFilesError::NotFound
    } else {
        SharedFilesError::Io(error)
    }
}

fn map_already_exists(error: std::io::Error) -> SharedFilesError {
    if error.kind() == std::io::ErrorKind::AlreadyExists {
        SharedFilesError::AlreadyExists
    } else {
        SharedFilesError::Io(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempTree(PathBuf);

    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("unified-files-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TempTree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn workspace(tree: &TempTree) -> SharedWorkspace {
        SharedWorkspace::open(&tree.0).unwrap()
    }

    #[test]
    fn directory_listing_and_range_reads_use_the_grant() {
        let tree = TempTree::new();
        fs::create_dir(tree.0.join("docs")).unwrap();
        fs::write(tree.0.join("docs/readme.txt"), b"abcdef").unwrap();
        let service = workspace(&tree);

        let entries = service.list_directory(None, 10).unwrap();
        assert_eq!(entries[0].name, "docs");
        assert_eq!(entries[0].kind, SharedEntryKind::Directory);
        assert_eq!(
            service.read_file_range("docs/readme.txt", 2, 3).unwrap(),
            b"cde"
        );
    }

    #[test]
    fn uploads_commit_atomically_only_after_length_and_hash_match() {
        let tree = TempTree::new();
        let service = workspace(&tree);
        let contents = b"new shared file";
        let expected: [u8; 32] = Sha256::digest(contents).into();
        let mut upload = service
            .begin_upload("report.txt", contents.len() as u64)
            .unwrap();
        upload.write_chunk(0, &contents[..4]).unwrap();
        upload.write_chunk(4, &contents[4..]).unwrap();
        assert_eq!(upload.commit(expected).unwrap(), expected);
        assert_eq!(fs::read(tree.0.join("report.txt")).unwrap(), contents);
    }

    #[test]
    fn wrong_upload_hash_does_not_publish_a_destination() {
        let tree = TempTree::new();
        let service = workspace(&tree);
        let mut upload = service.begin_upload("report.txt", 3).unwrap();
        upload.write_chunk(0, b"abc").unwrap();
        assert!(matches!(
            upload.commit([0; 32]),
            Err(SharedFilesError::ChecksumMismatch)
        ));
        assert!(!tree.0.join("report.txt").exists());
        assert_eq!(fs::read_dir(&tree.0).unwrap().count(), 0);
    }

    #[test]
    fn upload_never_overwrites_existing_destination() {
        let tree = TempTree::new();
        fs::write(tree.0.join("report.txt"), b"original").unwrap();
        let service = workspace(&tree);
        let contents = b"replacement";
        let mut upload = service
            .begin_upload("report.txt", contents.len() as u64)
            .unwrap();
        upload.write_chunk(0, contents).unwrap();
        let expected: [u8; 32] = Sha256::digest(contents).into();
        assert!(matches!(
            upload.commit(expected),
            Err(SharedFilesError::AlreadyExists)
        ));
        assert_eq!(fs::read(tree.0.join("report.txt")).unwrap(), b"original");
    }

    #[test]
    fn traversal_and_alternate_data_stream_paths_are_rejected() {
        let tree = TempTree::new();
        let service = workspace(&tree);
        assert!(matches!(
            service.read_file_range("../outside", 0, 10),
            Err(SharedFilesError::InvalidPath)
        ));
        assert!(matches!(
            service.begin_upload("file.txt:secret", 1),
            Err(SharedFilesError::InvalidPath)
        ));
    }

    #[test]
    fn delete_and_rename_require_explicit_confirmation() {
        let tree = TempTree::new();
        fs::write(tree.0.join("old.txt"), b"data").unwrap();
        let service = workspace(&tree);
        assert!(matches!(
            service.rename_entry("old.txt", "new.txt", false),
            Err(SharedFilesError::ConfirmationRequired)
        ));
        assert!(matches!(
            service.delete_file("old.txt", false),
            Err(SharedFilesError::ConfirmationRequired)
        ));
        service.rename_entry("old.txt", "new.txt", true).unwrap();
        service.delete_file("new.txt", true).unwrap();
        assert!(!tree.0.join("new.txt").exists());
    }
}
