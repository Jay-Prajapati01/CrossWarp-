#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::sync::{Arc, Mutex};
use unified_workspace_agent::protocol::{Capability, CapabilityState, DeviceId, SessionId};
use unified_workspace_agent::{
    DeviceIdentity, HostPlatform, IdentityStore, LanEndpoint, PeerBrowser, SessionCoordinator,
    SharedEntry, SharedWorkspace,
};

#[derive(Default)]
struct WorkspaceState(Mutex<Option<SharedWorkspace>>);

#[derive(Default)]
struct NetworkState(Mutex<NetworkInner>);

#[derive(Default)]
struct NetworkInner {
    identity: Option<Arc<DeviceIdentity>>,
    endpoint: Option<LanEndpoint>,
    browser: Option<PeerBrowser>,
}

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
fn get_local_status(state: tauri::State<'_, WorkspaceState>) -> LocalStatus {
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
        connection: "disconnected",
        implementation: "local status only; pairing and remote services are not active",
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
    state: tauri::State<'_, NetworkState>,
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
    let endpoint = LanEndpoint::bind(
        &identity,
        &std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Windows device".into()),
    )
    .await
    .map_err(|error| error.to_string())?;
    let browser = PeerBrowser::start(identity.device_id()).map_err(|error| error.to_string())?;
    let listen_address = endpoint
        .local_addr()
        .map_err(|error| error.to_string())?
        .to_string();
    let mut inner = state.0.lock().map_err(|_| "network state is unavailable")?;
    inner.endpoint = Some(endpoint);
    inner.browser = Some(browser);
    inner.identity = Some(identity.clone());
    Ok(NetworkStatus {
        device_id: identity.device_id().0,
        listen_address,
        discovery:
            "TCP listener bound; mDNS publish and browse requested (peer presence unverified)"
                .into(),
    })
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
        .map_err(|_| "workspace state is unavailable".to_owned())? = Some(workspace);
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
        .invoke_handler(tauri::generate_handler![
            get_local_status,
            get_platform_report,
            start_local_network,
            discover_local_peers,
            configure_shared_workspace,
            list_workspace_directory,
            create_workspace_directory
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Unified Workspace desktop shell");
}
