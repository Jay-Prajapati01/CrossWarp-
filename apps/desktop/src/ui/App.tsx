import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";

type Capability = {
  capability: string;
  state: string;
  detail: string;
};

type LocalStatus = {
  device_name: string;
  connection: string;
  implementation: string;
  capabilities: Capability[];
};
type HostPlatform = {
  os_name: string;
  architecture: string;
  version: { major: number; minor: number; build: number } | null;
  meets_iddcx_111_os_floor: boolean;
  meets_wgc_create_for_window_os_floor: boolean;
};

type WorkspaceEntry = { name: string; relative_path: string; kind: "file" | "directory"; size: number };
type WorkspaceView = { root: string; entries: WorkspaceEntry[] };
type NetworkStatus = { device_id: string; listen_address: string; discovery: string };
type Peer = { device_id: string; addresses: string[] };
type PairingChallenge = { request_id: string; local_device: string; peer_device: string; verification_code: string };
type PairingResult = { peer_device_id: string; session_id: string; trusted: boolean; transport_active: boolean; detail: string };
type WindowShareRequest = { request_id: string; peer_device_id: string; stream_id: number; width: number; height: number };
type RemoteWindowFrame = { peer_device_id: string; stream_id: number; sequence: number; timestamp_100ns: number; received_at_unix_ms: number; width: number; height: number; jpeg_base64: string };
type CapturableWindow = { hwnd: number; title: string };
type RemoteTransferProgress = { peer_device_id: string; direction: "upload" | "download"; bytes_sent: number; total_bytes: number; complete: boolean };
type InputDiagnostic = { running: boolean; total: number; pointer_move: number; pointer_button: number; pointer_wheel: number; key_transition: number; error: string | null };
type PortalPosition = { left: number; top: number };

declare global {
  interface Window {
    __CROSSWARP_PORTAL?: { peer_device_id: string; stream_id: number };
  }
}

const labels: Record<string, { title: string; glyph: string }> = {
  input: { title: "Shared input", glyph: "↔" },
  shared_files: { title: "Shared Workspace", glyph: "▤" },
  extended_display: { title: "Extended display", glyph: "▣" },
  live_window_handoff: { title: "Live Window Handoff", glyph: "▱" },
};

export function App() {
  return window.__CROSSWARP_PORTAL ? <NativePortalWindow /> : <Dashboard />;
}

