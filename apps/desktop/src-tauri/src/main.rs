#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, oneshot};
use unified_workspace_agent::protocol::{Capability, CapabilityState, DeviceId, SessionId};
use unified_workspace_agent::windows_input::{InputCapture, InputKind};
use unified_workspace_agent::windows_capture::{
    encode_preview_jpeg, enumerate_windows, CapturedFrame, WindowCapture,
};
use unified_workspace_agent::{
    accept_pair_or_trusted_over_stream, connect_trusted_over_stream, pair_over_stream,
    send_media_frame, spawn_file_rpc, spawn_session_transport, DeviceIdentity,
    FileRequest, FileResponse, FileRpcClient, HandshakeRole, HostPlatform, IdentityStore,
    LanEndpoint, MediaFrame, MediaLane, MediaSessionError, MediaSessionReceiver,
    PairedTransport, PairingApproval,
    PeerBrowser, SessionChallenge, SessionChannels, SessionCoordinator, SharedEntry,
    SessionTransportError, SharedWorkspace, TrustedPeerStore, VideoCodec, MAX_RPC_READ_CHUNK,
    MAX_RPC_UPLOAD_CHUNK,
};

#[derive(Clone, Default)]
struct WorkspaceState(Arc<Mutex<Option<Arc<SharedWorkspace>>>>);

#[derive(Clone, Default)]
struct NetworkState(Arc<Mutex<NetworkInner>>);

#[derive(Default)]
struct NetworkInner {
    identity: Option<Arc<DeviceIdentity>>,
    endpoint: Option<Arc<LanEndpoint>>,
    browser: Option<PeerBrowser>,
}

#[derive(Clone, Default)]
struct PairingState(Arc<PairingInner>);

#[derive(Clone, Default)]
struct InputDiagnosticState(Arc<Mutex<Option<InputDiagnosticHandle>>>);

struct InputDiagnosticHandle {
    stop: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
    counters: Arc<InputDiagnosticCounters>,
}

#[derive(Default)]
struct InputDiagnosticCounters {
    total: AtomicU64,
    pointer_move: AtomicU64,
    pointer_button: AtomicU64,
    pointer_wheel: AtomicU64,
    key_transition: AtomicU64,
    error: Mutex<Option<String>>,
}

#[derive(Serialize)]
struct InputDiagnosticReport {
    running: bool,
    total: u64,
    pointer_move: u64,
    pointer_button: u64,
    pointer_wheel: u64,
    key_transition: u64,
    error: Option<String>,
}

#[derive(Default)]
struct PairingInner {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    pending_window_shares: Mutex<HashMap<String, (String, oneshot::Sender<bool>)>>,
    trusted: Mutex<Vec<String>>,
    active: Mutex<HashSet<String>>,
    clients: Mutex<HashMap<String, FileRpcClient>>,
    services: Mutex<HashMap<String, SessionChannels>>,
    session_tasks: Mutex<HashMap<String, tokio::task::AbortHandle>>,
    window_frames: Mutex<HashMap<String, RemoteWindowFrame>>,
    media_contexts: Mutex<HashMap<String, (SessionId, DeviceId)>>,
    window_share_stops: Mutex<HashMap<String, Arc<AtomicBool>>>,
    window_share_targets: Mutex<HashMap<String, (u64, u64)>>,
    window_control_streams: Mutex<HashMap<String, (u64, u64)>>,
    window_portal_held: Mutex<HashMap<String, Vec<WindowPortalInput>>>,
    window_control_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

struct AbortChildrenOnDrop(Vec<tokio::task::AbortHandle>);

impl Drop for AbortChildrenOnDrop {
    fn drop(&mut self) {
        for child in &self.0 {
            child.abort();
        }
    }
}

#[derive(Clone, Serialize)]
struct PairingChallengeView {
    request_id: String,
    local_device: String,
    peer_device: String,
    verification_code: String,
}

#[derive(Deserialize)]
struct PairAddress {
    device_id: String,
    address: String,
}

#[derive(Serialize)]
struct PairingResult {
    peer_device_id: String,
    session_id: String,
    trusted: bool,
    transport_active: bool,
    detail: String,
}

#[derive(Clone, Serialize)]
struct RemoteTransferProgress {
    peer_device_id: String,
    direction: &'static str,
    bytes_sent: u64,
    total_bytes: u64,
    complete: bool,
}

#[derive(Clone, Serialize)]
struct WindowShareRequestView {
    request_id: String,
    peer_device_id: String,
    stream_id: u64,
    width: u32,
    height: u32,
}

#[derive(Clone, Serialize)]
struct RemoteWindowFrame {
    peer_device_id: String,
    stream_id: u64,
    sequence: u64,
    timestamp_100ns: u64,
    received_at_unix_ms: u64,
    width: u32,
    height: u32,
    jpeg_base64: String,
}

#[derive(Clone, Serialize)]
struct CapturableWindowView {
    hwnd: u64,
    title: String,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum WindowControlAction {
    Stop,
    Input { source_stream_id: u64, event: WindowPortalInput },
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WindowPortalInput {
    PointerMove { x: u16, y: u16 },
    PointerButton { x: u16, y: u16, button: u8, down: bool },
    PointerWheel { x: u16, y: u16, delta: i32, horizontal: bool },
    Key { virtual_key: u16, scan_code: u16, extended: bool, down: bool },
}

impl WindowPortalInput {
    fn transition(&self) -> Option<(String, bool)> {
        match self {
            Self::PointerButton { button, down, .. } => Some((format!("button:{button}"), *down)),
            Self::Key { virtual_key, scan_code, down, .. } => Some((format!("key:{virtual_key}:{scan_code}"), *down)),
            Self::PointerMove { .. } | Self::PointerWheel { .. } => None,
        }
    }

    fn released(&self) -> Option<Self> {
        match self {
            Self::PointerButton { x, y, button, .. } => Some(Self::PointerButton { x: *x, y: *y, button: *button, down: false }),
            Self::Key { virtual_key, scan_code, extended, .. } => Some(Self::Key { virtual_key: *virtual_key, scan_code: *scan_code, extended: *extended, down: false }),
            Self::PointerMove { .. } | Self::PointerWheel { .. } => None,
        }
    }
}

static NEXT_PAIRING_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_DOWNLOAD_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_WINDOW_SHARE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_VIDEO_STREAM_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_WINDOW_CONTROL_STREAM_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX / 2);

#[derive(Serialize)]
struct NetworkStatus {
    device_id: String,
    listen_address: String,
    discovery: String,
}

#[derive(Serialize)]
struct PeerView {
    device_id: String,
    addresses: Vec<String>,
}

#[derive(Serialize)]
struct CapabilityReport {
    capability: &'static str,
    state: &'static str,
    detail: String,
}

#[derive(Serialize)]
struct LocalStatus {
    device_name: String,
    connection: &'static str,
    implementation: &'static str,
    capabilities: Vec<CapabilityReport>,
}

fn input_diagnostic_report(
    running: bool,
    counters: &InputDiagnosticCounters,
) -> InputDiagnosticReport {
    InputDiagnosticReport {
        running,
        total: counters.total.load(Ordering::Relaxed),
        pointer_move: counters.pointer_move.load(Ordering::Relaxed),
        pointer_button: counters.pointer_button.load(Ordering::Relaxed),
        pointer_wheel: counters.pointer_wheel.load(Ordering::Relaxed),
        key_transition: counters.key_transition.load(Ordering::Relaxed),
        error: counters.error.lock().ok().and_then(|error| error.clone()),
    }
}

#[tauri::command]
fn start_input_capture_diagnostic(
    state: tauri::State<'_, InputDiagnosticState>,
) -> Result<InputDiagnosticReport, String> {
    let mut active = state
        .0
        .lock()
        .map_err(|_| "input diagnostic state is unavailable")?;
    if active
        .as_ref()
        .is_some_and(|handle| !handle.task.is_finished())
    {
        return Err("the local input capture diagnostic is already running".into());
    }

    // This command is reachable only after the user confirms the local-only
    // capture disclosure in the desktop UI. Events are summarized by kind;
    // key codes and pointer data never enter UI state or the network.
    let mut capture = InputCapture::start_after_user_consent()
        .map_err(|error| format!("local input capture could not start: {error}"))?;
    let counters = Arc::new(InputDiagnosticCounters::default());
    let task_counters = counters.clone();
    let (stop, mut stop_requested) = oneshot::channel();
    let task = tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut stop_requested => break,
                event = capture.recv() => match event {
                    Ok(Some(event)) => {
                        let counter = match event.kind {
                            InputKind::PointerMove => &task_counters.pointer_move,
                            InputKind::PointerButton => &task_counters.pointer_button,
                            InputKind::PointerWheel => &task_counters.pointer_wheel,
                            InputKind::Key => &task_counters.key_transition,
                        };
                        counter.fetch_add(1, Ordering::Relaxed);
                        task_counters.total.fetch_add(1, Ordering::Relaxed);
                    }
                    Ok(None) => break,
                    Err(error) => {
                        if let Ok(mut slot) = task_counters.error.lock() {
                            *slot = Some(error.to_string());
                        }
                        break;
                    }
                }
            }
        }
    });
    let report = input_diagnostic_report(true, &counters);
    *active = Some(InputDiagnosticHandle {
        stop,
        task,
        counters,
    });
    Ok(report)
}

