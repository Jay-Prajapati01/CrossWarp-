//! Durable, current-Windows-user protected peer approvals.

use crate::identity_store::{protect_with_entropy, unprotect_with_entropy, IdentityStoreError};
use crate::protocol::DeviceId;
use crate::trust::PairingApproval;
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

const FILE_NAME: &str = "trusted-peers.dpapi";
const APP_ENTROPY: &[u8] = b"Unified Cross-PC Workspace trusted peers v1";
const MAX_STORE_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TrustedPeer {
    pub device_id: DeviceId,
    pub public_key: [u8; 32],
}

#[derive(Debug, Error)]
pub enum PeerStoreError {
    #[error("peer store path is unavailable")]
    PathUnavailable,
    #[error("peer store I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("Windows DPAPI peer protection failed: {0}")]
    Protected(#[from] IdentityStoreError),
    #[error("only mutually approved pairings may be stored")]
    NotApproved,
    #[error("trusted peer identity does not match its public key")]
    IdentityMismatch,
    #[error("protected peer store is malformed or exceeds its size limit")]
    InvalidStore,
}

#[derive(Clone, Debug)]
pub struct TrustedPeerStore {
    path: PathBuf,
}

impl TrustedPeerStore {
    pub fn for_current_user() -> Result<Self, PeerStoreError> {
        let root = std::env::var_os("LOCALAPPDATA").ok_or(PeerStoreError::PathUnavailable)?;
        Ok(Self::new(
            PathBuf::from(root).join("UnifiedWorkspace").join(FILE_NAME),
        ))
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn list(&self) -> Result<Vec<TrustedPeer>, PeerStoreError> {
        self.read_all()
    }

    /// Persist a peer only after this endpoint has approved locally and verified
    /// the other endpoint's signature over the exact same pairing transcript.
    pub fn add_approved(&self, approval: &PairingApproval) -> Result<TrustedPeer, PeerStoreError> {
        let public_key = approval
            .approved_peer_public_key()
            .ok_or(PeerStoreError::NotApproved)?;
        let device_id = DeviceId::from_public_key(&public_key);
        let peer = TrustedPeer {
            device_id,
            public_key,
        };
        let mut peers = self.read_all()?;
        peers.retain(|existing| existing.device_id != peer.device_id);
        peers.push(peer.clone());
        peers.sort_by(|left, right| left.device_id.cmp(&right.device_id));
        self.write_all(&peers)?;
        Ok(peer)
    }

    pub fn remove(&self, device_id: &DeviceId) -> Result<bool, PeerStoreError> {
        let mut peers = self.read_all()?;
        let old_len = peers.len();
        peers.retain(|peer| &peer.device_id != device_id);
        if peers.len() == old_len {
            return Ok(false);
        }
        self.write_all(&peers)?;
        Ok(true)
    }

    fn read_all(&self) -> Result<Vec<TrustedPeer>, PeerStoreError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let metadata = fs::metadata(&self.path)?;
        if metadata.len() > MAX_STORE_SIZE {
            return Err(PeerStoreError::InvalidStore);
        }
        let mut encrypted = Vec::with_capacity(metadata.len() as usize);
        OpenOptions::new()
            .read(true)
            .open(&self.path)?
            .take(MAX_STORE_SIZE + 1)
            .read_to_end(&mut encrypted)?;
        if encrypted.len() as u64 > MAX_STORE_SIZE {
            return Err(PeerStoreError::InvalidStore);
        }
        let mut plaintext = unprotect_with_entropy(&encrypted, APP_ENTROPY)?;
        let parsed = serde_json::from_slice::<Vec<TrustedPeer>>(&plaintext)
            .map_err(|_| PeerStoreError::InvalidStore);
        zeroize::Zeroize::zeroize(&mut plaintext);
        let peers = parsed?;
        if peers
            .iter()
            .any(|peer| DeviceId::from_public_key(&peer.public_key) != peer.device_id)
        {
            return Err(PeerStoreError::IdentityMismatch);
        }
        Ok(peers)
    }

    fn write_all(&self, peers: &[TrustedPeer]) -> Result<(), PeerStoreError> {
        let plaintext = serde_json::to_vec(peers).map_err(|_| PeerStoreError::InvalidStore)?;
        let encrypted = protect_with_entropy(&plaintext, APP_ENTROPY)?;
        if encrypted.len() as u64 > MAX_STORE_SIZE {
            return Err(PeerStoreError::InvalidStore);
        }
        let parent = self.path.parent().ok_or(PeerStoreError::PathUnavailable)?;
        fs::create_dir_all(parent)?;
        let temp_path = self
            .path
            .with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp_path)?;
            file.write_all(&encrypted)?;
            file.sync_all()?;
            replace_file(&temp_path, &self.path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> Result<(), PeerStoreError> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
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
        return Err(PeerStoreError::Io(std::io::Error::from_raw_os_error(
            unsafe { GetLastError() } as i32,
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> Result<(), PeerStoreError> {
    fs::rename(source, destination)?;
    Ok(())
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn MoveFileExW(existing: *const u16, new: *const u16, flags: u32) -> i32;
    fn GetLastError() -> u32;
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::trust::{DeviceIdentity, PairingTranscript};

    fn approved_pair() -> (PairingApproval, DeviceIdentity, DeviceIdentity) {
        let local = DeviceIdentity::generate();
        let peer = DeviceIdentity::generate();
        let transcript = PairingTranscript::create(local.device_id(), peer.device_id());
        let mut local_approval =
            PairingApproval::new(transcript.clone(), local.device_id(), peer.device_id()).unwrap();
        let mut peer_approval =
            PairingApproval::new(transcript, peer.device_id(), local.device_id()).unwrap();
        let local_proof = local_approval.approve_local(&local).unwrap();
        let peer_proof = peer_approval.approve_local(&peer).unwrap();
        local_approval.record_peer_approval(peer_proof).unwrap();
        peer_approval.record_peer_approval(local_proof).unwrap();
        (local_approval, local, peer)
    }

    #[test]
    fn only_mutual_approval_is_persisted_and_peer_survives_restart() {
        let root = std::env::temp_dir().join(format!("unified-peers-{}", uuid::Uuid::new_v4()));
        let store = TrustedPeerStore::new(root.join(FILE_NAME));
        let local = DeviceIdentity::generate();
        let peer = DeviceIdentity::generate();
        let challenge = PairingTranscript::create(local.device_id(), peer.device_id());
        let mut partial =
            PairingApproval::new(challenge, local.device_id(), peer.device_id()).unwrap();
        partial.approve_local(&local).unwrap();
        assert!(matches!(
            store.add_approved(&partial),
            Err(PeerStoreError::NotApproved)
        ));

        let (approved, _, peer) = approved_pair();
        let saved = store.add_approved(&approved).unwrap();
        assert_eq!(saved.device_id, peer.device_id());
        let on_disk = fs::read(store.path()).unwrap();
        assert!(!on_disk
            .windows(32)
            .any(|window| window == peer.public_key()));
        assert_eq!(store.list().unwrap(), vec![saved.clone()]);
        assert!(store.remove(&saved.device_id).unwrap());
        assert!(store.list().unwrap().is_empty());
        let _ = fs::remove_dir_all(root);
    }
}