function Dashboard() {
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [platform, setPlatform] = useState<HostPlatform | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [workspace, setWorkspace] = useState<WorkspaceView | null>(null);
  const [directory, setDirectory] = useState<string | null>(null);
  const [network, setNetwork] = useState<NetworkStatus | null>(null);
  const [peers, setPeers] = useState<Peer[]>([]);
  const [trustedPeerIds, setTrustedPeerIds] = useState<string[]>([]);
  const [challenge, setChallenge] = useState<PairingChallenge | null>(null);
  const [windowShareRequest, setWindowShareRequest] = useState<WindowShareRequest | null>(null);
  const [remoteWindowFrame, setRemoteWindowFrame] = useState<RemoteWindowFrame | null>(null);
  const [windowStreamError, setWindowStreamError] = useState<string | null>(null);
  const [capturableWindows, setCapturableWindows] = useState<CapturableWindow[] | null>(null);
  const [windowShareActive, setWindowShareActive] = useState(false);
  const [pairingResult, setPairingResult] = useState<PairingResult | null>(null);
  const [remoteEntries, setRemoteEntries] = useState<WorkspaceEntry[] | null>(null);
  const [remoteDirectory, setRemoteDirectory] = useState<string | null>(null);
  const [transferProgress, setTransferProgress] = useState<RemoteTransferProgress | null>(null);
  const [inputDiagnostic, setInputDiagnostic] = useState<InputDiagnostic | null>(null);
  const [portalPosition, setPortalPosition] = useState<PortalPosition>({ left: 24, top: 120 });
  const lastPortalMove = useRef(0);
  const portalSendQueue = useRef<Promise<void>>(Promise.resolve());
  const portalDrag = useRef<{ pointerId: number; offsetX: number; offsetY: number } | null>(null);
  const heldPortalButtons = useRef(new Map<number, { x: number; y: number }>());
  const heldPortalKeys = useRef(new Set<number>());

  useEffect(() => {
    invoke<LocalStatus>("get_local_status")
      .then(setStatus)
      .catch((reason: unknown) => setError(String(reason)));
    invoke<HostPlatform>("get_platform_report")
      .then(setPlatform)
      .catch((reason: unknown) => setError(String(reason)));
    invoke<string[]>("list_trusted_peers")
      .then(setTrustedPeerIds)
      .catch((reason: unknown) => setError(String(reason)));
    invoke<InputDiagnostic>("get_input_capture_diagnostic")
      .then(setInputDiagnostic)
      .catch((reason: unknown) => setError(String(reason)));
  }, []);

  useEffect(() => {
    if (!inputDiagnostic?.running) return;
    const timer = window.setInterval(() => {
      void invoke<InputDiagnostic>("get_input_capture_diagnostic")
        .then(setInputDiagnostic)
        .catch((reason: unknown) => setError(String(reason)));
    }, 300);
    return () => window.clearInterval(timer);
  }, [inputDiagnostic?.running]);

  useEffect(() => {
    if (!pairingResult?.transport_active) {
      setRemoteEntries(null);
      setRemoteDirectory(null);
      return;
    }
    void refreshRemoteDirectory(null, pairingResult.peer_device_id);
  }, [pairingResult?.peer_device_id, pairingResult?.transport_active]);

  useEffect(() => {
    const peerDeviceId = pairingResult?.transport_active ? pairingResult.peer_device_id : null;
    if (!peerDeviceId) {
      setRemoteWindowFrame(null);
      return;
    }
    let lastFrameKey = "";
    let active = true;
    const poll = async () => {
      try {
        const frame = await invoke<RemoteWindowFrame | null>("get_remote_window_frame", { peerDeviceId });
        const frameKey = frame ? frame.stream_id + ":" + frame.sequence : "";
        if (active && frame && frameKey !== lastFrameKey) {
          lastFrameKey = frameKey;
          setRemoteWindowFrame(frame);
        } else if (active && !frame) {
          setRemoteWindowFrame((current) => current?.peer_device_id === peerDeviceId ? null : current);
        } else if (active && frame && Date.now() - frame.received_at_unix_ms > 1000) {
          setRemoteWindowFrame((current) => current?.peer_device_id === peerDeviceId ? null : current);
        }
      } catch (reason) {
        if (active) setWindowStreamError(String(reason));
      }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 100);
    return () => { active = false; window.clearInterval(timer); };
  }, [pairingResult?.peer_device_id, pairingResult?.transport_active]);

  useEffect(() => {
    let unlisten: (() => void)[] = [];
    void Promise.all([
      listen<PairingChallenge>("pairing-challenge", (event) => setChallenge(event.payload)),
      listen<PairingResult>("pairing-established", (event) => setPairingResult(event.payload)),
      listen<string>("pairing-ended", (event) => {
        setPairingResult((current) => current?.peer_device_id === event.payload ? null : current);
        setWindowShareRequest((current) => current?.peer_device_id === event.payload ? null : current);
        setRemoteWindowFrame((current) => current?.peer_device_id === event.payload ? null : current);
        setWindowShareActive(false);
      }),
      listen<RemoteTransferProgress>("remote-transfer-progress", (event) => setTransferProgress(event.payload)),
      listen<WindowShareRequest>("window-share-request", (event) => setWindowShareRequest(event.payload)),
      listen<string>("window-stream-error", (event) => setWindowStreamError(event.payload)),
      listen<string>("window-share-ended", (event) => {
        setWindowShareActive(false);
        setRemoteWindowFrame((current) => current?.peer_device_id === event.payload ? null : current);
      }),
    ])
      .then((dispose) => { unlisten = dispose; })
      .catch((reason: unknown) => setError(String(reason)));
    return () => unlisten.forEach((dispose) => dispose());
  }, []);

  async function chooseWorkspace() {
    const path = await open({ directory: true, multiple: false, title: "Choose a folder for this device's Shared Workspace" });
    if (!path || Array.isArray(path)) return;
    try {
      const result = await invoke<WorkspaceView>("configure_shared_workspace", { path });
      setWorkspace(result);
      setDirectory(null);
      setStatus(await invoke<LocalStatus>("get_local_status"));
      setError(null);
    } catch (reason) { setError(String(reason)); }
  }

  async function openDirectory(path: string | null) {
    try {
      const entries = await invoke<WorkspaceEntry[]>("list_workspace_directory", { path });
      setWorkspace((current) => current ? { ...current, entries } : current);
      setDirectory(path);
      setError(null);
    } catch (reason) { setError(String(reason)); }
  }

  async function createFolder() {
    const name = window.prompt("New folder name");
    if (!name) return;
    const path = [directory, name].filter(Boolean).join("/");
    try {
      await invoke("create_workspace_directory", { path });
      await openDirectory(directory);
    } catch (reason) { setError(String(reason)); }
  }

  async function startNetwork() {
    try {
      setNetwork(await invoke<NetworkStatus>("start_local_network"));
      setTrustedPeerIds(await invoke<string[]>("list_trusted_peers"));
      setError(null);
      for (let attempt = 0; attempt < 10; attempt += 1) {
        await new Promise((resolve) => window.setTimeout(resolve, 300));
        const found = await invoke<Peer[]>("discover_local_peers");
        if (found.length) setPeers((current) => [...new Map([...current, ...found].map((peer) => [peer.device_id, peer])).values()]);
      }
    } catch (reason) { setError(String(reason)); }
  }

  async function toggleInputCaptureDiagnostic() {
    try {
      if (inputDiagnostic?.running) {
        setInputDiagnostic(await invoke<InputDiagnostic>("stop_input_capture_diagnostic"));
        return;
      }
      const approved = window.confirm(
        "Start a local-only Raw Input diagnostic? While active, CrossWarp can observe system mouse and keyboard events and count their types. Input still reaches local apps. Event data is briefly buffered in memory for counting, but is not written to disk or forwarded to a peer. Avoid typing passwords or sensitive text until you stop it.",
      );
      if (!approved) return;
      setInputDiagnostic(await invoke<InputDiagnostic>("start_input_capture_diagnostic"));
      setError(null);
    } catch (reason) { setError(String(reason)); }
  }

  async function pairWith(peer: Peer, address: string) {
    try {
      setPairingResult(null);
      const reconnect = trustedPeerIds.includes(peer.device_id);
      const command = reconnect ? "reconnect_trusted_peer" : "pair_with_peer";
      const result = await invoke<PairingResult>(command, { peer: { device_id: peer.device_id, address } });
      setPairingResult(result);
      setTrustedPeerIds(await invoke<string[]>("list_trusted_peers"));
      setError(null);
    } catch (reason) { setError(String(reason)); }
  }

  async function respondToPairing(approved: boolean) {
    if (!challenge) return;
    try {
      await invoke("respond_to_pairing", { requestId: challenge.request_id, approved });
      setChallenge(null);
    } catch (reason) { setError(String(reason)); setChallenge(null); }
  }

  async function respondToWindowShare(approved: boolean) {
    if (!windowShareRequest) return;
    try {
      await invoke("respond_to_window_share", { requestId: windowShareRequest.request_id, approved });
      setWindowShareRequest(null);
      if (!approved) setRemoteWindowFrame(null);
      setWindowStreamError(approved ? null : "Live window frames were declined.");
    } catch (reason) { setError(String(reason)); setWindowShareRequest(null); }
  }

  async function refreshCapturableWindows() {
    try {
      setCapturableWindows(await invoke<CapturableWindow[]>("list_capturable_windows"));
      setWindowStreamError(null);
    } catch (reason) { setWindowStreamError(String(reason)); }
  }

  async function startWindowShare(windowInfo: CapturableWindow) {
    if (!pairingResult?.transport_active) return;
    const approved = window.confirm(
      "Share the visible window \"" + windowInfo.title + "\" with " + pairingResult.peer_device_id +
      "? Its pixels will be captured and sent over the encrypted session. If the other PC approves, pointer and keyboard events entered in the proxy will be forwarded to this window. Windows security boundaries remain in force. Continue?",
    );
    if (!approved) return;
    try {
      await invoke("start_window_share", {
        peerDeviceId: pairingResult.peer_device_id,
        hwnd: windowInfo.hwnd,
      });
      setWindowShareActive(true);
      setWindowStreamError(null);
    } catch (reason) {
      setWindowShareActive(false);
      setWindowStreamError(String(reason));
    }
  }

  async function stopWindowShare() {
    if (!pairingResult?.peer_device_id) return;
    try {
      await invoke("stop_window_share", { peerDeviceId: pairingResult.peer_device_id });
      setWindowShareActive(false);
    } catch (reason) { setWindowStreamError(String(reason)); }
  }

  async function revokeRemoteWindowPreview() {
    if (!pairingResult?.peer_device_id) return;
    try {
      await invoke("revoke_remote_window_preview", { peerDeviceId: pairingResult.peer_device_id });
      setRemoteWindowFrame(null);
      setWindowStreamError(null);
    } catch (reason) { setWindowStreamError(String(reason)); }
  }

  function portalPoint(event: React.PointerEvent<HTMLImageElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    return { x: Math.max(0, Math.min(65535, Math.round((event.clientX - rect.left) * 65535 / rect.width))), y: Math.max(0, Math.min(65535, Math.round((event.clientY - rect.top) * 65535 / rect.height))) };
  }

  async function sendPortalInput(event: object) {
    if (!pairingResult || !remoteWindowFrame) return;
    const peerDeviceId = pairingResult.peer_device_id;
    const sourceStreamId = remoteWindowFrame.stream_id;
    portalSendQueue.current = portalSendQueue.current.then(async () => {
      await invoke("send_window_portal_input", { peerDeviceId, sourceStreamId, event });
    }).catch((reason: unknown) => { setWindowStreamError(String(reason)); });
    await portalSendQueue.current;
  }

  async function openNativePortal() {
    if (!pairingResult || !remoteWindowFrame) return;
    try {
      await invoke("open_native_window_portal", { peerDeviceId: pairingResult.peer_device_id, streamId: remoteWindowFrame.stream_id });
      setWindowStreamError(null);
    } catch (reason) { setWindowStreamError(String(reason)); }
  }

  function beginPortalDrag(event: React.PointerEvent<HTMLDivElement>) {
    if ((event.target as HTMLElement).closest("button")) return;
    portalDrag.current = { pointerId: event.pointerId, offsetX: event.clientX - portalPosition.left, offsetY: event.clientY - portalPosition.top };
    event.currentTarget.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function movePortal(event: React.PointerEvent<HTMLDivElement>) {
    const drag = portalDrag.current;
    if (!drag || drag.pointerId !== event.pointerId) return;
    setPortalPosition({
      left: Math.max(0, Math.min(window.innerWidth - 280, event.clientX - drag.offsetX)),
      top: Math.max(0, Math.min(window.innerHeight - 120, event.clientY - drag.offsetY)),
    });
  }

  function releasePortalInputs() {
    for (const [button, point] of heldPortalButtons.current) void sendPortalInput({ kind: "pointer_button", ...point, button, down: false });
    for (const virtual_key of heldPortalKeys.current) void sendPortalInput({ kind: "key", virtual_key, scan_code: 0, extended: false, down: false });
    heldPortalButtons.current.clear();
    heldPortalKeys.current.clear();
  }

  async function disconnectPeer() {
    if (!pairingResult) return;
    try {
      await invoke("disconnect_peer", { peerDeviceId: pairingResult.peer_device_id });
      setPairingResult(null);
      setRemoteEntries(null);
      setRemoteDirectory(null);
      setTransferProgress(null);
      setError(null);
    } catch (reason) { setError(String(reason)); }
  }

  async function refreshRemoteDirectory(path: string | null, peerDeviceId = pairingResult?.peer_device_id) {
    if (!peerDeviceId) return;
    try {
      const entries = await invoke<WorkspaceEntry[]>("list_remote_workspace", { peerDeviceId, directory: path });
      setRemoteEntries(entries);
      setRemoteDirectory(path);
      setError(null);
    } catch (reason) {
      setRemoteEntries(null);
      setError(String(reason));
    }
  }

  async function createRemoteFolder() {
    if (!pairingResult) return;
    const name = window.prompt("New folder name on the peer Shared Workspace");
    if (!name) return;
    const path = [remoteDirectory, name].filter(Boolean).join("/");
    try {
      await invoke("create_remote_directory", { peerDeviceId: pairingResult.peer_device_id, path });
      await refreshRemoteDirectory(remoteDirectory);
    } catch (reason) { setError(String(reason)); }
  }

  async function uploadRemoteFile() {
    if (!pairingResult) return;
    const sourcePath = await open({ multiple: false, directory: false, title: "Choose a file to copy to the peer" });
    if (!sourcePath || Array.isArray(sourcePath)) return;
    const fileName = sourcePath.split(/[\\/]/).at(-1);
    if (!fileName) return;
    const existing = remoteEntries?.find((entry) => entry.name === fileName);
    if (existing?.kind === "directory") {
      setError(`A folder named ${fileName} already exists in the peer workspace.`);
      return;
    }
    const replaceExisting = existing?.kind === "file";
    if (replaceExisting && !window.confirm(`Replace the existing peer file ${fileName}? The new file will appear only after its upload and SHA-256 check succeed.`)) return;
    const destinationPath = [remoteDirectory, fileName].filter(Boolean).join("/");
    try {
      setTransferProgress({ peer_device_id: pairingResult.peer_device_id, direction: "upload", bytes_sent: 0, total_bytes: 0, complete: false });
      await invoke("upload_remote_file", { peerDeviceId: pairingResult.peer_device_id, destinationPath, sourcePath, replaceExisting });
      await refreshRemoteDirectory(remoteDirectory);
      setError(null);
      setTransferProgress(null);
    } catch (reason) { setTransferProgress(null); setError(String(reason)); }
  }

  async function renameRemoteEntry(entry: WorkspaceEntry) {
    if (!pairingResult) return;
    const nextName = window.prompt("Rename peer workspace entry", entry.name);
    if (!nextName || nextName === entry.name) return;
    if (!window.confirm(`Rename ${entry.name} to ${nextName} on the peer?`)) return;
    const parent = entry.relative_path.includes("/") ? entry.relative_path.slice(0, entry.relative_path.lastIndexOf("/")) : null;
    const destination = [parent, nextName].filter(Boolean).join("/");
    try {
      await invoke("rename_remote_entry", { peerDeviceId: pairingResult.peer_device_id, source: entry.relative_path, destination, confirmed: true });
      await refreshRemoteDirectory(remoteDirectory);
      setError(null);
    } catch (reason) { setError(String(reason)); }
  }

  async function deleteRemoteFile(entry: WorkspaceEntry) {
    if (!pairingResult || !window.confirm(`Permanently delete ${entry.name} from the peer workspace?`)) return;
    try {
      await invoke("delete_remote_file", { peerDeviceId: pairingResult.peer_device_id, path: entry.relative_path, confirmed: true });
      await refreshRemoteDirectory(remoteDirectory);
      setError(null);
    } catch (reason) { setError(String(reason)); }
  }

  async function downloadRemoteFile(entry: WorkspaceEntry) {
    if (!pairingResult) return;
    const destinationPath = await save({ title: "Save file from peer", defaultPath: entry.name });
    if (!destinationPath) return;
    try {
      setTransferProgress({ peer_device_id: pairingResult.peer_device_id, direction: "download", bytes_sent: 0, total_bytes: entry.size, complete: false });
      await invoke("download_remote_file_to_path", {
        peerDeviceId: pairingResult.peer_device_id,
        path: entry.relative_path,
        expectedSize: entry.size,
        destinationPath,
      });
      setTransferProgress(null);
      setError(null);
    } catch (reason) { setTransferProgress(null); setError(String(reason)); }
  }

  const availableCapabilityCount = (status?.capabilities ?? []).filter((capability) => {
    if (capability.capability === "shared_files") {
      return Boolean(pairingResult?.transport_active && remoteEntries !== null);
    }
    return capability.state === "available" || capability.state === "active";
  }).length;

  return (
    <main className="shell">
      <aside className="rail" aria-label="Main navigation">
        <div className="brand-mark"><img src="/crosswarp-logo.svg" alt="CrossWarp" /></div>
        <nav>
          <a className="nav-link active" href="#workspace" aria-current="page" title="Workspace">⌂</a>
          <button className="nav-link" disabled aria-label="Shared files, not available yet" title="Shared files are not available yet">▤</button>
          <button className="nav-link" disabled aria-label="Windows, not available yet" title="Window navigation is not available yet">▱</button>
          <button className="nav-link" disabled aria-label="Settings, not available yet" title="Settings are not available yet">⚙</button>
        </nav>
        <div className="rail-bottom"><span className="avatar">J</span></div>
      </aside>

      <section className="content" id="workspace">
        <header className="topbar">
          <div className="breadcrumb"><span>CrossWarp</span><span className="slash">/</span><strong>Overview</strong></div>
          <div className="top-actions"><span className="status-pill"><i /> {pairingResult?.transport_active ? "Encrypted transport active" : "Disconnected"}</span><button className="icon-button" disabled aria-label="Notifications, not available yet">♧</button></div>
        </header>

        <div className="page">
          <div className="heading-row">
            <div>
              <p className="eyebrow">CROSSWARP · YOUR CROSS-PC SETUP</p>
              <h1>Jump between PCs without breaking flow</h1>
              <p className="lede">One connected workspace for your screens, input, files, and live windows.</p>
            </div>
            <button className="primary-button" onClick={startNetwork}>{network ? "Refresh discovery" : "Start local discovery"} <span>→</span></button>
          </div>

          <section className="device-card" aria-label="Device connection">
            <div className="device-icon">▣</div>
            <div className="device-copy">
              <span className="card-label">THIS DEVICE</span>
              <strong>{status?.device_name ?? "Reading device…"}</strong>
              <span className="muted">{status?.implementation ?? "Checking local status"}</span>
            </div>
            <div className="connection-state"><span className="state-dot" /> {pairingResult?.transport_active ? "Transport active" : "Not paired"}</div>
            <div className="device-divider" />
            <div className="peer-placeholder"><span className="add-peer">＋</span><span><strong>{peers.length ? `${peers.length} peer${peers.length === 1 ? "" : "s"} discovered` : network ? "Listening for peers" : "No peer session"}</strong><small>{network ? `${network.discovery} · ${network.listen_address}` : "Start local discovery to advertise this device and find others."}</small></span></div>
          </section>

          {peers.length > 0 && <section className="setup-panel"><div className="workspace-content"><strong>Discovered devices</strong><p>New peers require both users to approve the same six-digit code. Previously approved peers reconnect using their stored device identity.</p>{peers.map((peer) => { const trusted = trustedPeerIds.includes(peer.device_id); return <div className="peer-row" key={peer.device_id}><span>{peer.device_id} · {peer.addresses.join(", ")}</span><button onClick={() => peer.addresses[0] && pairWith(peer, peer.addresses[0])}>{trusted ? "Reconnect" : "Request pairing"}</button></div>; })}</div></section>}
          {pairingResult && <section className="setup-panel"><div className="workspace-content"><strong>Trusted peer · encrypted transport active</strong><p>{pairingResult.detail}</p><div className="workspace-root">Authenticated peer · {pairingResult.peer_device_id}</div><div className="workspace-root">Session · {pairingResult.session_id}</div><div className="workspace-actions"><button onClick={() => void disconnectPeer()}>Disconnect peer</button></div></div></section>}

          {windowShareRequest && <section className="setup-panel" role="alert"><div className="setup-symbol">▱</div><div className="workspace-content"><strong>Approve live window and interactive control?</strong><p>{windowShareRequest.peer_device_id} is requesting permission to show stream {windowShareRequest.stream_id}, sized {windowShareRequest.width} × {windowShareRequest.height}. Frames and forwarded pointer, wheel, and keyboard input remain disabled until approved. Input is limited to the selected visible window and Windows UIPI restrictions still apply.</p><div className="workspace-actions"><button onClick={() => void respondToWindowShare(true)}>Approve live control</button><button onClick={() => void respondToWindowShare(false)}>Decline</button></div></div></section>}

          {pairingResult?.transport_active && <section className="setup-panel workspace-panel"><div className="setup-symbol">▱</div><div className="workspace-content"><strong>Live Window Handoff · interactive proxy</strong><p>The approved live window remains on its source PC. The proxy can be moved and resized in this dashboard. Click its image to focus it; pointer, wheel, and keyboard input are forwarded to that visible source window. Windows security boundaries still apply.</p>{windowStreamError && <div className="workspace-root" role="status">{windowStreamError}</div>}{remoteWindowFrame?.peer_device_id === pairingResult.peer_device_id && <div className="remote-window-preview portal-floating" style={{ left: portalPosition.left, top: portalPosition.top }}><div className="portal-titlebar" onPointerDown={beginPortalDrag} onPointerMove={movePortal} onPointerUp={() => { portalDrag.current = null; }} onPointerCancel={() => { portalDrag.current = null; }}><span>Live proxy · {remoteWindowFrame.width} × {remoteWindowFrame.height} · frame {remoteWindowFrame.sequence}</span><button onClick={() => setPortalPosition({ left: 24, top: 120 })}>Reset position</button></div><img tabIndex={0} onPointerMove={(event) => { const now = performance.now(); if (now - lastPortalMove.current >= 32) { lastPortalMove.current = now; const point = portalPoint(event); void sendPortalInput({ kind: "pointer_move", ...point }); } }} onPointerDown={(event) => { event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId); const point = portalPoint(event); const button = event.button === 1 ? 3 : event.button === 2 ? 2 : event.button + 1; heldPortalButtons.current.set(button, point); void sendPortalInput({ kind: "pointer_button", ...point, button, down: true }); }} onPointerUp={(event) => { const point = portalPoint(event); const button = event.button === 1 ? 3 : event.button === 2 ? 2 : event.button + 1; heldPortalButtons.current.delete(button); void sendPortalInput({ kind: "pointer_button", ...point, button, down: false }); }} onPointerCancel={() => { for (const [button, point] of heldPortalButtons.current) void sendPortalInput({ kind: "pointer_button", ...point, button, down: false }); heldPortalButtons.current.clear(); }} onBlur={releasePortalInputs} onWheel={(event) => { event.preventDefault(); const rect = event.currentTarget.getBoundingClientRect(); const point = { x: Math.max(0, Math.min(65535, Math.round((event.clientX - rect.left) * 65535 / rect.width))), y: Math.max(0, Math.min(65535, Math.round((event.clientY - rect.top) * 65535 / rect.height))) }; void sendPortalInput({ kind: "pointer_wheel", ...point, delta: -Math.sign(event.deltaY) * 120, horizontal: Math.abs(event.deltaX) > Math.abs(event.deltaY) }); }} onKeyDown={(event) => { if (event.repeat) return; event.preventDefault(); heldPortalKeys.current.add(event.nativeEvent.keyCode); void sendPortalInput({ kind: "key", virtual_key: event.nativeEvent.keyCode, scan_code: 0, extended: false, down: true }); }} onKeyUp={(event) => { event.preventDefault(); heldPortalKeys.current.delete(event.nativeEvent.keyCode); void sendPortalInput({ kind: "key", virtual_key: event.nativeEvent.keyCode, scan_code: 0, extended: false, down: false }); }} onContextMenu={(event) => event.preventDefault()} src={"data:image/jpeg;base64," + remoteWindowFrame.jpeg_base64} width={remoteWindowFrame.width} height={remoteWindowFrame.height} alt="Approved interactive live remote window proxy" /><div className="portal-actions"><button onClick={() => void openNativePortal()}>Open as a native desktop window</button><button onClick={() => void revokeRemoteWindowPreview()}>Revoke proxy and stop remote capture</button></div></div>}</div></section>}

          {pairingResult?.transport_active && <section className="setup-panel workspace-panel"><div className="setup-symbol">▱</div><div className="workspace-content"><strong>Present a live window</strong><p>Choose a visible window from this PC. Your confirmation permits capture and input in this selected window; the other PC must separately approve before pixels or remote controls are enabled. This does not transfer the running process.</p><div className="workspace-actions"><button onClick={() => void refreshCapturableWindows()}>Find local windows</button>{windowShareActive && <button onClick={() => void stopWindowShare()}>Stop sharing</button>}</div>{capturableWindows?.map((windowInfo) => <div className="peer-row" key={windowInfo.hwnd}><span>{windowInfo.title}</span><button disabled={windowShareActive} onClick={() => void startWindowShare(windowInfo)}>Present live</button></div>)}</div></section>}

          {pairingResult?.transport_active && <section className="setup-panel workspace-panel"><div className="setup-symbol">⇄</div><div className="workspace-content"><strong>Peer Shared Workspace</strong><p>Contents are requested over the authenticated encrypted connection. The peer must grant a folder before it appears here.</p><div className="workspace-actions"><button onClick={() => void refreshRemoteDirectory(remoteDirectory)}>Refresh peer folder</button><button onClick={() => void createRemoteFolder()}>New peer folder</button><button disabled={Boolean(transferProgress && !transferProgress.complete)} onClick={() => void uploadRemoteFile()}>Upload file</button></div>{transferProgress?.peer_device_id === pairingResult.peer_device_id && <div className="workspace-root" role="status">{transferProgress.complete ? (transferProgress.direction === "upload" ? "Upload verified" : "Download saved") : (transferProgress.direction === "upload" ? "Uploading" : "Downloading")} · {transferProgress.bytes_sent.toLocaleString()} / {transferProgress.total_bytes.toLocaleString()} bytes{transferProgress.total_bytes > 0 ? ` · ${Math.floor(transferProgress.bytes_sent * 100 / transferProgress.total_bytes)}%` : ""}</div>}{remoteEntries && <><div className="workspace-root">Remote folder · {remoteDirectory || "/"}</div><div className="file-list">{remoteDirectory && <button className="file-row folder-row" onClick={() => void refreshRemoteDirectory(remoteDirectory.includes("/") ? remoteDirectory.slice(0, remoteDirectory.lastIndexOf("/")) : null)}>↰ <span>Parent folder</span></button>}{remoteEntries.map((entry) => <div className="remote-entry" key={entry.relative_path}><button className="file-row" onClick={() => entry.kind === "directory" ? void refreshRemoteDirectory(entry.relative_path) : void downloadRemoteFile(entry)}><span>{entry.kind === "directory" ? "▰" : "▤"} {entry.name}</span><small>{entry.kind === "directory" ? "Folder" : `${entry.size.toLocaleString()} bytes · download`}</small></button><div className="remote-entry-actions"><button onClick={() => void renameRemoteEntry(entry)}>Rename</button>{entry.kind === "file" && <button onClick={() => void deleteRemoteFile(entry)}>Delete</button>}</div></div>)}{remoteEntries.length === 0 && <span className="empty-files">Peer folder is empty.</span>}</div></>}</div></section>}

          {platform && <section className="setup-panel"><div className="setup-symbol">⌘</div><div className="workspace-content"><strong>Windows compatibility diagnostics · OS floors only</strong><p>{platform.os_name}{platform.version ? ` · build ${platform.version.build}` : ""} · {platform.architecture}</p><div className="workspace-root">IDD 1.11 documented OS floor: {platform.meets_iddcx_111_os_floor ? "met" : "not met"} · HWND capture API floor (Windows 10 1903): {platform.meets_wgc_create_for_window_os_floor ? "met" : "not met"}</div><p>These checks report documented OS build boundaries only. They do not confirm an installed driver, capture permission, hardware support, or complete feature compatibility.</p></div></section>}

          <section className="setup-panel">
            <div className="setup-symbol">⌁</div>
            <div className="workspace-content">
              <strong>Local input capture diagnostic · not input sharing</strong>
              <p>Raw Input observes system keyboard and mouse events while running. This diagnostic briefly buffers events in memory to count their types; it does not suppress local input, route events to a peer, or write event contents to disk. Avoid entering sensitive text while it runs.</p>
              <div className="workspace-actions"><button onClick={() => void toggleInputCaptureDiagnostic()}>{inputDiagnostic?.running ? "Stop diagnostic" : "Start local diagnostic"}</button></div>
              {inputDiagnostic && <div className="workspace-root" role="status">{inputDiagnostic.running ? "Capture running" : "Capture stopped"} · {inputDiagnostic.total} events · {inputDiagnostic.pointer_move} pointer moves · {inputDiagnostic.pointer_button} button events · {inputDiagnostic.pointer_wheel} wheel events · {inputDiagnostic.key_transition} key transitions{inputDiagnostic.error ? ` · ${inputDiagnostic.error}` : ""}</div>}
            </div>
          </section>

          {network && <section className="setup-panel"><div className="setup-symbol">⌁</div><div className="workspace-content"><strong>Device identity and LAN transport</strong><p>Identity is saved with Windows DPAPI. Discovery advertisements are untrusted hints; pairing uses two-sided code approval. Shared Workspace RPC is available on an approved encrypted session.</p><div className="workspace-root">Device fingerprint · {network.device_id}</div>{peers.map((peer) => <div className="workspace-root" key={peer.device_id}>Untrusted discovered peer · {peer.device_id} · {peer.addresses.join(", ")}</div>)}</div></section>}

          <div className="section-heading"><div><h2>Connected capabilities</h2><p>Service state is based on the current peer and local service report.</p></div><span className="all-state">{availableCapabilityCount} of 4 available</span></div>
          <section className="capability-grid" aria-label="Capability status">
            {(status?.capabilities ?? Object.keys(labels).map((capability) => ({ capability, state: "checking", detail: "Loading local status." }))).map((capability) => {
              const label = labels[capability.capability] ?? { title: capability.capability, glyph: "•" };
              const filesReady = capability.capability === "shared_files" && Boolean(pairingResult?.transport_active && remoteEntries);
              const detail = filesReady ? "Peer folder is responding over authenticated RPC; browse, upload, and download are available." : capability.detail;
              return (
                <article className="capability-card" key={capability.capability}>
                  <div className="capability-top"><span className="capability-icon">{label.glyph}</span><span className="state-tag"><i /> {filesReady ? "Available" : "Unavailable"}</span></div>
                  <h3>{label.title}</h3>
                  <p>{detail}</p>
                  <div className="card-footer"><span>{filesReady ? "Active on this session" : "Inactive"}</span><span aria-hidden="true">↗</span></div>
                </article>
              );
            })}
          </section>

          {error && <div className="error-banner" role="alert">{error}</div>}

          {challenge && <div className="pairing-overlay"><section className="pairing-dialog" role="alertdialog" aria-modal="true" aria-labelledby="pairing-title"><p className="eyebrow">TWO-SIDED DEVICE APPROVAL</p><h2 id="pairing-title">Confirm this peer</h2><p>Compare this code with the other computer. Approve only if both screens show the same code.</p><div className="pairing-code">{challenge.verification_code}</div><div className="workspace-root">Peer fingerprint · {challenge.peer_device}</div><div className="workspace-actions"><button onClick={() => void respondToPairing(true)}>Approve pairing</button><button onClick={() => void respondToPairing(false)}>Reject</button></div></section></div>}

          <section className="setup-panel workspace-panel">
            <div className="setup-symbol">✦</div>
            <div className="workspace-content"><strong>Shared Workspace</strong><p>This device's selected folder is offered to a peer only while an approved encrypted session is active.</p>
              <div className="workspace-actions"><button onClick={chooseWorkspace}>{workspace ? "Change authorized folder" : "Choose a folder"}</button>{workspace && <button onClick={createFolder}>New folder</button>}</div>
              {workspace && <><div className="workspace-root">Authorized host folder · {workspace.root}</div><div className="file-list">
                {directory && <button className="file-row folder-row" onClick={() => openDirectory(directory.includes("/") ? directory.slice(0, directory.lastIndexOf("/")) : null)}>↰ <span>Parent folder</span></button>}
                {workspace.entries.map((entry) => <button className="file-row" key={entry.relative_path} disabled={entry.kind !== "directory"} onClick={() => openDirectory(entry.relative_path)}><span>{entry.kind === "directory" ? "▰" : "▤"} {entry.name}</span><small>{entry.kind === "directory" ? "Folder" : `${entry.size.toLocaleString()} bytes`}</small></button>)}
                {workspace.entries.length === 0 && <span className="empty-files">This folder is empty.</span>}
              </div></>}
            </div>
            <span className="setup-progress">{workspace ? "Local folder authorized" : "Choose storage location"}</span>
          </section>

          <footer><span>CrossWarp <b>0.1.0 · Development</b></span><span>Local network only <i className="secure-mark">⌑</i></span></footer>
        </div>
      </section>
    </main>
  );
}

