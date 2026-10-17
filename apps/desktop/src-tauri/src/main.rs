#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::Serialize;
use unified_workspace_agent::protocol::{Capability, CapabilityState, DeviceId, SessionId};
use unified_workspace_agent::SessionCoordinator;

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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_local_status])
        .run(tauri::generate_context!())
        .expect("failed to run Unified Workspace desktop shell");
}
