//! Typed encoded-video frames for the independent window and display lanes.
//!
//! This validates the media transport boundary only. Capture, encoding,
//! decoding, rendering, consent, and native display integration are separate.

use crate::protocol::{DeviceId, SessionId};
use crate::session_transport::{ServiceSender, SessionTransportError};
use crate::transport::MAX_ENCRYPTED_MESSAGE;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Binary payload budget reserves the encrypted record tag, a 2-byte header
/// length, and up to 512 bytes for bounded JSON metadata.
pub const MAX_MEDIA_PAYLOAD_BYTES: usize = MAX_ENCRYPTED_MESSAGE - 1 - 2 - 512;
const MAX_DIMENSION: u32 = 16_384;
const MAX_MEDIA_HEADER_BYTES: usize = 512;

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

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Serialize, Deserialize)]
struct MediaFrameHeader {
    session_id: SessionId,
    target: DeviceId,
    stream_id: u64,
    sequence: u64,
    timestamp_100ns: u64,
    width: u32,
    height: u32,
    codec: VideoCodec,
    key_frame: bool,
    payload_len: u32,
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

    fn encode(&self) -> Result<Vec<u8>, MediaSessionError> {
        self.validate()?;
        let header = MediaFrameHeader {
            session_id: self.session_id,
            target: self.target.clone(),
            stream_id: self.stream_id,
            sequence: self.sequence,
            timestamp_100ns: self.timestamp_100ns,
            width: self.width,
            height: self.height,
            codec: self.codec,
            key_frame: self.key_frame,
            payload_len: u32::try_from(self.payload.len())
                .map_err(|_| MediaSessionError::InvalidFrame)?,
        };
        let header = serde_json::to_vec(&header).map_err(|_| MediaSessionError::InvalidFrame)?;
        if header.is_empty() || header.len() > MAX_MEDIA_HEADER_BYTES {
            return Err(MediaSessionError::InvalidFrame);
        }
        let total_len = 2usize
            .checked_add(header.len())
            .and_then(|size| size.checked_add(self.payload.len()))
            .ok_or(MediaSessionError::InvalidFrame)?;
        if total_len > MAX_ENCRYPTED_MESSAGE - 1 {
            return Err(MediaSessionError::InvalidFrame);
        }
        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(&(header.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&self.payload);
        Ok(bytes)
    }

    fn decode(bytes: &[u8]) -> Result<Self, MediaSessionError> {
        if bytes.len() < 2 || bytes.len() > MAX_ENCRYPTED_MESSAGE - 1 {
            return Err(MediaSessionError::InvalidFrame);
        }
        let header_len = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        if header_len == 0 || header_len > MAX_MEDIA_HEADER_BYTES || bytes.len() < 2 + header_len {
            return Err(MediaSessionError::InvalidFrame);
        }
        let header: MediaFrameHeader = serde_json::from_slice(&bytes[2..2 + header_len])
            .map_err(|_| MediaSessionError::InvalidFrame)?;
        let payload = &bytes[2 + header_len..];
        if payload.len() != header.payload_len as usize {
            return Err(MediaSessionError::InvalidFrame);
        }
        let frame = Self {
            session_id: header.session_id,
            target: header.target,
            stream_id: header.stream_id,
            sequence: header.sequence,
            timestamp_100ns: header.timestamp_100ns,
            width: header.width,
            height: header.height,
            codec: header.codec,
            key_frame: header.key_frame,
            payload: payload.to_vec(),
        };
        frame.validate()?;
        Ok(frame)
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
    channel.try_send(frame.encode()?)?;
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
        let frame = MediaFrame::decode(bytes)?;
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
            .accept(&frame(session, target.clone(), 0, 10).encode().unwrap())
            .is_ok());
        assert!(receiver
            .accept(&frame(session, target, 3, 40).encode().unwrap())
            .is_ok());
    }

    #[test]
    fn rejects_wrong_destination_replay_and_invalid_dimensions() {
        let session = SessionId::new();
        let target = DeviceId("target".into());
        let mut receiver = MediaSessionReceiver::new(session, target.clone(), MediaLane::Display);
        let wrong = frame(session, DeviceId("other".into()), 0, 10);
        assert!(matches!(
            receiver.accept(&wrong.encode().unwrap()),
            Err(MediaSessionError::WrongDestination)
        ));
        let first = frame(session, target.clone(), 1, 20).encode().unwrap();
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

    #[test]
    fn binary_payload_round_trips_without_json_array_expansion() {
        let session = SessionId::new();
        let target = DeviceId("target".into());
        let mut frame = frame(session, target, 0, 10);
        frame.payload = vec![0xA5; 512 * 1024];
        let bytes = frame.encode().unwrap();
        assert!(bytes.len() < frame.payload.len() + MAX_MEDIA_HEADER_BYTES + 2);
        assert_eq!(MediaFrame::decode(&bytes).unwrap(), frame);

        let mut truncated = bytes;
        truncated.pop();
        assert!(matches!(
            MediaFrame::decode(&truncated),
            Err(MediaSessionError::InvalidFrame)
        ));
    }
}
