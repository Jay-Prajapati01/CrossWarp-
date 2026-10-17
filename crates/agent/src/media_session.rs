//! Typed encoded-video frames for the independent window and display lanes.
//!
//! This validates the media transport boundary only. Capture, encoding,
//! decoding, rendering, consent, and native display integration are separate.

use crate::protocol::{DeviceId, SessionId};
use crate::session_transport::{ServiceSender, SessionTransportError};
use crate::transport::MAX_ENCRYPTED_MESSAGE;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// JSON envelope overhead is reserved inside the encrypted record limit.
pub const MAX_MEDIA_PAYLOAD_BYTES: usize = MAX_ENCRYPTED_MESSAGE - 512;
const MAX_DIMENSION: u32 = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoCodec {
    H264,
    Hevc,
    Av1,
    Bgra8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaLane {
    Window,
    Display,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MediaFrame {
    pub session_id: SessionId,
    pub target: DeviceId,
    pub stream_id: u64,
    pub sequence: u64,
    /// Monotonic source clock in 100 ns units.
    pub timestamp_100ns: u64,
    pub width: u32,
    pub height: u32,
    pub codec: VideoCodec,
    pub key_frame: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum MediaSessionError {
    #[error("media frame fields or payload are invalid")]
    InvalidFrame,
    #[error("media frame is for another session or device")]
    WrongDestination,
    #[error("media frame was sent on the wrong service lane")]
    WrongLane,
    #[error("media frame sequence or timestamp was replayed or reordered")]
    OutOfOrder,
    #[error("media session transport failed: {0}")]
    Transport(#[from] SessionTransportError),
}

impl MediaFrame {
    fn validate(&self) -> Result<(), MediaSessionError> {
        if self.width == 0
            || self.height == 0
            || self.width > MAX_DIMENSION
            || self.height > MAX_DIMENSION
            || self.payload.is_empty()
            || self.payload.len() > MAX_MEDIA_PAYLOAD_BYTES
        {
            return Err(MediaSessionError::InvalidFrame);
        }
        Ok(())
    }
}

/// Video is lossy under queue pressure: producers use `try_send` so stale
/// frames can be dropped without stalling input or file work.
pub async fn send_media_frame(
    channel: &ServiceSender,
    lane: MediaLane,
    frame: &MediaFrame,
) -> Result<(), MediaSessionError> {
    frame.validate()?;
    let expected = match lane {
        MediaLane::Window => channel.is_window_video_lane(),
        MediaLane::Display => channel.is_display_video_lane(),
    };
    if !expected {
        return Err(MediaSessionError::WrongLane);
    }
    let bytes = serde_json::to_vec(frame).map_err(|_| MediaSessionError::InvalidFrame)?;
    if bytes.len() > MAX_ENCRYPTED_MESSAGE - 1 {
        return Err(MediaSessionError::InvalidFrame);
    }
    channel.try_send(bytes)?;
    Ok(())
}

/// Validates stream identity and monotonic source time. Sequence gaps are
/// allowed because the bounded real-time queue intentionally drops old frames.
pub struct MediaSessionReceiver {
    session_id: SessionId,
    local_device: DeviceId,
    lane: MediaLane,
    last_frame: Option<(u64, u64, u64)>,
}

impl MediaSessionReceiver {
    pub fn new(session_id: SessionId, local_device: DeviceId, lane: MediaLane) -> Self {
        Self {
            session_id,
            local_device,
            lane,
            last_frame: None,
        }
    }

    pub fn accept(&mut self, bytes: &[u8]) -> Result<MediaFrame, MediaSessionError> {
        if bytes.is_empty() || bytes.len() > MAX_ENCRYPTED_MESSAGE - 1 {
            return Err(MediaSessionError::InvalidFrame);
        }
        let frame: MediaFrame =
            serde_json::from_slice(bytes).map_err(|_| MediaSessionError::InvalidFrame)?;
        frame.validate()?;
        if frame.session_id != self.session_id || frame.target != self.local_device {
            return Err(MediaSessionError::WrongDestination);
        }
        if let Some((stream_id, sequence, timestamp)) = self.last_frame {
            if frame.stream_id != stream_id
                || frame.sequence <= sequence
                || frame.timestamp_100ns <= timestamp
            {
                return Err(MediaSessionError::OutOfOrder);
            }
        }
        self.last_frame = Some((frame.stream_id, frame.sequence, frame.timestamp_100ns));
        Ok(frame)
    }

    pub fn lane(&self) -> MediaLane {
        self.lane
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::SessionId;

    fn frame(session_id: SessionId, target: DeviceId, sequence: u64, timestamp: u64) -> MediaFrame {
        MediaFrame {
            session_id,
            target,
            stream_id: 17,
            sequence,
            timestamp_100ns: timestamp,
            width: 1280,
            height: 720,
            codec: VideoCodec::H264,
            key_frame: sequence == 0,
            payload: vec![1, 2, 3, 4],
        }
    }

    #[test]
    fn accepts_forward_progress_with_dropped_frame_gaps() {
        let session = SessionId::new();
        let target = DeviceId("target".into());
        let mut receiver = MediaSessionReceiver::new(session, target.clone(), MediaLane::Window);
        assert!(receiver
            .accept(&serde_json::to_vec(&frame(session, target.clone(), 0, 10)).unwrap())
            .is_ok());
        assert!(receiver
            .accept(&serde_json::to_vec(&frame(session, target, 3, 40)).unwrap())
            .is_ok());
    }

    #[test]
    fn rejects_wrong_destination_replay_and_invalid_dimensions() {
        let session = SessionId::new();
        let target = DeviceId("target".into());
        let mut receiver = MediaSessionReceiver::new(session, target.clone(), MediaLane::Display);
        let wrong = frame(session, DeviceId("other".into()), 0, 10);
        assert!(matches!(
            receiver.accept(&serde_json::to_vec(&wrong).unwrap()),
            Err(MediaSessionError::WrongDestination)
        ));
        let first = serde_json::to_vec(&frame(session, target.clone(), 1, 20)).unwrap();
        receiver.accept(&first).unwrap();
        assert!(matches!(
            receiver.accept(&first),
            Err(MediaSessionError::OutOfOrder)
        ));
        let mut invalid = frame(session, target, 2, 30);
        invalid.width = MAX_DIMENSION + 1;
        assert!(matches!(
            invalid.validate(),
            Err(MediaSessionError::InvalidFrame)
        ));
    }
}