#[tauri::command]
async fn stop_input_capture_diagnostic(
    state: tauri::State<'_, InputDiagnosticState>,
) -> Result<InputDiagnosticReport, String> {
    let handle = state
        .0
        .lock()
        .map_err(|_| "input diagnostic state is unavailable")?
        .take();
    let Some(handle) = handle else {
        return Ok(InputDiagnosticReport {
            running: false,
            total: 0,
            pointer_move: 0,
            pointer_button: 0,
            pointer_wheel: 0,
            key_transition: 0,
            error: None,
        });
    };
    let _ = handle.stop.send(());
    handle
        .task
        .await
        .map_err(|error| format!("input diagnostic worker stopped unexpectedly: {error}"))?;
    Ok(input_diagnostic_report(false, &handle.counters))
}

#[tauri::command]
fn get_input_capture_diagnostic(
    state: tauri::State<'_, InputDiagnosticState>,
) -> Result<InputDiagnosticReport, String> {
    let active = state
        .0
        .lock()
        .map_err(|_| "input diagnostic state is unavailable")?;
    match active.as_ref() {
        Some(handle) => Ok(input_diagnostic_report(
            !handle.task.is_finished(),
            &handle.counters,
        )),
        None => Ok(InputDiagnosticReport {
            running: false,
            total: 0,
            pointer_move: 0,
            pointer_button: 0,
            pointer_wheel: 0,
            key_transition: 0,
            error: None,
        }),
    }
}

#[tauri::command]
fn get_local_status(
    state: tauri::State<'_, WorkspaceState>,
    pairing: tauri::State<'_, PairingState>,
) -> LocalStatus {
    let has_local_workspace = state
        .0
        .lock()
        .map(|workspace| workspace.is_some())
        .unwrap_or(false);
    let mut session = SessionCoordinator::new(
        SessionId::new(),
        DeviceId(std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Unknown device".into())),
        DeviceId("unpaired".into()),
    );
    let reports = [
        (
            Capability::Input,
            "input",
            "Input routing is not connected.".to_owned(),
        ),
        (
            Capability::Files,
            "shared_files",
            if has_local_workspace {
                "A local folder is selected; connected sharing requires a paired peer transport."
                    .to_owned()
            } else {
                "No local Shared Workspace folder is selected; connected sharing is unavailable."
                    .to_owned()
            },
        ),
        (
            Capability::ExtendedDisplay,
            "extended_display",
            "The IddCx driver and stream are not available.".to_owned(),
        ),
        (
            Capability::WindowPortal,
            "live_window_handoff",
            "No capture or remote window transport is active.".to_owned(),
        ),
    ];
    let capabilities = reports
        .into_iter()
        .map(|(capability, name, detail)| {
            let _ = session.set_capability(
                capability,
                CapabilityState::Blocked {
                    reason: detail.clone(),
                },
            );
            CapabilityReport {
                capability: name,
                state: "unavailable",
                detail,
            }
        })
        .collect();
    LocalStatus {
        device_name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Unknown device".into()),
        connection: if pairing.0.active.lock().map(|peers| !peers.is_empty()).unwrap_or(false) { "connected" } else { "disconnected" },
        implementation: "authenticated pairing transport is available; feature readiness is reported per service",
        capabilities,
    }
}

#[tauri::command]
fn get_platform_report() -> HostPlatform {
    HostPlatform::detect()
}

fn load_or_create_identity() -> Result<DeviceIdentity, String> {
    let store = IdentityStore::for_current_user().map_err(|error| error.to_string())?;
    match store.load() {
        Ok(identity) => Ok(identity),
        Err(_) if !store.path().exists() => {
            let identity = DeviceIdentity::generate();
            store.save(&identity).map_err(|error| error.to_string())?;
            Ok(identity)
        }
        Err(error) => Err(format!(
            "saved device identity could not be opened: {error}"
        )),
    }
}

#[tauri::command]
async fn start_local_network(
    app: tauri::AppHandle,
    state: tauri::State<'_, NetworkState>,
    pairing: tauri::State<'_, PairingState>,
    workspace: tauri::State<'_, WorkspaceState>,
) -> Result<NetworkStatus, String> {
    let identity = {
        let mut inner = state.0.lock().map_err(|_| "network state is unavailable")?;
        if inner.identity.is_none() {
            inner.identity = Some(Arc::new(load_or_create_identity()?));
        }
        if let (Some(identity), Some(endpoint)) = (&inner.identity, &inner.endpoint) {
            return Ok(NetworkStatus {
                device_id: identity.device_id().0,
                listen_address: endpoint.local_addr().map_err(|error| error.to_string())?.to_string(),
                discovery: "TCP listener bound; mDNS publish and browse requested (peer presence unverified)".into(),
            });
        }
        inner
            .identity
            .as_ref()
            .expect("identity was initialized")
            .clone()
    };

    // Bind before advertising so the published endpoint always has a live listener.
    let endpoint = Arc::new(
        LanEndpoint::bind(
            &identity,
            &std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows device".into()),
        )
        .await
        .map_err(|error| error.to_string())?,
    );
    let browser = PeerBrowser::start(identity.device_id()).map_err(|error| error.to_string())?;
    let listen_address = endpoint
        .local_addr()
        .map_err(|error| error.to_string())?
        .to_string();
    let mut inner = state.0.lock().map_err(|_| "network state is unavailable")?;
    inner.endpoint = Some(endpoint.clone());
    inner.browser = Some(browser);
    inner.identity = Some(identity.clone());
    let listener = endpoint;
    let local_device_id = identity.device_id().0;
    let accept_identity = identity.clone();
    let pairing_state = pairing.0.clone();
    let workspace_state = workspace.0.clone();
    let app_handle = app.clone();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _remote)) = listener.accept().await else {
                break;
            };
            let peer_identity = accept_identity.clone();
            let pairing_state = pairing_state.clone();
            let workspace_state = workspace_state.clone();
            let app_handle = app_handle.clone();
            tokio::spawn(async move {
                let _ = run_accept(
                    stream,
                    peer_identity,
                    app_handle,
                    pairing_state,
                    workspace_state,
                )
                .await;
            });
        }
    });
    Ok(NetworkStatus {
        device_id: local_device_id,
        listen_address,
        discovery:
            "TCP listener bound; mDNS publish and browse requested (peer presence unverified)"
                .into(),
    })
}

#[tauri::command]
async fn pair_with_peer(
    app: tauri::AppHandle,
    peer: PairAddress,
    network: tauri::State<'_, NetworkState>,
    pairing: tauri::State<'_, PairingState>,
    workspace: tauri::State<'_, WorkspaceState>,
) -> Result<PairingResult, String> {
    let address: std::net::SocketAddr = peer
        .address
        .parse()
        .map_err(|_| "peer address is invalid")?;
    let identity = {
        let inner = network
            .0
            .lock()
            .map_err(|_| "network state is unavailable")?;
        inner
            .identity
            .as_ref()
            .ok_or("start local discovery first")?
            .clone()
    };
    let stream = tokio::net::TcpStream::connect(address)
        .await
        .map_err(|error| format!("could not connect to discovered peer: {error}"))?;
    let result = run_pairing(
        stream,
        identity,
        HandshakeRole::Initiator,
        app,
        pairing.0.clone(),
        workspace.0.clone(),
        Some(peer.device_id.clone()),
    )
    .await?;
    Ok(result)
}

