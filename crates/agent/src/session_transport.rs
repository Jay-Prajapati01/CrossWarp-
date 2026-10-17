//! Multiplexes independent control, window-video, display-video, and file
//! services over one authenticated encrypted connection.
//!
//! Each encrypted record carries one service byte followed by that service's
//! payload. The writer prioritizes input and interactive window frames, while
//! bounded queues keep media and bulk work from allocating without limit.

use crate::transport::{EncryptedReceiver, EncryptedSender, TransportError, MAX_ENCRYPTED_MESSAGE};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, Mutex};

const INPUT_QUEUE: usize = 256;
const VIDEO_QUEUE: usize = 2;
const FILE_QUEUE: usize = 8;
const MAX_INPUT_PAYLOAD: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
enum ServiceKind {
    Input = 1,
    WindowVideo = 2,
    DisplayVideo = 3,
    Files = 4,
}

impl ServiceKind {
    fn from_tag(tag: u8) -> Option<Self> {
        Some(match tag {
            1 => Self::Input,
            2 => Self::WindowVideo,
            3 => Self::DisplayVideo,
            4 => Self::Files,
            _ => return None,
        })
    }

    fn capacity(self) -> usize {
        match self {
            Self::Input => INPUT_QUEUE,
            Self::WindowVideo | Self::DisplayVideo => VIDEO_QUEUE,
            Self::Files => FILE_QUEUE,
        }
    }

    fn max_payload_bytes(self) -> usize {
        match self {
            Self::Input => MAX_INPUT_PAYLOAD,
            Self::WindowVideo | Self::DisplayVideo | Self::Files => MAX_ENCRYPTED_MESSAGE - 1,
        }
    }
}

#[derive(Debug, Error)]
pub enum SessionTransportError {
    #[error("encrypted session transport failed: {0}")]
    Transport(#[from] TransportError),
    #[error("session frame is empty, oversized, or has an unknown service tag")]
    InvalidFrame,
    #[error("the session service queue is closed")]
    Closed,
    #[error("the bounded session service queue is full")]
    QueueFull,
    #[error("a session transport worker stopped unexpectedly")]
    WorkerStopped,
}

struct AbortWorkersOnDrop(Vec<tokio::task::AbortHandle>);

impl Drop for AbortWorkersOnDrop {
    fn drop(&mut self) {
        for worker in &self.0 {
            worker.abort();
        }
    }
}

#[derive(Clone)]
pub struct ServiceSender {
    kind: ServiceKind,
    tx: mpsc::Sender<Vec<u8>>,
}

impl ServiceSender {
    pub(crate) fn is_input_lane(&self) -> bool {
        self.kind == ServiceKind::Input
    }

    pub(crate) fn is_window_video_lane(&self) -> bool {
        self.kind == ServiceKind::WindowVideo
    }

    pub(crate) fn is_display_video_lane(&self) -> bool {
        self.kind == ServiceKind::DisplayVideo
    }

    /// Losslessly enqueue a payload, applying bounded backpressure.
    pub async fn send(&self, payload: Vec<u8>) -> Result<(), SessionTransportError> {
        validate_payload(self.kind, &payload)?;
        self.tx
            .send(payload)
            .await
            .map_err(|_| SessionTransportError::Closed)
    }

    /// Nonblocking enqueue for real-time producers. Callers may drop stale
    /// frames when this returns `QueueFull`; input and file traffic should use
    /// `send` so key transitions and file operations are not silently lost.
    pub fn try_send(&self, payload: Vec<u8>) -> Result<(), SessionTransportError> {
        validate_payload(self.kind, &payload)?;
        self.tx.try_send(payload).map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => SessionTransportError::QueueFull,
            mpsc::error::TrySendError::Closed(_) => SessionTransportError::Closed,
        })
    }

    pub fn queue_capacity(&self) -> usize {
        self.kind.capacity()
    }
}

#[derive(Clone)]
pub struct ServiceReceiver {
    kind: ServiceKind,
    rx: Arc<Mutex<mpsc::Receiver<Vec<u8>>>>,
}

