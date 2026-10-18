//! Typed, ordered input events for the encrypted session input lane.
//!
//! This module provides routing and wire validation only. It does not start
//! native capture, authorize a peer, bypass UIPI, or inject input on its own.

use crate::ownership::{InputOwner, InputRouter};
use crate::protocol::{DeviceId, SessionId};
use crate::session_transport::{ServiceSender, SessionTransportError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const MAX_INPUT_FRAME_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RoutedInput {
    PointerMove {
        x: i32,
        y: i32,
    },
    PointerButton {
        button: u8,
        x: i32,
        y: i32,
        down: bool,
    },
    Wheel {
        delta: i32,
        horizontal: bool,
        x: i32,
        y: i32,
    },
    Key {
        virtual_key: u16,
        scan_code: u16,
        extended: bool,
        down: bool,
    },
}

impl RoutedInput {
    fn validate(&self) -> Result<(), InputSessionError> {
        match self {
            Self::PointerButton { button, .. } if !(1..=5).contains(button) => {
                Err(InputSessionError::InvalidEvent)
            }
            Self::Key { virtual_key, .. } if *virtual_key > 0xFF => {
                Err(InputSessionError::InvalidEvent)
            }
            _ => Ok(()),
        }
    }

    fn held_id(&self) -> Option<String> {
        match self {
            Self::PointerButton { button, .. } => Some(format!("button:{button}")),
            Self::Key {
                virtual_key,
                scan_code,
                ..
            } => Some(format!("key:{virtual_key}:{scan_code}")),
            Self::PointerMove { .. } | Self::Wheel { .. } => None,
        }
    }

    fn is_down(&self) -> Option<bool> {
        match self {
            Self::PointerButton { down, .. } | Self::Key { down, .. } => Some(*down),
            Self::PointerMove { .. } | Self::Wheel { .. } => None,
        }
    }

    fn released(&self) -> Option<Self> {
        match self {
            Self::PointerButton { button, x, y, .. } => Some(Self::PointerButton {
                button: *button,
                x: *x,
                y: *y,
                down: false,
            }),
            Self::Key {
                virtual_key,
                scan_code,
                extended,
                ..
            } => Some(Self::Key {
                virtual_key: *virtual_key,
                scan_code: *scan_code,
                extended: *extended,
                down: false,
            }),
            Self::PointerMove { .. } | Self::Wheel { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct InputFrame {
    session_id: SessionId,
    sequence: u64,
    target: DeviceId,
    event: RoutedInput,
}

#[derive(Debug, Error)]
pub enum InputSessionError {
    #[error("input event is invalid")]
    InvalidEvent,
    #[error("input frame is malformed or exceeds its size limit")]
    InvalidFrame,
    #[error("input frame is for another session or device")]
    WrongDestination,
    #[error("input payload was sent on a non-input service channel")]
    WrongService,
    #[error("input frame sequence is out of order")]
    OutOfOrder,
    #[error("input transition repeats a down or releases an unheld key/button")]
    UnbalancedTransition,
    #[error("input ownership refers to a device outside this local peer session")]
    UnknownOwner,
    #[error("input sequence space is exhausted")]
    SequenceExhausted,
    #[error("input session transport failed: {0}")]
    Transport(#[from] SessionTransportError),
}

/// Sends ordered input only when its caller has already selected an authorized
/// remote owner. This API intentionally has no global capture or auto-enable.
pub struct InputSessionSender {
    session_id: SessionId,
    target: DeviceId,
    next_sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputDispatchResult {
    /// A malformed/repeated transition was ignored by the ownership router.
    Ignored,
    /// The event belongs to this PC and should continue through the local OS path.
    Local(InputOwner),
    /// The event belongs to the paired PC and was queued on the encrypted input lane.
    Remote(InputOwner),
}

/// Combines hit-tested ownership with the ordered encrypted input sender.
/// Callers must only feed events from an ownership-aware capture path that can
/// suppress locally delivered events when this returns `Remote`.
pub struct InputSessionController {
    local_device: DeviceId,
    remote_device: DeviceId,
    router: InputRouter,
    sender: InputSessionSender,
}

impl InputSessionController {
    pub fn new(
        session_id: SessionId,
        local_device: DeviceId,
        remote_device: DeviceId,
        router: InputRouter,
    ) -> Self {
        Self {
            sender: InputSessionSender::new(session_id, remote_device.clone()),
            local_device,
            remote_device,
            router,
        }
    }

    pub fn router_mut(&mut self) -> &mut InputRouter {
        &mut self.router
    }

    pub async fn dispatch(
        &mut self,
        channel: &ServiceSender,
        event: RoutedInput,
    ) -> Result<InputDispatchResult, InputSessionError> {
        let Some((owner, event)) = route_local_input(&mut self.router, event) else {
            return Ok(InputDispatchResult::Ignored);
        };
        if owner.device_id == self.local_device.0 {
            return Ok(InputDispatchResult::Local(owner));
        }
        if owner.device_id != self.remote_device.0 {
            return Err(InputSessionError::UnknownOwner);
        }
        self.sender.send(channel, event).await?;
        Ok(InputDispatchResult::Remote(owner))
    }
}

impl InputSessionSender {
    pub fn new(session_id: SessionId, target: DeviceId) -> Self {
        Self {
            session_id,
            target,
            next_sequence: 0,
        }
    }

    pub async fn send(
        &mut self,
        channel: &ServiceSender,
        event: RoutedInput,
    ) -> Result<(), InputSessionError> {
        if !channel.is_input_lane() {
            return Err(InputSessionError::WrongService);
        }
        event.validate()?;
        let next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(InputSessionError::SequenceExhausted)?;
        let frame = InputFrame {
            session_id: self.session_id,
            sequence: self.next_sequence,
            target: self.target.clone(),
            event,
        };
        let bytes = serde_json::to_vec(&frame).map_err(|_| InputSessionError::InvalidFrame)?;
        if bytes.len() > MAX_INPUT_FRAME_BYTES {
            return Err(InputSessionError::InvalidFrame);
        }
        channel.send(bytes).await?;
        self.next_sequence = next_sequence;
        Ok(())
    }
}

/// Validates destination, ordering, and balanced keyboard/button transitions.
/// The caller must still check explicit user consent and pass accepted events
/// to the supported Windows input API, which retains UIPI restrictions.
pub struct InputSessionReceiver {
    session_id: SessionId,
    local_device: DeviceId,
    expected_sequence: u64,
    held: BTreeMap<String, RoutedInput>,
}

impl InputSessionReceiver {
    pub fn new(session_id: SessionId, local_device: DeviceId) -> Self {
        Self {
            session_id,
            local_device,
            expected_sequence: 0,
            held: BTreeMap::new(),
        }
    }

    pub fn accept(&mut self, bytes: &[u8]) -> Result<RoutedInput, InputSessionError> {
        if bytes.is_empty() || bytes.len() > MAX_INPUT_FRAME_BYTES {
            return Err(InputSessionError::InvalidFrame);
        }
        let frame: InputFrame =
            serde_json::from_slice(bytes).map_err(|_| InputSessionError::InvalidFrame)?;
        if frame.session_id != self.session_id || frame.target != self.local_device {
            return Err(InputSessionError::WrongDestination);
        }
        if frame.sequence != self.expected_sequence {
            return Err(InputSessionError::OutOfOrder);
        }
        let next_sequence = self
            .expected_sequence
            .checked_add(1)
            .ok_or(InputSessionError::SequenceExhausted)?;
        frame.event.validate()?;
        if let (Some(id), Some(down)) = (frame.event.held_id(), frame.event.is_down()) {
            if down {
                if self.held.contains_key(&id) {
                    return Err(InputSessionError::UnbalancedTransition);
                }
                self.held.insert(id, frame.event.clone());
            } else if self.held.remove(&id).is_none() {
                return Err(InputSessionError::UnbalancedTransition);
            }
        }
        self.expected_sequence = next_sequence;
        Ok(frame.event)
    }

    /// Produce balanced release events for every held key/button on disconnect.
    pub fn release_all(&mut self) -> Vec<RoutedInput> {
        std::mem::take(&mut self.held)
            .into_values()
            .filter_map(|event| event.released())
            .collect()
    }
}

/// Routes a local event according to the session ownership map. The caller
/// sends only when the returned owner is remote; local events remain local.
pub fn route_local_input(
    router: &mut InputRouter,
    event: RoutedInput,
) -> Option<(InputOwner, RoutedInput)> {
    event.validate().ok()?;
    match &event {
        RoutedInput::PointerMove { x, y } => {
            let owner = router.focus_at(*x as f64, *y as f64).clone();
            Some((owner, event))
        }
        RoutedInput::Wheel { x, y, .. } => {
            let owner = router.owner_at(*x as f64, *y as f64).clone();
            Some((owner, event))
        }
        RoutedInput::PointerButton {
            button,
            x,
            y,
            down: true,
        } => {
            let owner = router.button_down(*button, *x as f64, *y as f64)?;
            Some((owner, event))
        }
        RoutedInput::PointerButton {
            button,
            down: false,
            ..
        } => {
            let owner = router.input_up(&format!("button:{button}"))?;
            Some((owner, event))
        }
        RoutedInput::Key {
            virtual_key,
            scan_code,
            down: true,
            ..
        } => {
            let key = format!("vk:{virtual_key}:scan:{scan_code}");
            let owner = router.key_down(key)?;
            Some((owner, event))
        }
        RoutedInput::Key {
            virtual_key,
            scan_code,
            down: false,
            ..
        } => {
            let owner = router.input_up(&format!("key:vk:{virtual_key}:scan:{scan_code}"))?;
            Some((owner, event))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ownership::{Rect, Surface, SurfaceKind};
    use crate::protocol::DeviceId;
    use crate::session_transport::spawn_session_transport;
    use crate::transport::{pair_over_stream, HandshakeRole};
    use crate::trust::DeviceIdentity;
    use tokio::io::duplex;

    fn owner(device: &str) -> InputOwner {
        InputOwner {
            device_id: device.into(),
            window_token: Some(format!("{device}-window")),
        }
    }

    #[tokio::test]
    async fn input_frames_cross_the_authenticated_input_lane_in_order() {
        let (a, b) = (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (stream_a, stream_b) = duplex(32 * 1024);
        let (paired_a, paired_b) = tokio::join!(
            pair_over_stream(stream_a, &a, HandshakeRole::Initiator, |_| async { true }),
            pair_over_stream(stream_b, &b, HandshakeRole::Responder, |_| async { true }),
        );
        let (sender_a, receiver_a, _) = paired_a.unwrap().into_encrypted();
        let (sender_b, receiver_b, _) = paired_b.unwrap().into_encrypted();
        let (channels_a, _task_a) = spawn_session_transport(sender_a, receiver_a);
        let (channels_b, _task_b) = spawn_session_transport(sender_b, receiver_b);
        let session_id = SessionId::new();
        let local_id = DeviceId("device-b".into());
        let mut input_sender = InputSessionSender::new(session_id, local_id.clone());
        let mut input_receiver = InputSessionReceiver::new(session_id, local_id);
        input_sender
            .send(
                &channels_a.input.sender,
                RoutedInput::Key {
                    virtual_key: 0x41,
                    scan_code: 0x1e,
                    extended: false,
                    down: true,
                },
            )
            .await
            .unwrap();
        let payload = channels_b.input.receiver.recv().await.unwrap();
        assert!(matches!(
            input_receiver.accept(&payload),
            Ok(RoutedInput::Key { down: true, .. })
        ));
        input_sender
            .send(
                &channels_a.input.sender,
                RoutedInput::Key {
                    virtual_key: 0x41,
                    scan_code: 0x1e,
                    extended: false,
                    down: false,
                },
            )
            .await
            .unwrap();
        let payload = channels_b.input.receiver.recv().await.unwrap();
        assert!(matches!(
            input_receiver.accept(&payload),
            Ok(RoutedInput::Key { down: false, .. })
        ));
    }

    #[tokio::test]
    async fn controller_routes_only_remote_owned_events_and_keeps_balanced_keys_with_owner() {
        let (a, b) = (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (stream_a, stream_b) = duplex(32 * 1024);
        let (paired_a, paired_b) = tokio::join!(
            pair_over_stream(stream_a, &a, HandshakeRole::Initiator, |_| async { true }),
            pair_over_stream(stream_b, &b, HandshakeRole::Responder, |_| async { true }),
        );
        let (sender_a, receiver_a, _) = paired_a.unwrap().into_encrypted();
        let (sender_b, receiver_b, _) = paired_b.unwrap().into_encrypted();
        let (channels_a, _task_a) = spawn_session_transport(sender_a, receiver_a);
        let (channels_b, _task_b) = spawn_session_transport(sender_b, receiver_b);

        let session_id = SessionId::new();
        let local_id = DeviceId("local".into());
        let remote_id = DeviceId("remote".into());
        let mut router = InputRouter::new(owner("local"));
        router.surfaces_mut().upsert(Surface {
            id: "live-window-proxy".into(),
            kind: SurfaceKind::WindowProxy,
            bounds: Rect {
                x: 100.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z_order: 4,
            owner: owner("remote"),
            interactive: true,
        });
        let mut controller =
            InputSessionController::new(session_id, local_id.clone(), remote_id.clone(), router);
        let mut receiver = InputSessionReceiver::new(session_id, remote_id);

        let local_result = controller
            .dispatch(
                &channels_a.input.sender,
                RoutedInput::PointerMove { x: 20, y: 20 },
            )
            .await
            .unwrap();
        assert!(matches!(local_result, InputDispatchResult::Local(_)));
        assert!(tokio::time::timeout(
            std::time::Duration::from_millis(10),
            channels_b.input.receiver.recv()
        )
        .await
        .is_err());

        let remote_move = controller
            .dispatch(
                &channels_a.input.sender,
                RoutedInput::PointerMove { x: 150, y: 50 },
            )
            .await
            .unwrap();
        assert!(matches!(remote_move, InputDispatchResult::Remote(_)));
        receiver
            .accept(&channels_b.input.receiver.recv().await.unwrap())
            .unwrap();

        let remote_key_down = controller
            .dispatch(
                &channels_a.input.sender,
                RoutedInput::Key {
                    virtual_key: 0x41,
                    scan_code: 0x1e,
                    extended: false,
                    down: true,
                },
            )
            .await
            .unwrap();
        assert!(matches!(remote_key_down, InputDispatchResult::Remote(_)));
        receiver
            .accept(&channels_b.input.receiver.recv().await.unwrap())
            .unwrap();

        // Move back over the local desktop: the key-up still goes to the peer
        // that received key-down, preventing a stuck key on either device.
        assert!(matches!(
            controller
                .dispatch(
                    &channels_a.input.sender,
                    RoutedInput::PointerMove { x: 20, y: 20 },
                )
                .await
                .unwrap(),
            InputDispatchResult::Local(_)
        ));
        let remote_key_up = controller
            .dispatch(
                &channels_a.input.sender,
                RoutedInput::Key {
                    virtual_key: 0x41,
                    scan_code: 0x1e,
                    extended: false,
                    down: false,
                },
            )
            .await
            .unwrap();
        assert!(matches!(remote_key_up, InputDispatchResult::Remote(_)));
        assert!(matches!(
            receiver.accept(&channels_b.input.receiver.recv().await.unwrap()),
            Ok(RoutedInput::Key { down: false, .. })
        ));
    }

    #[test]
    fn receiver_rejects_replay_wrong_target_and_unbalanced_transitions() {
        let session = SessionId::new();
        let target = DeviceId("local".into());
        let other = DeviceId("other".into());
        let encode = |sequence, target: DeviceId, down| {
            serde_json::to_vec(&InputFrame {
                session_id: session,
                sequence,
                target,
                event: RoutedInput::Key {
                    virtual_key: 0x41,
                    scan_code: 0x1e,
                    extended: false,
                    down,
                },
            })
            .unwrap()
        };
        let mut receiver = InputSessionReceiver::new(session, target.clone());
        let first = encode(0, target.clone(), true);
        assert!(receiver.accept(&first).is_ok());
        assert!(matches!(
            receiver.accept(&first),
            Err(InputSessionError::OutOfOrder)
        ));
        let mut wrong_target_receiver = InputSessionReceiver::new(session, target);
        assert!(matches!(
            wrong_target_receiver.accept(&encode(0, other, true)),
            Err(InputSessionError::WrongDestination)
        ));
        let mut unbalanced = InputSessionReceiver::new(session, DeviceId("local".into()));
        assert!(matches!(
            unbalanced.accept(&encode(0, DeviceId("local".into()), false)),
            Err(InputSessionError::UnbalancedTransition)
        ));
    }

    #[test]
    fn disconnect_releases_held_inputs_and_router_keeps_original_owner() {
        let mut map = crate::ownership::SurfaceMap::default();
        map.upsert(Surface {
            id: "portal".into(),
            kind: SurfaceKind::WindowProxy,
            bounds: Rect {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z_order: 3,
            owner: owner("peer"),
            interactive: true,
        });
        let mut router = InputRouter::new(owner("local"));
        *router.surfaces_mut() = map;
        route_local_input(&mut router, RoutedInput::PointerMove { x: 50, y: 50 }).unwrap();
        let down = route_local_input(
            &mut router,
            RoutedInput::Key {
                virtual_key: 0x41,
                scan_code: 0x1e,
                extended: false,
                down: true,
            },
        )
        .unwrap();
        assert_eq!(down.0.device_id, "peer");
        router.focus_at(200.0, 200.0);
        let up = route_local_input(
            &mut router,
            RoutedInput::Key {
                virtual_key: 0x41,
                scan_code: 0x1e,
                extended: false,
                down: false,
            },
        )
        .unwrap();
        assert_eq!(up.0.device_id, "peer");

        let mut receiver = InputSessionReceiver::new(SessionId::new(), DeviceId("local".into()));
        let encoded = serde_json::to_vec(&InputFrame {
            session_id: receiver.session_id,
            sequence: 0,
            target: DeviceId("local".into()),
            event: RoutedInput::PointerButton {
                button: 1,
                x: 0,
                y: 0,
                down: true,
            },
        })
        .unwrap();
        receiver.accept(&encoded).unwrap();
        assert_eq!(
            receiver.release_all(),
            vec![RoutedInput::PointerButton {
                button: 1,
                x: 0,
                y: 0,
                down: false
            }]
        );
    }

    #[test]
    fn wheel_routes_to_surface_under_pointer_not_stale_keyboard_focus() {
        let local = owner("local");
        let mut router = InputRouter::new(local);
        router.surfaces_mut().upsert(Surface {
            id: "portal".into(),
            kind: SurfaceKind::WindowProxy,
            bounds: Rect {
                x: 100.0,
                y: 0.0,
                width: 100.0,
                height: 100.0,
            },
            z_order: 1,
            owner: owner("remote"),
            interactive: true,
        });
        assert_eq!(router.focus_at(300.0, 50.0).device_id, "local");
        let routed = route_local_input(
            &mut router,
            RoutedInput::Wheel {
                delta: 120,
                horizontal: false,
                x: 150,
                y: 50,
            },
        )
        .unwrap();
        assert_eq!(routed.0.device_id, "remote");
        assert_eq!(router.keyboard_owner().device_id, "local");
    }
}