#[tauri::command]
async fn reconnect_trusted_peer(
    app: tauri::AppHandle,
    peer: PairAddress,
    network: tauri::State<'_, NetworkState>,
    pairing: tauri::State<'_, PairingState>,
    workspace: tauri::State<'_, WorkspaceState>,
) -> Result<PairingResult, String> {
    let address: std::net::SocketAddr = peer
        .address
        .parse()
        .map_err(|_| "peer address is invalid")?;
    let device_id = DeviceId(peer.device_id.clone());
    let trusted_peer = TrustedPeerStore::for_current_user()
        .map_err(|error| error.to_string())?
        .list()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|trusted| trusted.device_id == device_id)
        .ok_or("this device has no saved approval for that peer")?;
    let identity = {
        let inner = network
            .0
            .lock()
            .map_err(|_| "network state is unavailable")?;
        inner
            .identity
            .as_ref()
            .ok_or("start local discovery first")?
            .clone()
    };
    let stream = tokio::net::TcpStream::connect(address)
        .await
        .map_err(|error| format!("could not connect to discovered peer: {error}"))?;
    let paired =
        connect_trusted_over_stream(stream, &identity, HandshakeRole::Initiator, &trusted_peer)
            .await
            .map_err(|error| error.to_string())?;
    activate_session(
        paired,
        app,
        pairing.0.clone(),
        workspace.0.clone(),
        Some(peer.device_id),
    )
    .await
}

#[tauri::command]
fn list_trusted_peers() -> Result<Vec<String>, String> {
    TrustedPeerStore::for_current_user()
        .map_err(|error| error.to_string())?
        .list()
        .map(|peers| peers.into_iter().map(|peer| peer.device_id.0).collect())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn respond_to_pairing(
    request_id: String,
    approved: bool,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    let sender = pairing
        .0
        .pending
        .lock()
        .map_err(|_| "pairing state is unavailable")?
        .remove(&request_id)
        .ok_or("pairing request expired")?;
    sender
        .send(approved)
        .map_err(|_| "pairing request ended".to_owned())
}

#[tauri::command]
fn respond_to_window_share(
    request_id: String,
    approved: bool,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    let (_, sender) = pairing
        .0
        .pending_window_shares
        .lock()
        .map_err(|_| "window-share request state is unavailable")?
        .remove(&request_id)
        .ok_or("window-share request expired")?;
    sender
        .send(approved)
        .map_err(|_| "window-share request ended".to_owned())
}

#[tauri::command]
fn get_remote_window_frame(
    peer_device_id: String,
    pairing: tauri::State<'_, PairingState>,
) -> Result<Option<RemoteWindowFrame>, String> {
    pairing
        .0
        .window_frames
        .lock()
        .map(|frames| frames.get(&peer_device_id).cloned())
        .map_err(|_| "window-frame state is unavailable".to_owned())
}

#[tauri::command]
fn list_capturable_windows() -> Vec<CapturableWindowView> {
    enumerate_windows()
        .into_iter()
        .map(|window| CapturableWindowView {
            hwnd: window.hwnd,
            title: window.title,
        })
        .collect()
}

#[tauri::command]
async fn start_window_share(
    app: tauri::AppHandle,
    peer_device_id: String,
    hwnd: u64,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    use tauri::Emitter;

    if !enumerate_windows().iter().any(|window| window.hwnd == hwnd) {
        return Err("selected window is no longer visible; refresh the window list".into());
    }
    let session_id = pairing
        .0
        .media_contexts
        .lock()
        .map_err(|_| "session media state is unavailable")?
        .get(&peer_device_id)
        .map(|(session_id, _)| *session_id)
        .ok_or("peer has no active authenticated media session")?;
    let channel = pairing
        .0
        .services
        .lock()
        .map_err(|_| "session service state is unavailable")?
        .get(&peer_device_id)
        .map(|services| services.window_video.sender.clone())
        .ok_or("peer window-video lane is unavailable")?;

    let stop = Arc::new(AtomicBool::new(false));
    if let Ok(mut active) = pairing.0.window_share_stops.lock() {
        if active
            .get(&peer_device_id)
            .is_some_and(|current| !current.load(Ordering::Acquire))
        {
            return Err("stop the current window preview before starting another".into());
        }
        active.insert(peer_device_id.clone(), stop.clone());
    } else {
        return Err("window-share state is unavailable".into());
    }

    let (frame_tx, mut frame_rx) = mpsc::channel::<Result<CapturedFrame, String>>(1);
    let (ready_tx, ready_rx) = oneshot::channel();
    let worker_stop = stop.clone();
    let capture_worker = tokio::task::spawn_blocking(move || {
        let mut capture = match WindowCapture::start_after_user_consent(hwnd) {
            Ok(capture) => capture,
            Err(error) => {
                let message = error.to_string();
                let _ = ready_tx.send(Err(message.clone()));
                let _ = frame_tx.blocking_send(Err(message));
                return;
            }
        };
        let _ = ready_tx.send(Ok(()));
        while !worker_stop.load(Ordering::Acquire) {
            match capture.read_latest() {
                Ok(Some(frame)) => {
                    // A full one-frame handoff queue means encoding/networking
                    // is behind. Drop this newest frame rather than buffer delay.
                    let _ = frame_tx.try_send(Ok(frame));
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Ok(None) => std::thread::sleep(std::time::Duration::from_millis(5)),
                Err(error) => {
                    let _ = frame_tx.blocking_send(Err(error.to_string()));
                    break;
                }
            }
        }
    });

    let task_stop = stop.clone();
    let task_pairing = pairing.0.clone();
    let task_peer = peer_device_id.clone();
    let task_app = app.clone();
    let stream_id = NEXT_VIDEO_STREAM_ID.fetch_add(1, Ordering::Relaxed);
    pairing.0.window_share_targets.lock().map_err(|_| "window target state is unavailable")?.insert(peer_device_id.clone(), (stream_id, hwnd));
    tokio::spawn(async move {
        let started = std::time::Instant::now();
        let mut sent_sequence = 0u64;
        while let Some(result) = frame_rx.recv().await {
            let frame = match result {
                Ok(frame) => frame,
                Err(error) => {
                    let _ = task_app.emit("window-stream-error", error);
                    break;
                }
            };
            let encoded = tokio::task::spawn_blocking(move || encode_preview_jpeg(&frame)).await;
            let (width, height, payload) = match encoded {
                Ok(Ok(encoded)) => encoded,
                Ok(Err(error)) => {
                    let _ = task_app.emit("window-stream-error", error.to_string());
                    break;
                }
                Err(error) => {
                    let _ = task_app.emit("window-stream-error", error.to_string());
                    break;
                }
            };
            let Some(sequence) = sent_sequence.checked_add(1) else {
                let _ = task_app.emit("window-stream-error", "window stream sequence exhausted");
                break;
            };
            sent_sequence = sequence;
            let frame = MediaFrame {
                session_id,
                target: DeviceId(task_peer.clone()),
                stream_id,
                sequence,
                timestamp_100ns: (started.elapsed().as_nanos() / 100).min(u128::from(u64::MAX)) as u64,
                width,
                height,
                codec: VideoCodec::Jpeg,
                key_frame: true,
                payload,
            };
            if let Err(error) = send_media_frame(&channel, MediaLane::Window, &frame).await {
                if matches!(
                    error,
                    MediaSessionError::Transport(SessionTransportError::QueueFull)
                ) {
                    continue;
                }
                let _ = task_app.emit("window-stream-error", error.to_string());
                break;
            }
        }
        task_stop.store(true, Ordering::Release);
        let _ = capture_worker.await;
        if let Ok(mut active) = task_pairing.window_share_stops.lock() {
            if active
                .get(&task_peer)
                .is_some_and(|current| Arc::ptr_eq(current, &task_stop))
            {
                active.remove(&task_peer);
            }
        }
        release_window_portal_inputs(&task_pairing, &task_peer);
        if let Ok(mut targets) = task_pairing.window_share_targets.lock() {
            if targets.get(&task_peer).is_some_and(|current| *current == (stream_id, hwnd)) {
                targets.remove(&task_peer);
            }
        }
        let _ = task_app.emit("window-share-ended", &task_peer);
    });
    let readiness = match tokio::time::timeout(std::time::Duration::from_secs(10), ready_rx).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(error))) => Err(error),
        Ok(Err(_)) => Err("window capture worker ended during initialization".into()),
        Err(_) => Err("window capture did not initialize within 10 seconds".into()),
    };
    if let Err(error) = readiness {
        stop.store(true, Ordering::Release);
        return Err(error);
    }
    Ok(())
}