impl ServiceReceiver {
    pub async fn recv(&self) -> Option<Vec<u8>> {
        let mut rx = self.rx.lock().await;
        let first = rx.recv().await?;
        if matches!(
            self.kind,
            ServiceKind::WindowVideo | ServiceKind::DisplayVideo
        ) {
            // Drain buffered stale frames and return the newest frame available.
            let mut latest = first;
            while let Ok(next) = rx.try_recv() {
                latest = next;
            }
            Some(latest)
        } else {
            Some(first)
        }
    }
}

#[derive(Clone)]
pub struct ServicePipe {
    pub sender: ServiceSender,
    pub receiver: ServiceReceiver,
}

#[derive(Clone)]
pub struct SessionChannels {
    pub input: ServicePipe,
    pub window_video: ServicePipe,
    pub display_video: ServicePipe,
    pub files: ServicePipe,
}

/// Starts a single reader and prioritized writer for all independent services.
/// Dropping the returned channel endpoints terminates the workers and closes
/// the authenticated session; dropping one service does not close the others.
pub fn spawn_session_transport<S, R>(
    sender: EncryptedSender<S>,
    receiver: EncryptedReceiver<R>,
) -> (
    SessionChannels,
    tokio::task::JoinHandle<Result<(), SessionTransportError>>,
)
where
    S: AsyncWrite + Unpin + Send + 'static,
    R: AsyncRead + Unpin + Send + 'static,
{
    let (channels, outbound, inbound) = make_channels();
    let mut writer = tokio::spawn(run_writer(sender, outbound));
    let mut reader = tokio::spawn(run_reader(receiver, inbound));
    let abort_workers = AbortWorkersOnDrop(vec![writer.abort_handle(), reader.abort_handle()]);
    let task = tokio::spawn(async move {
        let _abort_workers = abort_workers;
        tokio::select! {
            result = &mut writer => {
                result.map_err(|_| SessionTransportError::WorkerStopped)?
            }
            result = &mut reader => {
                result.map_err(|_| SessionTransportError::WorkerStopped)?
            }
        }
    });
    (channels, task)
}

type OutboundReceivers = (
    mpsc::Receiver<Vec<u8>>,
    mpsc::Receiver<Vec<u8>>,
    mpsc::Receiver<Vec<u8>>,
    mpsc::Receiver<Vec<u8>>,
);
type InboundSenders = (
    mpsc::Sender<Vec<u8>>,
    mpsc::Sender<Vec<u8>>,
    mpsc::Sender<Vec<u8>>,
    mpsc::Sender<Vec<u8>>,
);

fn channel(kind: ServiceKind) -> (ServicePipe, mpsc::Receiver<Vec<u8>>, mpsc::Sender<Vec<u8>>) {
    let (out_tx, out_rx) = mpsc::channel(kind.capacity());
    let (in_tx, in_rx) = mpsc::channel(kind.capacity());
    (
        ServicePipe {
            sender: ServiceSender { kind, tx: out_tx },
            receiver: ServiceReceiver {
                kind,
                rx: Arc::new(Mutex::new(in_rx)),
            },
        },
        out_rx,
        in_tx,
    )
}

fn make_channels() -> (SessionChannels, OutboundReceivers, InboundSenders) {
    let (input, input_out, input_in) = channel(ServiceKind::Input);
    let (window, window_out, window_in) = channel(ServiceKind::WindowVideo);
    let (display, display_out, display_in) = channel(ServiceKind::DisplayVideo);
    let (files, files_out, files_in) = channel(ServiceKind::Files);
    (
        SessionChannels {
            input,
            window_video: window,
            display_video: display,
            files,
        },
        (input_out, window_out, display_out, files_out),
        (input_in, window_in, display_in, files_in),
    )
}

