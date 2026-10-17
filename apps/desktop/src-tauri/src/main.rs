#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::oneshot;
use unified_workspace_agent::protocol::{Capability, CapabilityState, DeviceId, SessionId};
use unified_workspace_agent::{
    pair_over_stream, serve_file_rpc, DeviceIdentity, HandshakeRole, HostPlatform, IdentityStore,
    LanEndpoint, PairingApproval, PeerBrowser, SessionCoordinator, SharedEntry, SharedWorkspace,
    TrustedPeerStore,
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

#[derive(Default)]
struct PairingInner {
    pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
    trusted: Mutex<Vec<String>>,
    active: Mutex<HashSet<String>>,
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
    trusted: bool,
    transport_active: bool,
    detail: String,
}

static NEXT_PAIRING_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

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
                let _ = run_pairing(
                    stream,
                    peer_identity,
                    HandshakeRole::Responder,
                    app_handle,
                    pairing_state,
                    workspace_state,
                    None,
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

async fn run_pairing(
    stream: tokio::net::TcpStream,
    identity: Arc<DeviceIdentity>,
    role: HandshakeRole,
    app: tauri::AppHandle,
    pairing: Arc<PairingInner>,
    workspace: Arc<Mutex<Option<Arc<SharedWorkspace>>>>,
    expected_peer: Option<String>,
) -> Result<PairingResult, String> {
    use tauri::Emitter;
    let completion_app = app.clone();
    let challenge_pairing = pairing.clone();
    let paired = pair_over_stream(stream, &identity, role, move |challenge| {
        let app = app.clone();
        let pairing = challenge_pairing.clone();
        async move {
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
    })
    .await
    .map_err(|error| error.to_string())?;
    let peer_device_id = paired.peer_device().0.clone();
    if expected_peer
        .as_deref()
        .is_some_and(|expected| expected != peer_device_id)
    {
        return Err(
            "authenticated identity differs from the discovered peer; connection closed".into(),
        );
    }
    let (sender, receiver, approval) = paired.into_encrypted();
    let approval = approval.ok_or("pairing approval missing")?;
    persist_approval(&approval)?;
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
        trusted: true,
        transport_active: true,
        detail: if has_workspace {
            "Both users approved; the encrypted transport is active and this device is serving its authorized workspace. Input, display, and live window services are not active yet.".into()
        } else {
            "Both users approved; the encrypted transport is active. Choose a Shared Workspace folder to enable this device's host-side file service. Input, display, and live window services are not active yet.".into()
        },
    };
    let _ = completion_app.emit("pairing-established", &result);
    let peer_for_task = result.peer_device_id.clone();
    let session_pairing = pairing.clone();
    let session_app = completion_app.clone();
    tokio::spawn(async move {
        let _ = serve_file_rpc(sender, receiver, workspace).await;
        if let Ok(mut active) = session_pairing.active.lock() {
            active.remove(&peer_for_task);
        }
        let _ = session_app.emit("pairing-ended", peer_for_task);
    });
    Ok(result)
}

fn persist_approval(approval: &PairingApproval) -> Result<(), String> {
    let store = TrustedPeerStore::for_current_user().map_err(|error| error.to_string())?;
    store
        .add_approved(approval)
        .map_err(|error| error.to_string())?;
    Ok(())
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
        .invoke_handler(tauri::generate_handler![
            get_local_status,
            get_platform_report,
            start_local_network,
            discover_local_peers,
            pair_with_peer,
            respond_to_pairing,
            configure_shared_workspace,
            list_workspace_directory,
            create_workspace_directory
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Unified Workspace desktop shell");
}