#[tauri::command]
fn stop_window_share(
    peer_device_id: String,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    let stop = pairing
        .0
        .window_share_stops
        .lock()
        .map_err(|_| "window-share state is unavailable")?
        .remove(&peer_device_id);
    if let Some(stop) = stop {
        stop.store(true, Ordering::Release);
    }
    Ok(())
}

#[tauri::command]
async fn revoke_remote_window_preview(
    peer_device_id: String,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    let session_id = pairing
        .0
        .media_contexts
        .lock()
        .map_err(|_| "session media state is unavailable")?
        .get(&peer_device_id)
        .map(|(session_id, _)| *session_id)
        .ok_or("peer has no active authenticated media session")?;
    let channel = pairing
        .0
        .services
        .lock()
        .map_err(|_| "session service state is unavailable")?
        .get(&peer_device_id)
        .map(|services| services.window_video.sender.clone())
        .ok_or("peer window-video lane is unavailable")?;
    let payload = serde_json::to_vec(&WindowControlAction::Stop)
        .map_err(|error| error.to_string())?;
    let frame = MediaFrame {
        session_id,
        target: DeviceId(peer_device_id.clone()),
        stream_id: NEXT_VIDEO_STREAM_ID.fetch_add(1, Ordering::Relaxed),
        sequence: 1,
        timestamp_100ns: 1,
        width: 1,
        height: 1,
        codec: VideoCodec::WindowControl,
        key_frame: true,
        payload,
    };
    send_media_frame(&channel, MediaLane::Window, &frame)
        .await
        .map_err(|error| error.to_string())?;
    if let Ok(mut frames) = pairing.0.window_frames.lock() {
        frames.remove(&peer_device_id);
    }
    Ok(())
}

#[tauri::command]
async fn send_window_portal_input(
    peer_device_id: String,
    source_stream_id: u64,
    event: WindowPortalInput,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    let send_lock = {
        let mut locks = pairing.0.window_control_locks.lock().map_err(|_| "window-control state is unavailable")?;
        locks.entry(peer_device_id.clone()).or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))).clone()
    };
    let _send_guard = send_lock.lock().await;
    let latest = pairing.0.window_frames.lock().map_err(|_| "window preview state is unavailable")?.get(&peer_device_id).cloned().ok_or("no approved live preview")?;
    if latest.stream_id != source_stream_id || latest.received_at_unix_ms.saturating_add(1500) < now_unix_ms() {
        return Err("the live preview is stale or belongs to another stream".into());
    }
    if let WindowPortalInput::PointerButton { button, .. } = event { if !(1..=5).contains(&button) { return Err("invalid pointer button".into()); } }
    if let WindowPortalInput::Key { virtual_key, .. } = event { if virtual_key > 0xFF { return Err("invalid virtual key".into()); } }
    let (session_id, _) = pairing.0.media_contexts.lock().map_err(|_| "session media state is unavailable")?.get(&peer_device_id).cloned().ok_or("peer has no active authenticated media session")?;
    let channel = pairing.0.services.lock().map_err(|_| "session service state is unavailable")?.get(&peer_device_id).map(|services| services.window_video.sender.clone()).ok_or("peer window-video lane is unavailable")?;
    let (stream_id, sequence) = {
        let mut streams = pairing.0.window_control_streams.lock().map_err(|_| "window-control state is unavailable")?;
        let (stream_id, sequence) = streams.entry(peer_device_id.clone()).or_insert_with(|| (NEXT_WINDOW_CONTROL_STREAM_ID.fetch_add(1, Ordering::Relaxed), 0));
        *sequence = sequence.checked_add(1).ok_or("window-control sequence exhausted")?;
        (*stream_id, *sequence)
    };
    let payload = serde_json::to_vec(&WindowControlAction::Input { source_stream_id, event }).map_err(|error| error.to_string())?;
    if payload.len() > 512 { return Err("window-control event exceeds the metadata limit".into()); }
    let frame = MediaFrame { session_id, target: DeviceId(peer_device_id), stream_id, sequence, timestamp_100ns: now_unix_ms().saturating_mul(10_000), width: 1, height: 1, codec: VideoCodec::WindowControl, key_frame: true, payload };
    send_media_frame(&channel, MediaLane::Window, &frame).await.map_err(|error| error.to_string())
}

fn now_unix_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis().min(u128::from(u64::MAX)) as u64
}

fn inject_portal_event(hwnd: u64, event: &WindowPortalInput) -> Result<(), String> {
    let result = match event {
        WindowPortalInput::PointerMove { x, y } => unified_workspace_agent::windows_input::inject_into_window(hwnd, InputKind::PointerMove, 0, *x, *y, 0, 0, 0),
        WindowPortalInput::PointerButton { x, y, button, down } => unified_workspace_agent::windows_input::inject_into_window(hwnd, InputKind::PointerButton, u32::from(*button), *x, *y, i32::from(*down), 0, 0),
        WindowPortalInput::PointerWheel { x, y, delta, horizontal } => unified_workspace_agent::windows_input::inject_into_window(hwnd, InputKind::PointerWheel, 0, *x, *y, *delta, if *horizontal { 1 << 1 } else { 0 }, 0),
        WindowPortalInput::Key { virtual_key, scan_code, extended, down } => unified_workspace_agent::windows_input::inject_into_window(hwnd, InputKind::Key, u32::from(*virtual_key), 0, 0, i32::from(*down), if *extended { 1 << 2 } else { 0 }, u32::from(*scan_code)),
    };
    result.map_err(|error| error.to_string())
}

fn dispatch_portal_event(pairing: &PairingInner, peer: &str, hwnd: u64, event: &WindowPortalInput) -> Result<(), String> {
    let mut held_by_peer = pairing.window_portal_held.lock().map_err(|_| "window input state is unavailable")?;
    let held = held_by_peer.entry(peer.to_owned()).or_default();
    validate_portal_transition(held, event)?;
    inject_portal_event(hwnd, event)?;
    if let Some((id, down)) = event.transition() {
        if down { held.push(event.clone()); }
        else { held.retain(|current| current.transition().is_none_or(|(held_id, _)| held_id != id)); }
    }
    Ok(())
}

fn validate_portal_transition(held: &[WindowPortalInput], event: &WindowPortalInput) -> Result<(), String> {
    if let Some((id, down)) = event.transition() {
        let already_held = held.iter().filter_map(WindowPortalInput::transition).any(|(held_id, _)| held_id == id);
        if (down && already_held) || (!down && !already_held) { return Err("unbalanced remote key/button transition".into()); }
    }
    Ok(())
}

fn release_window_portal_inputs(pairing: &PairingInner, peer: &str) {
    let target = pairing.window_share_targets.lock().ok().and_then(|targets| targets.get(peer).copied());
    let Some((_, hwnd)) = target else { return; };
    let Ok(mut held_by_peer) = pairing.window_portal_held.lock() else { return; };
    let Some(held) = held_by_peer.remove(peer) else { return; };
    for event in held.iter().filter_map(WindowPortalInput::released) {
        let _ = inject_portal_event(hwnd, &event);
    }
}

fn validate_window_preview_frame(
    frame: &unified_workspace_agent::MediaFrame,
) -> Result<(), &'static str> {
    if frame.codec != VideoCodec::Jpeg {
        return Err("the peer sent a window codec this desktop cannot display");
    }
    // Prevent hostile or accidental dimensions from causing extreme WebView
    // image-decoder allocations even when the transport payload is bounded.
    if frame.width > 3840
        || frame.height > 2160
        || u64::from(frame.width) * u64::from(frame.height) > 8_294_400
    {
        return Err("the peer sent a window frame larger than the 4K preview limit");
    }
    if !frame.payload.starts_with(&[0xff, 0xd8]) || !frame.payload.ends_with(&[0xff, 0xd9]) {
        return Err("the peer sent invalid JPEG data");
    }
    Ok(())
}

