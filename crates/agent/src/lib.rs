//! Typed, transport-independent contracts and session coordination primitives.
//!
//! This crate deliberately models each capability independently: starting a
//! display stream must never implicitly stop input, files, or window portals.

pub mod identity_store;
pub mod ownership;
pub mod protocol;
pub mod qos;
pub mod session;
pub mod shared_files;
pub mod trust;

pub use identity_store::{IdentityStore, IdentityStoreError};
pub use ownership::{
    InputOwner, InputRouter, Rect, ReleasedInput, Surface, SurfaceKind, SurfaceMap,
};
pub use protocol::{Capability, CapabilityState, DeviceId, SessionId};
pub use qos::{EnqueueError, QosQueue, TrafficClass};
pub use session::{SessionCoordinator, SessionError, SessionSnapshot};
pub use shared_files::{
    SharedEntry, SharedEntryKind, SharedFilesError, SharedWorkspace, WorkspaceUpload,
};
pub use trust::{DeviceIdentity, PairingApproval, PairingTranscript, SignatureProof, TrustError};
