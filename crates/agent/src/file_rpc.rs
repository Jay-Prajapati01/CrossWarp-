//! Request/response RPC for a user-granted Shared Workspace over an already
//! authenticated, encrypted session. This layer never accepts host-absolute paths.

use crate::shared_files::{SharedEntry, SharedWorkspace, WorkspaceUpload, MAX_READ_RANGE};
use crate::transport::{EncryptedReceiver, EncryptedSender, TransportError, MAX_ENCRYPTED_MESSAGE};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

/// JSON expands byte arrays substantially, so file writes are deliberately
/// chunked below the encrypted frame limit.
pub const MAX_RPC_UPLOAD_CHUNK: usize = 200 * 1024;
pub const MAX_RPC_READ_CHUNK: usize = 200 * 1024;
const MAX_LIST_ENTRIES: usize = 500;

#[derive(Debug, Error)]
pub enum FileRpcError {
    #[error("encrypted transport failed: {0}")]
    Transport(#[from] TransportError),
    #[error("file RPC message is invalid or exceeds its limit")]
    InvalidMessage,
    #[error("remote file operation failed: {0}")]
    Remote(String),
    #[error("file RPC session has closed")]
    SessionClosed,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Packet {
    Request { id: u64, request: FileRequest },
    Response { id: u64, response: FileResponse },
}

struct OutboundRequest {
    request: FileRequest,
    response: oneshot::Sender<Result<FileResponse, FileRpcError>>,
}

/// Cloneable request handle for the remote workspace on a live session.
#[derive(Clone)]
pub struct FileRpcClient {
    requests: mpsc::Sender<OutboundRequest>,
}

impl FileRpcClient {
    pub async fn request(&self, request: FileRequest) -> Result<FileResponse, FileRpcError> {
        let (response, result) = oneshot::channel();
        self.requests
            .send(OutboundRequest { request, response })
            .await
            .map_err(|_| FileRpcError::SessionClosed)?;
        result.await.map_err(|_| FileRpcError::SessionClosed)?
    }
}

/// Starts a bidirectional RPC actor. It owns the encrypted reader and writer,
/// correlates simultaneous requests, and serves the local grant on the same
/// session while returning a client for calls to the peer.
pub fn spawn_file_rpc<S, R>(
    sender: EncryptedSender<S>,
    receiver: EncryptedReceiver<R>,
    workspace: Arc<std::sync::Mutex<Option<Arc<SharedWorkspace>>>>,
) -> (
    FileRpcClient,
    tokio::task::JoinHandle<Result<(), FileRpcError>>,
)
where
    S: AsyncWrite + Unpin + Send + 'static,
    R: AsyncRead + Unpin + Send + 'static,
{
    let (requests, incoming) = mpsc::channel(32);
    let task = tokio::spawn(serve_file_rpc(sender, receiver, workspace, incoming));
    (FileRpcClient { requests }, task)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum FileRequest {
    List {
        directory: Option<String>,
    },
    ReadRange {
        path: String,
        offset: u64,
        length: usize,
    },
    CreateDirectory {
        path: String,
    },
    BeginUpload {
        path: String,
        length: u64,
    },
    UploadChunk {
        transfer: Uuid,
        offset: u64,
        bytes: Vec<u8>,
    },
    CommitUpload {
        transfer: Uuid,
        sha256: [u8; 32],
    },
    CommitUploadReplace {
        transfer: Uuid,
        sha256: [u8; 32],
        confirmed: bool,
    },
    AbortUpload {
        transfer: Uuid,
    },
    Rename {
        source: String,
        destination: String,
        confirmed: bool,
    },
    Delete {
        path: String,
        confirmed: bool,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum FileResponse {
    Entries { entries: Vec<SharedEntry> },
    Bytes { bytes: Vec<u8> },
    Transfer { transfer: Uuid },
    Progress { written: u64 },
    Digest { sha256: [u8; 32] },
    Ok,
    Error { message: String },
}

/// Serve remote workspace calls until the authenticated session closes.
/// Ownership of the opened workspace capability stays local to the host.
async fn serve_file_rpc<S, R>(
    mut sender: EncryptedSender<S>,
    mut receiver: EncryptedReceiver<R>,
    workspace: Arc<std::sync::Mutex<Option<Arc<SharedWorkspace>>>>,
    mut outgoing: mpsc::Receiver<OutboundRequest>,
) -> Result<(), FileRpcError>
where
    S: AsyncWrite + Unpin,
    R: AsyncRead + Unpin,
{
    let mut uploads: HashMap<Uuid, WorkspaceUpload> = HashMap::new();
    let mut pending: HashMap<u64, oneshot::Sender<Result<FileResponse, FileRpcError>>> =
        HashMap::new();
    let mut next_id = 0u64;
    loop {
        tokio::select! {
            frame = receiver.receive() => {
                let frame = frame?;
                let packet: Packet = serde_json::from_slice(&frame).map_err(|_| FileRpcError::InvalidMessage)?;
                match packet {
                    Packet::Request { id, request } => {
                        let grant = workspace.lock().ok().and_then(|slot| slot.clone());
                        let response = match grant {
                            Some(grant) => dispatch(&grant, &mut uploads, request),
                            None => FileResponse::Error { message: "this device has not granted a Shared Workspace folder".into() },
                        };
                        send_packet(&mut sender, &Packet::Response { id, response }).await?;
                    }
                    Packet::Response { id, response } => {
                        if let Some(waiter) = pending.remove(&id) { let _ = waiter.send(Ok(response)); }
                    }
                }
            }
            request = outgoing.recv() => {
                let Some(request) = request else { return Ok(()) };
                let id = next_id;
                next_id = next_id.checked_add(1).ok_or(FileRpcError::InvalidMessage)?;
                pending.insert(id, request.response);
                if let Err(error) = send_packet(&mut sender, &Packet::Request { id, request: request.request }).await {
                    if let Some(waiter) = pending.remove(&id) { let _ = waiter.send(Err(FileRpcError::SessionClosed)); }
                    return Err(error);
                }
            }
        }
    }
}

async fn send_packet<S: AsyncWrite + Unpin>(
    sender: &mut EncryptedSender<S>,
    packet: &Packet,
) -> Result<(), FileRpcError> {
    let bytes = serde_json::to_vec(packet).map_err(|_| FileRpcError::InvalidMessage)?;
    if bytes.len() > MAX_ENCRYPTED_MESSAGE {
        return Err(FileRpcError::InvalidMessage);
    }
    sender.send(&bytes).await?;
    Ok(())
}

fn dispatch(
    workspace: &SharedWorkspace,
    uploads: &mut HashMap<Uuid, WorkspaceUpload>,
    request: FileRequest,
) -> FileResponse {
    let result = match request {
        FileRequest::List { directory } => workspace
            .list_directory(directory.as_deref(), MAX_LIST_ENTRIES)
            .map(|entries| FileResponse::Entries { entries }),
        FileRequest::ReadRange {
            path,
            offset,
            length,
        } => {
            if length > MAX_READ_RANGE.min(MAX_RPC_READ_CHUNK) {
                return FileResponse::Error {
                    message: "read range exceeds the limit".into(),
                };
            }
            workspace
                .read_file_range(&path, offset, length)
                .map(|bytes| FileResponse::Bytes { bytes })
        }
        FileRequest::CreateDirectory { path } => {
            workspace.create_directory(&path).map(|()| FileResponse::Ok)
        }
        FileRequest::BeginUpload { path, length } => {
            workspace.begin_upload(&path, length).map(|upload| {
                let transfer = Uuid::new_v4();
                uploads.insert(transfer, upload);
                FileResponse::Transfer { transfer }
            })
        }
        FileRequest::UploadChunk {
            transfer,
            offset,
            bytes,
        } => {
            if bytes.len() > MAX_RPC_UPLOAD_CHUNK {
                return FileResponse::Error {
                    message: "upload chunk exceeds the RPC limit".into(),
                };
            }
            match uploads.get_mut(&transfer) {
                Some(upload) => {
                    upload
                        .write_chunk(offset, &bytes)
                        .map(|()| FileResponse::Progress {
                            written: upload.written_bytes(),
                        })
                }
                None => {
                    return FileResponse::Error {
                        message: "upload transfer was not found".into(),
                    }
                }
            }
        }
        FileRequest::CommitUpload { transfer, sha256 } => match uploads.remove(&transfer) {
            Some(upload) => upload
                .commit(sha256)
                .map(|digest| FileResponse::Digest { sha256: digest }),
            None => {
                return FileResponse::Error {
                    message: "upload transfer was not found".into(),
                }
            }
        },
        FileRequest::CommitUploadReplace {
            transfer,
            sha256,
            confirmed,
        } => match uploads.remove(&transfer) {
            Some(upload) => upload
                .commit_replace(sha256, confirmed)
                .map(|digest| FileResponse::Digest { sha256: digest }),
            None => {
                return FileResponse::Error {
                    message: "upload transfer was not found".into(),
                }
            }
        },
        FileRequest::AbortUpload { transfer } => {
            if uploads.remove(&transfer).is_some() {
                Ok(FileResponse::Ok)
            } else {
                return FileResponse::Error {
                    message: "upload transfer was not found".into(),
                };
            }
        }
        FileRequest::Rename {
            source,
            destination,
            confirmed,
        } => workspace
            .rename_entry(&source, &destination, confirmed)
            .map(|()| FileResponse::Ok),
        FileRequest::Delete { path, confirmed } => workspace
            .delete_file(&path, confirmed)
            .map(|()| FileResponse::Ok),
    };
    result.unwrap_or_else(|error| FileResponse::Error {
        message: error.to_string(),
    })
}

/// Compute the digest expected by `CommitUpload` without exposing filesystem
// capabilities to callers.
pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_files::SharedEntryKind;
    use crate::transport::{pair_over_stream, HandshakeRole};
    use crate::DeviceIdentity;
    use std::{fs, path::PathBuf};

    struct TempTree(PathBuf);
    impl TempTree {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("uw-file-rpc-{}", Uuid::new_v4()));
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
    fn rpc_dispatch_keeps_paths_capability_rooted_and_uploads_verified() {
        let tree = TempTree::new();
        let workspace = SharedWorkspace::open(&tree.0).unwrap();
        let mut uploads = HashMap::new();
        assert!(matches!(
            dispatch(
                &workspace,
                &mut uploads,
                FileRequest::CreateDirectory {
                    path: "docs".into()
                }
            ),
            FileResponse::Ok
        ));
        let listing = dispatch(
            &workspace,
            &mut uploads,
            FileRequest::List { directory: None },
        );
        assert!(
            matches!(listing, FileResponse::Entries { entries } if entries[0].kind == SharedEntryKind::Directory)
        );
        let contents = b"rpc upload";
        let transfer = match dispatch(
            &workspace,
            &mut uploads,
            FileRequest::BeginUpload {
                path: "docs/readme.txt".into(),
                length: contents.len() as u64,
            },
        ) {
            FileResponse::Transfer { transfer } => transfer,
            other => panic!("unexpected response: {other:?}"),
        };
        assert!(
            matches!(dispatch(&workspace, &mut uploads, FileRequest::UploadChunk { transfer, offset: 0, bytes: contents.to_vec() }), FileResponse::Progress { written } if written == contents.len() as u64)
        );
        assert!(matches!(
            dispatch(
                &workspace,
                &mut uploads,
                FileRequest::CommitUpload {
                    transfer,
                    sha256: sha256(contents)
                }
            ),
            FileResponse::Digest { .. }
        ));
        assert_eq!(fs::read(tree.0.join("docs/readme.txt")).unwrap(), contents);
    }

    #[test]
    fn rpc_rejects_unconfirmed_delete_and_oversized_upload_frame() {
        let tree = TempTree::new();
        fs::write(tree.0.join("keep.txt"), b"safe").unwrap();
        let workspace = SharedWorkspace::open(&tree.0).unwrap();
        let mut uploads = HashMap::new();
        assert!(matches!(
            dispatch(
                &workspace,
                &mut uploads,
                FileRequest::Delete {
                    path: "keep.txt".into(),
                    confirmed: false
                }
            ),
            FileResponse::Error { .. }
        ));
        assert!(tree.0.join("keep.txt").exists());
        let response = dispatch(
            &workspace,
            &mut uploads,
            FileRequest::BeginUpload {
                path: "large.bin".into(),
                length: (MAX_RPC_UPLOAD_CHUNK + 1) as u64,
            },
        );
        let FileResponse::Transfer { transfer } = response else {
            panic!("upload setup should succeed")
        };
        assert!(matches!(
            dispatch(
                &workspace,
                &mut uploads,
                FileRequest::UploadChunk {
                    transfer,
                    offset: 0,
                    bytes: vec![0; MAX_RPC_UPLOAD_CHUNK + 1]
                }
            ),
            FileResponse::Error { .. }
        ));
        assert!(!tree.0.join("large.bin").exists());
        assert!(matches!(
            dispatch(
                &workspace,
                &mut uploads,
                FileRequest::ReadRange {
                    path: "keep.txt".into(),
                    offset: 0,
                    length: MAX_RPC_READ_CHUNK + 1,
                }
            ),
            FileResponse::Error { .. }
        ));
    }

    #[tokio::test]
    async fn file_rpc_runs_over_the_authenticated_encrypted_stream() {
        let tree = TempTree::new();
        fs::write(tree.0.join("visible.txt"), b"workspace data").unwrap();
        let workspace = Arc::new(std::sync::Mutex::new(None));
        let (client_identity, host_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (client_stream, host_stream) = tokio::io::duplex(64 * 1024);
        let (client, host) = tokio::join!(
            pair_over_stream(
                client_stream,
                &client_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            ),
            pair_over_stream(
                host_stream,
                &host_identity,
                HandshakeRole::Responder,
                |_| async { true }
            ),
        );
        let (client_sender, client_receiver, _) = client.unwrap().into_encrypted();
        let (host_sender, host_receiver, _) = host.unwrap().into_encrypted();
        let (remote, _remote_task) = spawn_file_rpc(host_sender, host_receiver, workspace.clone());
        let (local, _local_task) = spawn_file_rpc(
            client_sender,
            client_receiver,
            Arc::new(std::sync::Mutex::new(None)),
        );
        let response = local
            .request(FileRequest::List { directory: None })
            .await
            .unwrap();
        assert!(matches!(response, FileResponse::Error { .. }));
        *workspace.lock().unwrap() = Some(Arc::new(SharedWorkspace::open(&tree.0).unwrap()));
        let response = local
            .request(FileRequest::List { directory: None })
            .await
            .unwrap();
        assert!(
            matches!(response, FileResponse::Entries { entries } if entries.iter().any(|entry| entry.name == "visible.txt"))
        );
        let (client_result, host_result) = tokio::join!(
            local.request(FileRequest::List { directory: None }),
            remote.request(FileRequest::List { directory: None }),
        );
        assert!(matches!(
            client_result.unwrap(),
            FileResponse::Entries { .. }
        ));
        assert!(matches!(host_result.unwrap(), FileResponse::Error { .. }));
        let replacement = b"remote edit saved";
        let transfer = match local
            .request(FileRequest::BeginUpload {
                path: "visible.txt".into(),
                length: replacement.len() as u64,
            })
            .await
            .unwrap()
        {
            FileResponse::Transfer { transfer } => transfer,
            other => panic!("unexpected begin upload response: {other:?}"),
        };
        assert!(matches!(
            local.request(FileRequest::UploadChunk { transfer, offset: 0, bytes: replacement.to_vec() }).await.unwrap(),
            FileResponse::Progress { written } if written == replacement.len() as u64
        ));
        assert!(matches!(
            local
                .request(FileRequest::CommitUploadReplace {
                    transfer,
                    sha256: sha256(replacement),
                    confirmed: true
                })
                .await
                .unwrap(),
            FileResponse::Digest { .. }
        ));
        assert!(matches!(
            local.request(FileRequest::ReadRange { path: "visible.txt".into(), offset: 0, length: replacement.len() }).await.unwrap(),
            FileResponse::Bytes { bytes } if bytes == replacement
        ));
    }
}