#[tauri::command]
fn disconnect_peer(
    app: tauri::AppHandle,
    peer_device_id: String,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    use tauri::Emitter;

    let abort = pairing
        .0
        .session_tasks
        .lock()
        .map_err(|_| "session state is unavailable")?
        .remove(&peer_device_id)
        .ok_or("peer has no active session")?;
    // Aborting the RPC owner drops the encrypted stream, closing this session.
    abort.abort();
    if let Ok(mut active) = pairing.0.active.lock() {
        active.remove(&peer_device_id);
    }
    if let Ok(mut clients) = pairing.0.clients.lock() {
        clients.remove(&peer_device_id);
    }
    let _ = app.emit("pairing-ended", &peer_device_id);
    Ok(())
}

async fn run_pairing(
    stream: tokio::net::TcpStream,
    identity: Arc<DeviceIdentity>,
    role: HandshakeRole,
    app: tauri::AppHandle,
    pairing: Arc<PairingInner>,
    workspace: Arc<Mutex<Option<Arc<SharedWorkspace>>>>,
    expected_peer: Option<String>,
) -> Result<PairingResult, String> {
    let challenge_app = app.clone();
    let challenge_pairing = pairing.clone();
    let paired = pair_over_stream(stream, &identity, role, move |challenge| {
        request_pairing_approval(challenge_app.clone(), challenge_pairing.clone(), challenge)
    })
    .await
    .map_err(|error| error.to_string())?;
    activate_session(paired, app, pairing, workspace, expected_peer).await
}

async fn run_accept(
    stream: tokio::net::TcpStream,
    identity: Arc<DeviceIdentity>,
    app: tauri::AppHandle,
    pairing: Arc<PairingInner>,
    workspace: Arc<Mutex<Option<Arc<SharedWorkspace>>>>,
) -> Result<PairingResult, String> {
    let trusted_peers = TrustedPeerStore::for_current_user()
        .map_err(|error| error.to_string())?
        .list()
        .map_err(|error| error.to_string())?;
    let challenge_app = app.clone();
    let challenge_pairing = pairing.clone();
    let paired =
        accept_pair_or_trusted_over_stream(stream, &identity, &trusted_peers, move |challenge| {
            request_pairing_approval(challenge_app.clone(), challenge_pairing.clone(), challenge)
        })
        .await
        .map_err(|error| error.to_string())?;
    activate_session(paired, app, pairing, workspace, None).await
}

