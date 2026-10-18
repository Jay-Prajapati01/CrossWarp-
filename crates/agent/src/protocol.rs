use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

pub const PROTOCOL_VERSION: u16 = 4;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(pub String);

impl DeviceId {
    /// Construct an ID from a public-key fingerprint rendered as lowercase hex.
    pub fn from_public_key(public_key: &[u8]) -> Self {
        use sha2::Digest;
        Self(hex::encode(sha2::Sha256::digest(public_key)))
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Input,
    Files,
    ExtendedDisplay,
    WindowPortal,
}

impl Capability {
    pub const ALL: [Self; 4] = [
        Self::Input,
        Self::Files,
        Self::ExtendedDisplay,
        Self::WindowPortal,
    ];
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum CapabilityState {
    #[default]
    Starting,
    Ready,
    Active,
    Degraded {
        reason: String,
    },
    Blocked {
        reason: String,
    },
    Offline,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SessionStatus {
    pub protocol_version: u16,
    pub session_id: SessionId,
    pub local_device_id: DeviceId,
    pub peer_device_id: DeviceId,
    pub capabilities: BTreeMap<Capability, CapabilityState>,
}

impl SessionStatus {
    pub fn new(session_id: SessionId, local_device_id: DeviceId, peer_device_id: DeviceId) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            session_id,
            local_device_id,
            peer_device_id,
            capabilities: Capability::ALL
                .into_iter()
                .map(|capability| (capability, CapabilityState::default()))
                .collect(),
        }
    }
}
