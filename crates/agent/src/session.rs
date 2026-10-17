use crate::protocol::{Capability, CapabilityState, DeviceId, SessionId, SessionStatus};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionSnapshot {
    pub status: SessionStatus,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SessionError {
    #[error("unknown capability")]
    UnknownCapability,
}

/// Owns the one paired-session status surface for all four independent services.
#[derive(Clone, Debug)]
pub struct SessionCoordinator {
    status: SessionStatus,
}

impl SessionCoordinator {
    pub fn new(session_id: SessionId, local: DeviceId, peer: DeviceId) -> Self {
        Self {
            status: SessionStatus::new(session_id, local, peer),
        }
    }

    pub fn set_capability(
        &mut self,
        capability: Capability,
        state: CapabilityState,
    ) -> Result<(), SessionError> {
        let slot = self
            .status
            .capabilities
            .get_mut(&capability)
            .ok_or(SessionError::UnknownCapability)?;
        *slot = state;
        Ok(())
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        SessionSnapshot {
            status: self.status.clone(),
        }
    }

    pub fn all_services_available(&self) -> bool {
        Capability::ALL.iter().all(|capability| {
            matches!(
                self.status.capabilities.get(capability),
                Some(CapabilityState::Ready | CapabilityState::Active)
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coordinator() -> SessionCoordinator {
        SessionCoordinator::new(
            SessionId::new(),
            DeviceId("device-a".into()),
            DeviceId("device-b".into()),
        )
    }

    #[test]
    fn session_starts_with_all_four_capabilities_visible() {
        let state = coordinator().snapshot().status;
        assert_eq!(state.capabilities.len(), 4);
        for capability in Capability::ALL {
            assert_eq!(state.capabilities[&capability], CapabilityState::Starting);
        }
    }

    #[test]
    fn display_failure_does_not_disable_other_capabilities() {
        let mut session = coordinator();
        for capability in Capability::ALL {
            session
                .set_capability(capability, CapabilityState::Ready)
                .unwrap();
        }
        session
            .set_capability(
                Capability::ExtendedDisplay,
                CapabilityState::Blocked {
                    reason: "IDD unavailable".into(),
                },
            )
            .unwrap();

        let snapshot = session.snapshot().status;
        assert_eq!(
            snapshot.capabilities[&Capability::Input],
            CapabilityState::Ready
        );
        assert_eq!(
            snapshot.capabilities[&Capability::Files],
            CapabilityState::Ready
        );
        assert_eq!(
            snapshot.capabilities[&Capability::WindowPortal],
            CapabilityState::Ready
        );
        assert!(!session.all_services_available());
    }

    #[test]
    fn session_status_serializes_capabilities_as_protocol_names() {
        let json = serde_json::to_value(coordinator().snapshot().status).unwrap();
        assert_eq!(json["protocol_version"], crate::protocol::PROTOCOL_VERSION);
        assert!(json["capabilities"]["window_portal"].is_object());
        assert!(json["capabilities"]["extended_display"].is_object());
    }
}
