//! Per-user persistent device identity protected by Windows DPAPI.
//!
//! Plaintext signing keys are never written to disk. The protected blob is user-scoped,
//! so another Windows account cannot decrypt it. This store does not sync keys between PCs.

use crate::trust::DeviceIdentity;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

const FILE_NAME: &str = "identity.dpapi";
const APP_ENTROPY: &[u8] = b"Unified Cross-PC Workspace device identity v1";

#[derive(Debug, Error)]
pub enum IdentityStoreError {
    #[error("identity store path is unavailable")]
    PathUnavailable,
    #[error("identity store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Windows DPAPI failed with error {0}")]
    Dpapi(u32),
    #[error("protected identity has an invalid length")]
    InvalidIdentity,
    #[error("DPAPI identity storage is only available on Windows")]
    UnsupportedPlatform,
}

/// Store one device signing identity in the current user's local application data.
#[derive(Clone, Debug)]
pub struct IdentityStore {
    path: PathBuf,
}

impl IdentityStore {
    pub fn for_current_user() -> Result<Self, IdentityStoreError> {
        let root = std::env::var_os("LOCALAPPDATA").ok_or(IdentityStoreError::PathUnavailable)?;
        Ok(Self::new(
            PathBuf::from(root).join("UnifiedWorkspace").join(FILE_NAME),
        ))
    }

    /// Explicit location constructor, useful for isolated tests and managed deployments.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn save(&self, identity: &DeviceIdentity) -> Result<(), IdentityStoreError> {
        let mut plaintext = identity.secret_bytes();
        let protected = protect(&plaintext);
        zeroize::Zeroize::zeroize(&mut plaintext);
        let encrypted = protected?;
        let parent = self
            .path
            .parent()
            .ok_or(IdentityStoreError::PathUnavailable)?;
        fs::create_dir_all(parent)?;

        let temp_path = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let write_result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(&encrypted)?;
            file.sync_all()?;
            replace_file(&temp_path, &self.path)
        })();
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        write_result
    }

    pub fn load(&self) -> Result<DeviceIdentity, IdentityStoreError> {
        let mut encrypted = Vec::new();
        OpenOptions::new()
            .read(true)
            .open(&self.path)?
            .take(4096)
            .read_to_end(&mut encrypted)?;
        let secret = unprotect(&encrypted)?;
        let mut secret: [u8; 32] = secret
            .try_into()
            .map_err(|_| IdentityStoreError::InvalidIdentity)?;
        let identity = DeviceIdentity::from_secret_bytes(secret);
        zeroize::Zeroize::zeroize(&mut secret);
        Ok(identity)
    }
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), IdentityStoreError> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    // REPLACE_EXISTING | WRITE_THROUGH keeps identity replacement atomic on the same volume.
    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        return Err(IdentityStoreError::Io(std::io::Error::from_raw_os_error(
            last_error() as i32,
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), IdentityStoreError> {
    fs::rename(source, destination)?;
    Ok(())
}

#[cfg(windows)]
fn protect(plaintext: &[u8]) -> Result<Vec<u8>, IdentityStoreError> {
    crypt(plaintext, true)
}

#[cfg(not(windows))]
fn protect(_plaintext: &[u8]) -> Result<Vec<u8>, IdentityStoreError> {
    Err(IdentityStoreError::UnsupportedPlatform)
}

#[cfg(windows)]
fn unprotect(ciphertext: &[u8]) -> Result<Vec<u8>, IdentityStoreError> {
    crypt(ciphertext, false)
}

#[cfg(not(windows))]
fn unprotect(_ciphertext: &[u8]) -> Result<Vec<u8>, IdentityStoreError> {
    Err(IdentityStoreError::UnsupportedPlatform)
}

#[cfg(windows)]
fn crypt(data: &[u8], encrypt: bool) -> Result<Vec<u8>, IdentityStoreError> {
    use std::ffi::c_void;

    #[repr(C)]
    struct DataBlob {
        size: u32,
        data: *mut u8,
    }

    #[link(name = "crypt32")]
    unsafe extern "system" {
        fn CryptProtectData(
            input: *const DataBlob,
            description: *const u16,
            optional_entropy: *const DataBlob,
            reserved: *const c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
        fn CryptUnprotectData(
            input: *const DataBlob,
            description: *mut *mut u16,
            optional_entropy: *const DataBlob,
            reserved: *const c_void,
            prompt: *const c_void,
            flags: u32,
            output: *mut DataBlob,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LocalFree(memory: *mut c_void) -> *mut c_void;
        fn GetLastError() -> u32;
    }

    const CRYPTPROTECT_UI_FORBIDDEN: u32 = 0x1;
    let input = DataBlob {
        size: u32::try_from(data.len()).map_err(|_| IdentityStoreError::InvalidIdentity)?,
        data: data.as_ptr() as *mut u8,
    };
    let entropy = DataBlob {
        size: APP_ENTROPY.len() as u32,
        data: APP_ENTROPY.as_ptr() as *mut u8,
    };
    let mut output = DataBlob {
        size: 0,
        data: std::ptr::null_mut(),
    };

    let succeeded = unsafe {
        if encrypt {
            CryptProtectData(
                &input,
                std::ptr::null(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptUnprotectData(
                &input,
                std::ptr::null_mut(),
                &entropy,
                std::ptr::null(),
                std::ptr::null(),
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        }
    };
    if succeeded == 0 {
        return Err(IdentityStoreError::Dpapi(unsafe { GetLastError() }));
    }
    if output.data.is_null() || output.size == 0 {
        if !output.data.is_null() {
            unsafe { LocalFree(output.data.cast()) };
        }
        return Err(IdentityStoreError::InvalidIdentity);
    }
    let result = unsafe { std::slice::from_raw_parts(output.data, output.size as usize).to_vec() };
    unsafe { LocalFree(output.data.cast()) };
    Ok(result)
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
}

#[cfg(windows)]
fn last_error() -> u32 {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetLastError() -> u32;
    }
    unsafe { GetLastError() }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::protocol::DeviceId;

    #[test]
    fn windows_dpapi_round_trip_persists_same_identity_without_plaintext_key() {
        let dir =
            std::env::temp_dir().join(format!("unified-identity-test-{}", uuid::Uuid::new_v4()));
        let store = IdentityStore::new(dir.join(FILE_NAME));
        let original = DeviceIdentity::generate();
        let original_id = original.device_id();

        store.save(&original).unwrap();
        let protected_blob = fs::read(store.path()).unwrap();
        assert_ne!(protected_blob, original.secret_bytes());
        assert_eq!(store.load().unwrap().device_id(), original_id);
        assert_eq!(
            DeviceId::from_public_key(&original.public_key()),
            original_id
        );

        fs::remove_dir_all(dir).unwrap();
    }
}