async fn request_pairing_approval(
    app: tauri::AppHandle,
    pairing: Arc<PairingInner>,
    challenge: SessionChallenge,
) -> bool {
    use tauri::Emitter;
    let request_id = format!(
        "{}-{}",
        std::process::id(),
        NEXT_PAIRING_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    let (approve, result) = oneshot::channel();
    if let Ok(mut pending) = pairing.pending.lock() {
        pending.insert(request_id.clone(), approve);
    } else {
        return false;
    }
    let event = PairingChallengeView {
        request_id: request_id.clone(),
        local_device: challenge.local_device.0,
        peer_device: challenge.peer_device.0,
        verification_code: challenge.verification_code,
    };
    if app.emit("pairing-challenge", event).is_err() {
        if let Ok(mut pending) = pairing.pending.lock() {
            pending.remove(&request_id);
        }
        return false;
    }
    let approved = tokio::time::timeout(std::time::Duration::from_secs(120), result)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
    if let Ok(mut pending) = pairing.pending.lock() {
        pending.remove(&request_id);
    }
    approved
}

async fn activate_session(
    paired: PairedTransport<tokio::net::TcpStream>,
    completion_app: tauri::AppHandle,
    pairing: Arc<PairingInner>,
    workspace: Arc<Mutex<Option<Arc<SharedWorkspace>>>>,
    expected_peer: Option<String>,
) -> Result<PairingResult, String> {
    use tauri::Emitter;
    let peer_device_id = paired.peer_device().0.clone();
    if expected_peer
        .as_deref()
        .is_some_and(|expected| expected != peer_device_id)
    {
        return Err(
            "authenticated identity differs from the discovered peer; connection closed".into(),
        );
    }
    let session_id = paired.session_id();
    let local_device_id = paired.local_device().clone();
    let (sender, receiver, approval) = paired.into_encrypted();
    let is_new_pairing = approval.is_some();
    if let Some(approval) = approval {
        persist_approval(&approval)?;
    }
    let has_workspace = workspace
        .lock()
        .map_err(|_| "workspace state is unavailable")?
        .is_some();
    if let Ok(mut trusted) = pairing.trusted.lock() {
        if !trusted.contains(&peer_device_id) {
            trusted.push(peer_device_id.clone());
        }
    }
    if let Ok(mut active) = pairing.active.lock() {
        active.insert(peer_device_id.clone());
    }
    let result = PairingResult {
        peer_device_id,
        session_id: session_id.0.to_string(),
        trusted: true,
        transport_active: true,
        detail: match (is_new_pairing, has_workspace) {
            (true, true) => "Both users approved; the encrypted transport is active and this device is serving its authorized workspace. Input, display, and live window services are not active yet.".into(),
            (true, false) => "Both users approved; the encrypted transport is active. Choose a Shared Workspace folder to enable this device's host-side file service. Input, display, and live window services are not active yet.".into(),
            (false, true) => "The previously approved peer identity was authenticated with fresh keys; the encrypted transport is active and this device is serving its authorized workspace. Input, display, and live window services are not active yet.".into(),
            (false, false) => "The previously approved peer identity was authenticated with fresh keys; the encrypted transport is active. Choose a Shared Workspace folder to enable this device's host-side file service. Input, display, and live window services are not active yet.".into(),
        },
    };
    let peer_for_task = result.peer_device_id.clone();
    let session_pairing = pairing.clone();
    let session_app = completion_app.clone();
    let (services, transport_task) = spawn_session_transport(sender, receiver);
    if let Ok(mut contexts) = pairing.media_contexts.lock() {
        contexts.insert(
            peer_for_task.clone(),
            (session_id, local_device_id.clone()),
        );
    }
    let (client, mut file_task) = spawn_file_rpc(
        services.files.sender.clone(),
        services.files.receiver.clone(),
        workspace,
    );
    let window_frames = services.window_video.receiver.clone();
    let window_pairing = pairing.clone();
    let window_app = completion_app.clone();
    let window_peer = peer_for_task.clone();
    let mut media_receiver =
        MediaSessionReceiver::new(session_id, local_device_id, MediaLane::Window);
    let window_video_task = tokio::spawn(async move {
        let mut approved_stream = None;
        let mut denied_stream = None;
        let mut control_times = std::collections::VecDeque::<std::time::Instant>::new();
        while let Some(bytes) = window_frames.recv().await {
            let frame = match media_receiver.accept(&bytes) {
                Ok(frame) if frame.codec == VideoCodec::WindowControl => {
                    match serde_json::from_slice::<WindowControlAction>(&frame.payload) {
                        Ok(WindowControlAction::Stop) => {
                            release_window_portal_inputs(&window_pairing, &window_peer);
                            if let Ok(mut shares) = window_pairing.window_share_stops.lock() {
                                if let Some(stop) = shares.remove(&window_peer) {
                                    stop.store(true, Ordering::Release);
                                }
                            }
                            let _ = window_app.emit("window-share-ended", &window_peer);
                            continue;
                        }
                        Ok(WindowControlAction::Input { source_stream_id, event }) => {
                            let now = std::time::Instant::now();
                            while control_times.front().is_some_and(|time| now.duration_since(*time) >= std::time::Duration::from_secs(1)) { control_times.pop_front(); }
                            if control_times.len() >= 180 { let _ = window_app.emit("window-stream-error", "remote window input rate limit exceeded"); continue; }
                            control_times.push_back(now);
                            let target = window_pairing.window_share_targets.lock().ok().and_then(|targets| targets.get(&window_peer).copied());
                            if let Some((_, hwnd)) = target.filter(|(active_stream, _)| *active_stream == source_stream_id) {
                                if let Err(error) = dispatch_portal_event(&window_pairing, &window_peer, hwnd, &event) { let _ = window_app.emit("window-stream-error", format!("remote window input rejected: {error}")); }
                            } else {
                                let _ = window_app.emit("window-stream-error", "remote input rejected: no matching consented capture target");
                            }
                            continue;
                        }
                        Err(error) => {
                            let _ = window_app.emit("window-stream-error", error.to_string());
                            break;
                        }
                    }
                }
                Ok(frame) => match validate_window_preview_frame(&frame) {
                    Ok(()) => frame,
                    Err(error) => {
                        let _ = window_app.emit("window-stream-error", error);
                        break;
                    }
                },
                Err(error) => {
                    let _ = window_app.emit("window-stream-error", error.to_string());
                    break;
                }
            };

            if approved_stream != Some(frame.stream_id) {
                if denied_stream == Some(frame.stream_id) {
                    continue;
                }
                approved_stream = None;
                if let Ok(mut stored) = window_pairing.window_frames.lock() {
                    stored.remove(&window_peer);
                }
                let approved = request_window_share_approval(
                    window_app.clone(),
                    window_pairing.clone(),
                    window_peer.clone(),
                    frame.stream_id,
                    frame.width,
                    frame.height,
                )
                .await;
                if !approved {
                    denied_stream = Some(frame.stream_id);
                    if let Ok(mut stored) = window_pairing.window_frames.lock() {
                        stored.remove(&window_peer);
                    }
                    let _ = window_app.emit("window-share-ended", &window_peer);
                    continue;
                }
                approved_stream = Some(frame.stream_id);
                denied_stream = None;
            }

            let latest = RemoteWindowFrame {
                peer_device_id: window_peer.clone(),
                stream_id: frame.stream_id,
                sequence: frame.sequence,
                timestamp_100ns: frame.timestamp_100ns,
                received_at_unix_ms: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64,
                width: frame.width,
                height: frame.height,
                jpeg_base64: base64::engine::general_purpose::STANDARD.encode(frame.payload),
            };
            if let Ok(mut stored) = window_pairing.window_frames.lock() {
                stored.insert(window_peer.clone(), latest);
            }
        }
        release_window_portal_inputs(&window_pairing, &window_peer);
        if let Ok(mut stored) = window_pairing.window_frames.lock() {
            stored.remove(&window_peer);
        }
        let _ = window_app.emit("window-share-ended", &window_peer);
    });
    let mut transport_task = transport_task;
    let abort_children = AbortChildrenOnDrop(vec![
        file_task.abort_handle(),
        window_video_task.abort_handle(),
        transport_task.abort_handle(),
    ]);
    let task = tokio::spawn(async move {
        let _abort_children = abort_children;
        tokio::select! {
            _ = &mut file_task => {},
            _ = &mut transport_task => {},
        }
    });
    if let Ok(mut tasks) = pairing.session_tasks.lock() {
        tasks.insert(peer_for_task.clone(), task.abort_handle());
    }
    if let Ok(mut clients) = pairing.clients.lock() {
        clients.insert(peer_for_task.clone(), client);
    }
    if let Ok(mut services_by_peer) = pairing.services.lock() {
        services_by_peer.insert(peer_for_task.clone(), services);
    }
    tokio::spawn(async move {
        let _ = task.await;
        release_window_portal_inputs(&session_pairing, &peer_for_task);
        if let Ok(mut active) = session_pairing.active.lock() {
            active.remove(&peer_for_task);
        }
        if let Ok(mut clients) = session_pairing.clients.lock() {
            clients.remove(&peer_for_task);
        }
        if let Ok(mut services_by_peer) = session_pairing.services.lock() {
            services_by_peer.remove(&peer_for_task);
        }
        if let Ok(mut frames) = session_pairing.window_frames.lock() {
            frames.remove(&peer_for_task);
        }
        if let Ok(mut contexts) = session_pairing.media_contexts.lock() {
            contexts.remove(&peer_for_task);
        }
        if let Ok(mut locks) = session_pairing.window_control_locks.lock() { locks.remove(&peer_for_task); }
        if let Ok(mut shares) = session_pairing.window_share_stops.lock() {
            if let Some(stop) = shares.remove(&peer_for_task) {
                stop.store(true, Ordering::Release);
            }
        }
        reject_pending_window_share_requests(&session_pairing, &peer_for_task);
        if let Ok(mut tasks) = session_pairing.session_tasks.lock() {
            tasks.remove(&peer_for_task);
        }
        let _ = session_app.emit("pairing-ended", peer_for_task);
    });
    let _ = completion_app.emit("pairing-established", &result);
    Ok(result)
}

async fn request_window_share_approval(
    app: tauri::AppHandle,
    pairing: Arc<PairingInner>,
    peer_device_id: String,
    stream_id: u64,
    width: u32,
    height: u32,
) -> bool {
    use tauri::Emitter;

    let request_id = format!(
        "{}-{}",
        std::process::id(),
        NEXT_WINDOW_SHARE_ID.fetch_add(1, Ordering::Relaxed)
    );
    let (approve, result) = oneshot::channel();
    if let Ok(mut pending) = pairing.pending_window_shares.lock() {
        pending.insert(request_id.clone(), (peer_device_id.clone(), approve));
    } else {
        return false;
    }
    let request = WindowShareRequestView {
        request_id: request_id.clone(),
        peer_device_id,
        stream_id,
        width,
        height,
    };
    if app.emit("window-share-request", request).is_err() {
        if let Ok(mut pending) = pairing.pending_window_shares.lock() {
            pending.remove(&request_id);
        }
        return false;
    }
    let approved = tokio::time::timeout(std::time::Duration::from_secs(120), result)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or(false);
    if let Ok(mut pending) = pairing.pending_window_shares.lock() {
        pending.remove(&request_id);
    }
    approved
}

fn reject_pending_window_share_requests(pairing: &PairingInner, peer_device_id: &str) {
    let Ok(mut pending) = pairing.pending_window_shares.lock() else {
        return;
    };
    let requests = pending
        .iter()
        .filter(|(_, (peer, _))| peer == peer_device_id)
        .map(|(request_id, _)| request_id.clone())
        .collect::<Vec<_>>();
    for request_id in requests {
        if let Some((_, sender)) = pending.remove(&request_id) {
            let _ = sender.send(false);
        }
    }
}

fn persist_approval(approval: &PairingApproval) -> Result<(), String> {
    let store = TrustedPeerStore::for_current_user().map_err(|error| error.to_string())?;
    store
        .add_approved(approval)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn remote_file_client(
    pairing: &PairingState,
    peer_device_id: &str,
) -> Result<FileRpcClient, String> {
    pairing
        .0
        .clients
        .lock()
        .map_err(|_| "pairing state is unavailable")?
        .get(peer_device_id)
        .cloned()
        .ok_or_else(|| "peer has no active encrypted file RPC session".to_owned())
}

#[tauri::command]
async fn list_remote_workspace(
    peer_device_id: String,
    directory: Option<String>,
    pairing: tauri::State<'_, PairingState>,
) -> Result<Vec<SharedEntry>, String> {
    match remote_file_client(&pairing, &peer_device_id)?
        .request(FileRequest::List { directory })
        .await
        .map_err(|error| error.to_string())?
    {
        FileResponse::Entries { entries } => Ok(entries),
        FileResponse::Error { message } => Err(message),
        _ => Err("peer returned an unexpected workspace response".into()),
    }
}

#[tauri::command]
async fn create_remote_directory(
    peer_device_id: String,
    path: String,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    match remote_file_client(&pairing, &peer_device_id)?
        .request(FileRequest::CreateDirectory { path })
        .await
        .map_err(|error| error.to_string())?
    {
        FileResponse::Ok => Ok(()),
        FileResponse::Error { message } => Err(message),
        _ => Err("peer returned an unexpected workspace response".into()),
    }
}

#[tauri::command]
async fn read_remote_file_range(
    peer_device_id: String,
    path: String,
    offset: u64,
    length: usize,
    pairing: tauri::State<'_, PairingState>,
) -> Result<Vec<u8>, String> {
    if length > MAX_RPC_READ_CHUNK {
        return Err("requested download chunk exceeds the RPC limit".into());
    }
    match remote_file_client(&pairing, &peer_device_id)?
        .request(FileRequest::ReadRange {
            path,
            offset,
            length,
        })
        .await
        .map_err(|error| error.to_string())?
    {
        FileResponse::Bytes { bytes } => Ok(bytes),
        FileResponse::Error { message } => Err(message),
        _ => Err("peer returned an unexpected workspace response".into()),
    }
}

#[tauri::command]
async fn download_remote_file_to_path(
    app: tauri::AppHandle,
    peer_device_id: String,
    path: String,
    expected_size: u64,
    destination_path: String,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    use tauri::Emitter;
    use tokio::io::AsyncWriteExt;

    let destination = std::path::PathBuf::from(destination_path);
    let parent = destination
        .parent()
        .ok_or("save location has no parent folder")?;
    let name = destination
        .file_name()
        .ok_or("save location has no file name")?;
    if name.is_empty()
        || !tokio::fs::metadata(parent)
            .await
            .map_err(|error| error.to_string())?
            .is_dir()
    {
        return Err("save location is not in an available folder".into());
    }
    match tokio::fs::symlink_metadata(&destination).await {
        Ok(metadata) if metadata.file_type().is_file() => {}
        Ok(_) => return Err("save location exists but is not a regular file".into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }

    let client = remote_file_client(&pairing, &peer_device_id)?;
    let starting_metadata = match client
        .request(FileRequest::Metadata { path: path.clone() })
        .await
        .map_err(|error| error.to_string())?
    {
        FileResponse::FileMetadata { metadata } if metadata.size == expected_size => metadata,
        FileResponse::FileMetadata { .. } => {
            return Err("peer file changed since the workspace listing; refresh and retry".into())
        }
        FileResponse::Error { message } => return Err(message),
        _ => return Err("peer returned an unexpected file metadata response".into()),
    };
    let (temporary, mut output) = loop {
        let id = NEXT_DOWNLOAD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = parent.join(format!(".crosswarp-{}-{id}.part", std::process::id()));
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
        {
            Ok(file) => break (path, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    };
    let emit_progress = |bytes_sent, complete| {
        let _ = app.emit(
            "remote-transfer-progress",
            RemoteTransferProgress {
                peer_device_id: peer_device_id.clone(),
                direction: "download",
                bytes_sent,
                total_bytes: expected_size,
                complete,
            },
        );
    };
    emit_progress(0, false);
    let result = async {
        let mut download_hasher = sha2::Sha256::new();
        let mut offset = 0u64;
        while offset < expected_size {
            let length = (expected_size - offset).min(MAX_RPC_READ_CHUNK as u64) as usize;
            let bytes = match client
                .request(FileRequest::ReadRange {
                    path: path.clone(),
                    offset,
                    length,
                })
                .await
                .map_err(|error| error.to_string())?
            {
                FileResponse::Bytes { bytes } if bytes.len() == length => bytes,
                FileResponse::Bytes { .. } => {
                    return Err(
                        "peer file changed during download; received an incomplete range".into(),
                    )
                }
                FileResponse::Error { message } => return Err(message),
                _ => return Err("peer returned an unexpected download response".into()),
            };
            output
                .write_all(&bytes)
                .await
                .map_err(|error| error.to_string())?;
            download_hasher.update(&bytes);
            offset += bytes.len() as u64;
            emit_progress(offset, false);
        }
        let ending_metadata = match client
            .request(FileRequest::Metadata { path: path.clone() })
            .await
            .map_err(|error| error.to_string())?
        {
            FileResponse::FileMetadata { metadata } => metadata,
            FileResponse::Error { message } => return Err(message),
            _ => return Err("peer returned an unexpected file verification response".into()),
        };
        if ending_metadata != starting_metadata {
            return Err("peer file changed during download; temporary data was discarded".into());
        }
        output.sync_all().await.map_err(|error| error.to_string())?;
        drop(output);
        let downloaded_digest: [u8; 32] = download_hasher.finalize().into();
        commit_download_file(
            &temporary,
            &destination,
            downloaded_digest,
            starting_metadata.sha256,
        )
        .await?;
        emit_progress(expected_size, true);
        Ok(())
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result
}

async fn commit_download_file(
    temporary: &std::path::Path,
    destination: &std::path::Path,
    downloaded_digest: [u8; 32],
    expected_digest: [u8; 32],
) -> Result<(), String> {
    if downloaded_digest != expected_digest {
        return Err("downloaded bytes do not match the peer file SHA-256".into());
    }
    tokio::fs::rename(temporary, destination)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn upload_remote_file(
    app: tauri::AppHandle,
    peer_device_id: String,
    destination_path: String,
    source_path: String,
    replace_existing: bool,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    use tauri::Emitter;

    let client = remote_file_client(&pairing, &peer_device_id)?;
    let metadata = tokio::fs::metadata(&source_path)
        .await
        .map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("selected upload source is not a regular file".into());
    }
    let total_bytes = metadata.len();
    let emit_progress = |bytes_sent, complete| {
        let _ = app.emit(
            "remote-transfer-progress",
            RemoteTransferProgress {
                peer_device_id: peer_device_id.clone(),
                direction: "upload",
                bytes_sent,
                total_bytes,
                complete,
            },
        );
    };
    emit_progress(0, false);
    let transfer = match client
        .request(FileRequest::BeginUpload {
            path: destination_path,
            length: metadata.len(),
        })
        .await
        .map_err(|error| error.to_string())?
    {
        FileResponse::Transfer { transfer } => transfer,
        FileResponse::Error { message } => return Err(message),
        _ => return Err("peer returned an unexpected upload response".into()),
    };
    let result = async {
        let mut file = tokio::fs::File::open(&source_path)
            .await
            .map_err(|error| error.to_string())?;
        let mut hasher = sha2::Sha256::new();
        let mut offset = 0u64;
        loop {
            let mut chunk = vec![0u8; MAX_RPC_UPLOAD_CHUNK];
            let read = tokio::io::AsyncReadExt::read(&mut file, &mut chunk)
                .await
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            chunk.truncate(read);
            hasher.update(&chunk);
            match client
                .request(FileRequest::UploadChunk {
                    transfer,
                    offset,
                    bytes: chunk,
                })
                .await
                .map_err(|error| error.to_string())?
            {
                FileResponse::Progress { written } if written == offset + read as u64 => {
                    offset = written;
                    emit_progress(offset, false);
                }
                FileResponse::Error { message } => return Err(message),
                _ => return Err("peer returned an unexpected upload progress response".into()),
            }
        }
        let digest: [u8; 32] = hasher.finalize().into();
        let commit = if replace_existing {
            FileRequest::CommitUploadReplace {
                transfer,
                sha256: digest,
                confirmed: true,
            }
        } else {
            FileRequest::CommitUpload {
                transfer,
                sha256: digest,
            }
        };
        match client
            .request(commit)
            .await
            .map_err(|error| error.to_string())?
        {
            FileResponse::Digest { sha256 } if sha256 == digest => {
                emit_progress(total_bytes, true);
                Ok(())
            }
            FileResponse::Error { message } => Err(message),
            _ => Err("peer returned an unexpected upload completion response".into()),
        }
    }
    .await;
    if result.is_err() {
        let _ = client.request(FileRequest::AbortUpload { transfer }).await;
    }
    result
}

#[tauri::command]
async fn rename_remote_entry(
    peer_device_id: String,
    source: String,
    destination: String,
    confirmed: bool,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    match remote_file_client(&pairing, &peer_device_id)?
        .request(FileRequest::Rename {
            source,
            destination,
            confirmed,
        })
        .await
        .map_err(|error| error.to_string())?
    {
        FileResponse::Ok => Ok(()),
        FileResponse::Error { message } => Err(message),
        _ => Err("peer returned an unexpected rename response".into()),
    }
}

#[tauri::command]
async fn delete_remote_file(
    peer_device_id: String,
    path: String,
    confirmed: bool,
    pairing: tauri::State<'_, PairingState>,
) -> Result<(), String> {
    match remote_file_client(&pairing, &peer_device_id)?
        .request(FileRequest::Delete { path, confirmed })
        .await
        .map_err(|error| error.to_string())?
    {
        FileResponse::Ok => Ok(()),
        FileResponse::Error { message } => Err(message),
        _ => Err("peer returned an unexpected delete response".into()),
    }
}

#[tauri::command]
fn discover_local_peers(state: tauri::State<'_, NetworkState>) -> Result<Vec<PeerView>, String> {
    let inner = state.0.lock().map_err(|_| "network state is unavailable")?;
    let browser = inner.browser.as_ref().ok_or("start local network first")?;
    let mut peers = Vec::new();
    for _ in 0..16 {
        let Some(peer) = browser
            .next_peer(std::time::Duration::from_millis(10))
            .map_err(|error| error.to_string())?
        else {
            break;
        };
        peers.push(PeerView {
            device_id: peer.device_id.0,
            addresses: peer
                .addresses
                .into_iter()
                .map(|address| address.to_string())
                .collect(),
        });
    }
    Ok(peers)
}

#[derive(Serialize)]
struct WorkspaceView {
    root: String,
    entries: Vec<SharedEntry>,
}

#[tauri::command]
fn configure_shared_workspace(
    path: String,
    state: tauri::State<'_, WorkspaceState>,
) -> Result<WorkspaceView, String> {
    let workspace = SharedWorkspace::open(&path).map_err(|error| error.to_string())?;
    let entries = workspace
        .list_directory(None, 10_000)
        .map_err(|error| error.to_string())?;
    let root = workspace.root_path().to_string_lossy().into_owned();
    *state
        .0
        .lock()
        .map_err(|_| "workspace state is unavailable".to_owned())? = Some(Arc::new(workspace));
    Ok(WorkspaceView { root, entries })
}

#[tauri::command]
fn list_workspace_directory(
    path: Option<String>,
    state: tauri::State<'_, WorkspaceState>,
) -> Result<Vec<SharedEntry>, String> {
    let guard = state
        .0
        .lock()
        .map_err(|_| "workspace state is unavailable".to_owned())?;
    let workspace = guard
        .as_ref()
        .ok_or_else(|| "select a Shared Workspace folder first".to_owned())?;
    workspace
        .list_directory(path.as_deref(), 10_000)
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn create_workspace_directory(
    path: String,
    state: tauri::State<'_, WorkspaceState>,
) -> Result<(), String> {
    let guard = state
        .0
        .lock()
        .map_err(|_| "workspace state is unavailable".to_owned())?;
    let workspace = guard
        .as_ref()
        .ok_or_else(|| "select a Shared Workspace folder first".to_owned())?;
    workspace
        .create_directory(&path)
        .map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(WorkspaceState::default())
        .manage(NetworkState::default())
        .manage(PairingState::default())
        .manage(InputDiagnosticState::default())
        .invoke_handler(tauri::generate_handler![
            get_local_status,
            get_platform_report,
            start_input_capture_diagnostic,
            stop_input_capture_diagnostic,
            get_input_capture_diagnostic,
            start_local_network,
            discover_local_peers,
            pair_with_peer,
            reconnect_trusted_peer,
            list_trusted_peers,
            respond_to_pairing,
            respond_to_window_share,
            get_remote_window_frame,
            list_capturable_windows,
            start_window_share,
            stop_window_share,
            revoke_remote_window_preview,
            send_window_portal_input,
            disconnect_peer,
            list_remote_workspace,
            create_remote_directory,
            read_remote_file_range,
            download_remote_file_to_path,
            upload_remote_file,
            rename_remote_entry,
            delete_remote_file,
            configure_shared_workspace,
            list_workspace_directory,
            create_workspace_directory
        ])
        .run(tauri::generate_context!())
        .expect("failed to run CrossWarp desktop shell");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_diagnostic_report_exposes_only_aggregate_counts() {
        let counters = InputDiagnosticCounters::default();
        counters.total.store(9, Ordering::Relaxed);
        counters.pointer_move.store(4, Ordering::Relaxed);
        counters.pointer_button.store(2, Ordering::Relaxed);
        counters.pointer_wheel.store(1, Ordering::Relaxed);
        counters.key_transition.store(2, Ordering::Relaxed);

        let report = input_diagnostic_report(true, &counters);
        assert!(report.running);
        assert_eq!(report.total, 9);
        assert_eq!(report.pointer_move, 4);
        assert_eq!(report.pointer_button, 2);
        assert_eq!(report.pointer_wheel, 1);
        assert_eq!(report.key_transition, 2);
        assert!(report.error.is_none());
    }

    #[tokio::test]
    async fn completed_download_atomically_replaces_the_selected_file() {
        let root = std::env::temp_dir().join(format!(
            "crosswarp-download-{}-{}",
            std::process::id(),
            NEXT_DOWNLOAD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        tokio::fs::create_dir(&root).await.unwrap();
        let destination = root.join("report.txt");
        let temporary = root.join(".crosswarp-transfer.part");
        tokio::fs::write(&destination, b"old contents")
            .await
            .unwrap();
        tokio::fs::write(&temporary, b"complete replacement")
            .await
            .unwrap();

        let downloaded_digest: [u8; 32] = sha2::Sha256::digest(b"complete replacement").into();
        commit_download_file(
            &temporary,
            &destination,
            downloaded_digest,
            downloaded_digest,
        )
        .await
        .unwrap();

        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"complete replacement"
        );
        assert!(!temporary.exists());
        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[tokio::test]
    async fn corrupt_download_does_not_replace_the_selected_file() {
        let root = std::env::temp_dir().join(format!(
            "crosswarp-download-corrupt-{}-{}",
            std::process::id(),
            NEXT_DOWNLOAD_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        tokio::fs::create_dir(&root).await.unwrap();
        let destination = root.join("report.txt");
        let temporary = root.join(".crosswarp-transfer.part");
        tokio::fs::write(&destination, b"original file")
            .await
            .unwrap();
        tokio::fs::write(&temporary, b"corrupt transfer")
            .await
            .unwrap();
        let actual: [u8; 32] = sha2::Sha256::digest(b"corrupt transfer").into();
        let expected: [u8; 32] = sha2::Sha256::digest(b"peer expected bytes").into();

        assert!(
            commit_download_file(&temporary, &destination, actual, expected)
                .await
                .is_err()
        );
        assert_eq!(
            tokio::fs::read(&destination).await.unwrap(),
            b"original file"
        );
        tokio::fs::remove_file(temporary).await.unwrap();
        tokio::fs::remove_dir_all(root).await.unwrap();
    }

    #[test]
    fn window_preview_accepts_bounded_jpeg_and_rejects_unsupported_or_excessive_frames() {
        let identity = DeviceIdentity::generate();
        let mut frame = unified_workspace_agent::MediaFrame {
            session_id: SessionId::new(),
            target: identity.device_id(),
            stream_id: 1,
            sequence: 0,
            timestamp_100ns: 1,
            width: 1920,
            height: 1080,
            codec: VideoCodec::Jpeg,
            key_frame: true,
            payload: vec![0xff, 0xd8, 0xff, 0xd9],
        };
        assert!(validate_window_preview_frame(&frame).is_ok());
        frame.codec = VideoCodec::H264;
        assert!(validate_window_preview_frame(&frame).is_err());
        frame.codec = VideoCodec::Jpeg;
        frame.payload = vec![1, 2, 3, 4];
        assert!(validate_window_preview_frame(&frame).is_err());
        frame.payload = vec![0xff, 0xd8, 0xff, 0xd9];
        frame.width = 7680;
        frame.height = 4320;
        assert!(validate_window_preview_frame(&frame).is_err());
    }

    #[test]
    fn window_stop_control_has_a_typed_wire_shape() {
        assert_eq!(
            serde_json::to_value(WindowControlAction::Stop).unwrap(),
            serde_json::json!({"action": "stop"})
        );
        assert!(serde_json::from_value::<WindowControlAction>(
            serde_json::json!({"action": "close_source_application"})
        )
        .is_err());
    }

    #[test]
    fn window_portal_pointer_and_keyboard_controls_have_bounded_typed_shapes() {
        let pointer = WindowControlAction::Input {
            source_stream_id: 42,
            event: WindowPortalInput::PointerButton { x: 0, y: 65535, button: 1, down: true },
        };
        let value = serde_json::to_value(pointer).unwrap();
        assert_eq!(value["action"], "input");
        assert_eq!(value["source_stream_id"], 42);
        assert_eq!(value["event"]["kind"], "pointer_button");
        assert_eq!(value["event"]["y"], 65535);
        assert!(serde_json::from_value::<WindowControlAction>(serde_json::json!({"action":"input","source_stream_id":42,"event":{"kind":"launch_process"}})).is_err());
    }

    #[test]
    fn portal_input_requires_balanced_button_and_key_transitions() {
        let down = WindowPortalInput::PointerButton { x: 10, y: 20, button: 1, down: true };
        let up = WindowPortalInput::PointerButton { x: 10, y: 20, button: 1, down: false };
        assert!(validate_portal_transition(&[], &down).is_ok());
        assert!(validate_portal_transition(std::slice::from_ref(&down), &down).is_err());
        assert!(validate_portal_transition(&[], &up).is_err());
        assert!(validate_portal_transition(&[down], &up).is_ok());
    }
}
