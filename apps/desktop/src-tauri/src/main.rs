#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use std::sync::Mutex;
use unified_workspace_agent::protocol::{Capability, CapabilityState, DeviceId, SessionId};
use unified_workspace_agent::{SessionCoordinator, SharedEntry, SharedWorkspace};

#[derive(Default)]
struct WorkspaceState(Mutex<Option<SharedWorkspace>>);

#[derive(Serialize)]
struct CapabilityReport {
    capability: &'static str,
    state: &'static str,
    detail: &'static str,
}

#[derive(Serialize)]
struct LocalStatus {
    device_name: String,
    connection: &'static str,
    implementation: &'static str,
    capabilities: Vec<CapabilityReport>,
}

#[tauri::command]
fn get_local_status() -> LocalStatus {
    let mut session = SessionCoordinator::new(
        SessionId::new(),
        DeviceId(std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Unknown device".into())),
        DeviceId("unpaired".into()),
    );
    let reports = [
        (Capability::Input, "input", "Input routing is not connected."),
        (Capability::Files, "shared_files", "No authorized Shared Workspace is configured."),
        (Capability::ExtendedDisplay, "extended_display", "The IddCx driver and stream are not available."),
        (Capability::WindowPortal, "live_window_handoff", "No capture or remote window transport is active."),
    ];
    let capabilities = reports
        .into_iter()
        .map(|(capability, name, detail)| {
            let _ = session.set_capability(
                capability,
                CapabilityState::Blocked { reason: detail.to_owned() },
            );
            CapabilityReport { capability: name, state: "unavailable", detail }
        })
        .collect();
    LocalStatus {
        device_name: std::env::var("COMPUTERNAME").unwrap_or_else(|_| "Unknown device".into()),
        connection: "disconnected",
        implementation: "local status only; pairing and remote services are not active",
        capabilities,
    }
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
    *state.0.lock().map_err(|_| "workspace state is unavailable".to_owned())? =
        Some(workspace);
    Ok(WorkspaceView { root, entries })
}

#[tauri::command]
fn list_workspace_directory(
    path: Option<String>,
    state: tauri::State<'_, WorkspaceState>,
) -> Result<Vec<SharedEntry>, String> {
    let guard = state.0.lock().map_err(|_| "workspace state is unavailable".to_owned())?;
    let workspace = guard.as_ref().ok_or_else(|| "select a Shared Workspace folder first".to_owned())?;
    workspace.list_directory(path.as_deref(), 10_000).map_err(|error| error.to_string())
}

#[tauri::command]
fn create_workspace_directory(
    path: String,
    state: tauri::State<'_, WorkspaceState>,
) -> Result<(), String> {
    let guard = state.0.lock().map_err(|_| "workspace state is unavailable".to_owned())?;
    let workspace = guard.as_ref().ok_or_else(|| "select a Shared Workspace folder first".to_owned())?;
    workspace.create_directory(&path).map_err(|error| error.to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(WorkspaceState::default())
        .invoke_handler(tauri::generate_handler![
            get_local_status,
            configure_shared_workspace,
            list_workspace_directory,
            create_workspace_directory
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Unified Workspace desktop shell");
}