function NativePortalWindow() {
  const config = window.__CROSSWARP_PORTAL!;
  const [frame, setFrame] = useState<RemoteWindowFrame | null>(null);
  const [error, setError] = useState<string | null>(null);
  const lastMove = useRef(0);
  const queue = useRef<Promise<void>>(Promise.resolve());
  const heldButtons = useRef(new Map<number, { x: number; y: number }>());
  const heldKeys = useRef(new Set<number>());

  useEffect(() => {
    document.body.classList.add("portal-body");
    return () => document.body.classList.remove("portal-body");
  }, []);

  useEffect(() => {
    let active = true;
    let querying = false;
    const poll = async () => {
      if (querying) return;
      querying = true;
      try {
        const latest = await invoke<RemoteWindowFrame | null>("get_remote_window_frame", { peerDeviceId: config.peer_device_id });
        if (!active) return;
        if (!latest || latest.stream_id !== config.stream_id || Date.now() - latest.received_at_unix_ms > 1500) setFrame(null);
        else setFrame((current) => current?.sequence === latest.sequence ? current : latest);
      } catch (reason) { if (active) setError(String(reason)); }
      finally { querying = false; }
    };
    void poll();
    const timer = window.setInterval(() => void poll(), 50);
    return () => { active = false; window.clearInterval(timer); };
  }, [config.peer_device_id, config.stream_id]);

  function forward(event: object) {
    const peerDeviceId = config.peer_device_id;
    const sourceStreamId = config.stream_id;
    queue.current = queue.current.then(() => invoke("send_window_portal_input", { peerDeviceId, sourceStreamId, event })).then(() => undefined).catch((reason: unknown) => setError(String(reason)));
  }

  function releaseHeld() {
    for (const [button, point] of heldButtons.current) forward({ kind: "pointer_button", ...point, button, down: false });
    for (const virtual_key of heldKeys.current) forward({ kind: "key", virtual_key, scan_code: 0, extended: false, down: false });
    heldButtons.current.clear();
    heldKeys.current.clear();
  }

  function point(event: React.PointerEvent<HTMLImageElement> | React.WheelEvent<HTMLImageElement>) {
    const rect = event.currentTarget.getBoundingClientRect();
    const scale = Math.min(rect.width / event.currentTarget.naturalWidth, rect.height / event.currentTarget.naturalHeight);
    const width = event.currentTarget.naturalWidth * scale;
    const height = event.currentTarget.naturalHeight * scale;
    const x = event.clientX - rect.left - (rect.width - width) / 2;
    const y = event.clientY - rect.top - (rect.height - height) / 2;
    if (x < 0 || y < 0 || x > width || y > height) return null;
    return { x: Math.max(0, Math.min(65535, Math.round(x * 65535 / width))), y: Math.max(0, Math.min(65535, Math.round(y * 65535 / height))) };
  }

  return <main className="native-portal"><header><span>Live window · {config.peer_device_id.slice(0, 12)}</span><span>Origin app remains on its PC</span></header>{error && <div className="portal-error" role="status">{error}</div>}{frame ? <img tabIndex={0} src={"data:image/jpeg;base64," + frame.jpeg_base64} width={frame.width} height={frame.height} alt="Interactive remote window" onPointerMove={(event) => { const now = performance.now(); if (now - lastMove.current < 32) return; lastMove.current = now; const position = point(event); if (position) forward({ kind: "pointer_move", ...position }); }} onPointerDown={(event) => { event.currentTarget.focus(); event.currentTarget.setPointerCapture(event.pointerId); const button = event.button === 1 ? 3 : event.button === 2 ? 2 : event.button + 1; const position = point(event); if (!position) return; heldButtons.current.set(button, position); forward({ kind: "pointer_button", ...position, button, down: true }); }} onPointerUp={(event) => { const button = event.button === 1 ? 3 : event.button === 2 ? 2 : event.button + 1; const position = point(event) ?? heldButtons.current.get(button); if (!position || !heldButtons.current.has(button)) return; heldButtons.current.delete(button); forward({ kind: "pointer_button", ...position, button, down: false }); }} onPointerCancel={releaseHeld} onBlur={releaseHeld} onWheel={(event) => { event.preventDefault(); const position = point(event); if (position) forward({ kind: "pointer_wheel", ...position, delta: -Math.sign(event.deltaY) * 120, horizontal: Math.abs(event.deltaX) > Math.abs(event.deltaY) }); }} onKeyDown={(event) => { if (event.repeat) return; event.preventDefault(); heldKeys.current.add(event.nativeEvent.keyCode); forward({ kind: "key", virtual_key: event.nativeEvent.keyCode, scan_code: 0, extended: false, down: true }); }} onKeyUp={(event) => { event.preventDefault(); heldKeys.current.delete(event.nativeEvent.keyCode); forward({ kind: "key", virtual_key: event.nativeEvent.keyCode, scan_code: 0, extended: false, down: false }); }} onContextMenu={(event) => event.preventDefault()} /> : <div className="portal-paused">Waiting for the approved live window stream…</div>}</main>;
}