async fn run_writer<W: AsyncWrite + Unpin>(
    mut sender: EncryptedSender<W>,
    (mut input, mut window, mut display, mut files): OutboundReceivers,
) -> Result<(), SessionTransportError> {
    let (mut input_open, mut window_open, mut display_open, mut files_open) =
        (true, true, true, true);
    let mut high_priority_since_file = 0u8;
    let mut input_burst = 0u8;
    let mut window_burst = 0u8;
    loop {
        if !(input_open || window_open || display_open || files_open) {
            return Ok(());
        }
        if files_open && high_priority_since_file >= 8 {
            match files.try_recv() {
                Ok(payload) => {
                    send_or_close(
                        &mut sender,
                        ServiceKind::Files,
                        Some(payload),
                        &mut files_open,
                    )
                    .await?;
                    high_priority_since_file = 0;
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => files_open = false,
                Err(mpsc::error::TryRecvError::Empty) => high_priority_since_file = 0,
            }
        }
        if input_open && input_burst >= 4 {
            match window.try_recv() {
                Ok(payload) => {
                    send_or_close(
                        &mut sender,
                        ServiceKind::WindowVideo,
                        Some(payload),
                        &mut window_open,
                    )
                    .await?;
                    input_burst = 0;
                    window_burst = window_burst.saturating_add(1);
                    high_priority_since_file = high_priority_since_file.saturating_add(1);
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => window_open = false,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            match display.try_recv() {
                Ok(payload) => {
                    send_or_close(
                        &mut sender,
                        ServiceKind::DisplayVideo,
                        Some(payload),
                        &mut display_open,
                    )
                    .await?;
                    input_burst = 0;
                    window_burst = 0;
                    high_priority_since_file = high_priority_since_file.saturating_add(1);
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => display_open = false,
                Err(mpsc::error::TryRecvError::Empty) => {}
            }
            input_burst = 0;
        }
        if window_open && window_burst >= 2 {
            match display.try_recv() {
                Ok(payload) => {
                    send_or_close(
                        &mut sender,
                        ServiceKind::DisplayVideo,
                        Some(payload),
                        &mut display_open,
                    )
                    .await?;
                    window_burst = 0;
                    input_burst = 0;
                    high_priority_since_file = high_priority_since_file.saturating_add(1);
                    continue;
                }
                Err(mpsc::error::TryRecvError::Disconnected) => display_open = false,
                Err(mpsc::error::TryRecvError::Empty) => window_burst = 0,
            }
        }
        tokio::select! {
            biased;
            item = input.recv(), if input_open => {
                send_or_close(&mut sender, ServiceKind::Input, item, &mut input_open).await?;
                high_priority_since_file = high_priority_since_file.saturating_add(1);
                input_burst = input_burst.saturating_add(1);
                window_burst = 0;
            },
            item = window.recv(), if window_open => {
                send_or_close(&mut sender, ServiceKind::WindowVideo, item, &mut window_open).await?;
                high_priority_since_file = high_priority_since_file.saturating_add(1);
                input_burst = 0;
                window_burst = window_burst.saturating_add(1);
            },
            item = display.recv(), if display_open => {
                send_or_close(&mut sender, ServiceKind::DisplayVideo, item, &mut display_open).await?;
                high_priority_since_file = high_priority_since_file.saturating_add(1);
                input_burst = 0;
                window_burst = 0;
            },
            item = files.recv(), if files_open => {
                send_or_close(&mut sender, ServiceKind::Files, item, &mut files_open).await?;
                high_priority_since_file = 0;
                input_burst = 0;
                window_burst = 0;
            },
        }
    }
}

async fn send_or_close<W: AsyncWrite + Unpin>(
    sender: &mut EncryptedSender<W>,
    kind: ServiceKind,
    payload: Option<Vec<u8>>,
    open: &mut bool,
) -> Result<(), SessionTransportError> {
    let Some(payload) = payload else {
        *open = false;
        return Ok(());
    };
    let mut frame = Vec::with_capacity(payload.len() + 1);
    frame.push(kind as u8);
    frame.extend_from_slice(&payload);
    sender.send(&frame).await?;
    Ok(())
}

async fn run_reader<R: AsyncRead + Unpin>(
    mut receiver: EncryptedReceiver<R>,
    (input, window, display, files): InboundSenders,
) -> Result<(), SessionTransportError> {
    loop {
        let frame = receiver.receive().await?;
        if frame.len() < 2 || frame.len() > MAX_ENCRYPTED_MESSAGE {
            return Err(SessionTransportError::InvalidFrame);
        }
        let kind = ServiceKind::from_tag(frame[0]).ok_or(SessionTransportError::InvalidFrame)?;
        let payload = frame[1..].to_vec();
        let tx = match kind {
            ServiceKind::Input => &input,
            ServiceKind::WindowVideo => &window,
            ServiceKind::DisplayVideo => &display,
            ServiceKind::Files => &files,
        };
        if matches!(kind, ServiceKind::WindowVideo | ServiceKind::DisplayVideo) {
            // Realtime producers and consumers can tolerate a dropped newest
            // frame when the receiver is behind; key/file records are lossless.
            let _ = tx.try_send(payload);
        } else {
            tx.send(payload)
                .await
                .map_err(|_| SessionTransportError::Closed)?;
        }
    }
}

fn validate_payload(kind: ServiceKind, payload: &[u8]) -> Result<(), SessionTransportError> {
    if payload.is_empty() || payload.len() > kind.max_payload_bytes() {
        Err(SessionTransportError::InvalidFrame)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::{pair_over_stream, HandshakeRole};
    use crate::trust::DeviceIdentity;
    use std::time::Duration;
    use tokio::io::duplex;
    use tokio::time::timeout;

    #[tokio::test]
    async fn all_service_channels_share_one_encrypted_session_without_cross_delivery() {
        let (initiator_identity, responder_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (initiator_stream, responder_stream) = duplex(128 * 1024);
        let (initiator, responder) = tokio::join!(
            pair_over_stream(
                initiator_stream,
                &initiator_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            ),
            pair_over_stream(
                responder_stream,
                &responder_identity,
                HandshakeRole::Responder,
                |_| async { true }
            ),
        );
        let (left_sender, left_receiver, _) = initiator.unwrap().into_encrypted();
        let (right_sender, right_receiver, _) = responder.unwrap().into_encrypted();
        let (left, _left_task) = spawn_session_transport(left_sender, left_receiver);
        let (right, _right_task) = spawn_session_transport(right_sender, right_receiver);

        let sends = tokio::join!(
            left.input.sender.send(b"input-event".to_vec()),
            left.window_video.sender.send(b"window-frame".to_vec()),
            left.display_video.sender.send(b"display-frame".to_vec()),
            left.files.sender.send(b"file-rpc".to_vec()),
        );
        assert!(sends.0.is_ok() && sends.1.is_ok() && sends.2.is_ok() && sends.3.is_ok());

        let right_input = right.input.receiver;
        let right_window = right.window_video.receiver;
        let right_display = right.display_video.receiver;
        let right_files = right.files.receiver;
        let received = tokio::join!(
            timeout(Duration::from_secs(2), right_input.recv()),
            timeout(Duration::from_secs(2), right_window.recv()),
            timeout(Duration::from_secs(2), right_display.recv()),
            timeout(Duration::from_secs(2), right_files.recv()),
        );
        assert_eq!(received.0.unwrap().as_deref(), Some(&b"input-event"[..]));
        assert_eq!(received.1.unwrap().as_deref(), Some(&b"window-frame"[..]));
        assert_eq!(received.2.unwrap().as_deref(), Some(&b"display-frame"[..]));
        assert_eq!(received.3.unwrap().as_deref(), Some(&b"file-rpc"[..]));
    }

    #[tokio::test]
    async fn bulk_file_service_makes_progress_under_continuous_input_load() {
        let (initiator_identity, responder_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (initiator_stream, responder_stream) = duplex(128 * 1024);
        let (initiator, responder) = tokio::join!(
            pair_over_stream(
                initiator_stream,
                &initiator_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            ),
            pair_over_stream(
                responder_stream,
                &responder_identity,
                HandshakeRole::Responder,
                |_| async { true }
            ),
        );
        let (left_sender, left_receiver, _) = initiator.unwrap().into_encrypted();
        let (right_sender, right_receiver, _) = responder.unwrap().into_encrypted();
        let (left, _left_task) = spawn_session_transport(left_sender, left_receiver);
        let (right, _right_task) = spawn_session_transport(right_sender, right_receiver);
        for _ in 0..16 {
            left.input
                .sender
                .try_send(b"key-transition".to_vec())
                .unwrap();
        }
        left.files.sender.try_send(b"file-chunk".to_vec()).unwrap();

        let input = right.input.receiver;
        let files = right.files.receiver;
        for _ in 0..8 {
            assert_eq!(
                timeout(Duration::from_secs(2), input.recv())
                    .await
                    .unwrap()
                    .as_deref(),
                Some(&b"key-transition"[..])
            );
        }
        assert_eq!(
            timeout(Duration::from_secs(2), files.recv())
                .await
                .unwrap()
                .as_deref(),
            Some(&b"file-chunk"[..])
        );
        for _ in 0..8 {
            assert_eq!(
                timeout(Duration::from_secs(2), input.recv())
                    .await
                    .unwrap()
                    .as_deref(),
                Some(&b"key-transition"[..])
            );
        }
    }

    #[tokio::test]
    async fn display_frames_make_progress_during_continuous_input_and_window_load() {
        let (initiator_identity, responder_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (initiator_stream, responder_stream) = duplex(128 * 1024);
        let (initiator, responder) = tokio::join!(
            pair_over_stream(
                initiator_stream,
                &initiator_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            ),
            pair_over_stream(
                responder_stream,
                &responder_identity,
                HandshakeRole::Responder,
                |_| async { true }
            ),
        );
        let (left_sender, left_receiver, _) = initiator.unwrap().into_encrypted();
        let (right_sender, right_receiver, _) = responder.unwrap().into_encrypted();
        let (left, _left_task) = spawn_session_transport(left_sender, left_receiver);
        let (right, _right_task) = spawn_session_transport(right_sender, right_receiver);
        let drain_input =
            tokio::spawn(async move { while right.input.receiver.recv().await.is_some() {} });
        let drain_window =
            tokio::spawn(
                async move { while right.window_video.receiver.recv().await.is_some() {} },
            );
        let input_sender = left.input.sender.clone();
        let window_sender = left.window_video.sender.clone();
        let input_producer = tokio::spawn(async move {
            loop {
                if matches!(
                    input_sender.try_send(b"input".to_vec()),
                    Err(SessionTransportError::Closed)
                ) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        });
        let window_producer = tokio::spawn(async move {
            loop {
                if matches!(
                    window_sender.try_send(b"window".to_vec()),
                    Err(SessionTransportError::Closed)
                ) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        });
        left.display_video
            .sender
            .try_send(b"display".to_vec())
            .unwrap();
        let display = timeout(Duration::from_secs(2), right.display_video.receiver.recv())
            .await
            .expect("display lane should not starve behind input and window traffic");
        assert_eq!(display.as_deref(), Some(&b"display"[..]));
        input_producer.abort();
        window_producer.abort();
        drain_input.abort();
        drain_window.abort();
    }

    #[tokio::test]
    async fn aborting_one_session_closes_its_transport_workers_and_peer() {
        let (initiator_identity, responder_identity) =
            (DeviceIdentity::generate(), DeviceIdentity::generate());
        let (initiator_stream, responder_stream) = duplex(128 * 1024);
        let (initiator, responder) = tokio::join!(
            pair_over_stream(
                initiator_stream,
                &initiator_identity,
                HandshakeRole::Initiator,
                |_| async { true }
            ),
            pair_over_stream(
                responder_stream,
                &responder_identity,
                HandshakeRole::Responder,
                |_| async { true }
            ),
        );
        let (left_sender, left_receiver, _) = initiator.unwrap().into_encrypted();
        let (right_sender, right_receiver, _) = responder.unwrap().into_encrypted();
        let (_left_channels, left_task) = spawn_session_transport(left_sender, left_receiver);
        let (_right_channels, right_task) = spawn_session_transport(right_sender, right_receiver);
        left_task.abort();

        let peer_result = timeout(Duration::from_secs(2), right_task)
            .await
            .expect("peer transport should observe the closed session")
            .expect("peer session task should finish normally");
        assert!(matches!(
            peer_result,
            Err(SessionTransportError::Transport(_))
        ));
    }

    #[test]
    fn service_queues_are_bounded_and_payloads_cannot_exceed_frame_limit() {
        let (channels, _, _) = make_channels();
        assert_eq!(channels.input.sender.queue_capacity(), INPUT_QUEUE);
        assert_eq!(channels.window_video.sender.queue_capacity(), VIDEO_QUEUE);
        assert_eq!(channels.display_video.sender.queue_capacity(), VIDEO_QUEUE);
        assert_eq!(channels.files.sender.queue_capacity(), FILE_QUEUE);
        assert!(matches!(
            validate_payload(ServiceKind::Files, &vec![0; MAX_ENCRYPTED_MESSAGE]),
            Err(SessionTransportError::InvalidFrame)
        ));
        assert!(matches!(
            validate_payload(ServiceKind::Input, &vec![0; MAX_INPUT_PAYLOAD + 1]),
            Err(SessionTransportError::InvalidFrame)
        ));
    }
}
