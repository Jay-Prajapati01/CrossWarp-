//! Typed, transport-independent contracts and session coordination primitives.
//!
//! This crate deliberately models each capability independently: starting a
//! display stream must never implicitly stop input, files, or window portals.

pub mod file_rpc;
pub mod identity_store;
pub mod lan;
pub mod ownership;
pub mod peer_store;
pub mod platform;
pub mod protocol;
pub mod qos;
pub mod session;
pub mod shared_files;
pub mod transport;
pub mod trust;
#[cfg(windows)]
pub mod windows_input;

pub use file_rpc::{
    spawn_file_rpc, FileRequest, FileResponse, FileRpcClient, FileRpcError, MAX_RPC_READ_CHUNK,
    MAX_RPC_UPLOAD_CHUNK,
};
pub use identity_store::{IdentityStore, IdentityStoreError};
pub use lan::{DiscoveredPeer, LanEndpoint, LanError, PeerBrowser};
pub use ownership::{
    InputOwner, InputRouter, Rect, ReleasedInput, Surface, SurfaceKind, SurfaceMap,
};
pub use peer_store::{PeerStoreError, TrustedPeer, TrustedPeerStore};
pub use platform::{HostPlatform, WindowsVersion};
pub use protocol::{Capability, CapabilityState, DeviceId, SessionId};
pub use qos::{EnqueueError, QosQueue, TrafficClass};
pub use session::{SessionCoordinator, SessionError, SessionSnapshot};
pub use shared_files::{
    SharedEntry, SharedEntryKind, SharedFilesError, SharedWorkspace, WorkspaceUpload,
};
pub use transport::{
    connect_trusted_over_stream, pair_over_stream, EncryptedReceiver, EncryptedSender,
    HandshakeRole, PairedTransport, SessionChallenge, SessionKeys, TransportError,
    MAX_ENCRYPTED_MESSAGE,
};
pub use trust::{DeviceIdentity, PairingApproval, PairingTranscript, SignatureProof, TrustError};
