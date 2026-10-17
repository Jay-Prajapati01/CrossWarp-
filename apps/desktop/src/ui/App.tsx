import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

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

type WorkspaceEntry = { name: string; relative_path: string; kind: "file" | "directory"; size: number };
type WorkspaceView = { root: string; entries: WorkspaceEntry[] };
type NetworkStatus = { device_id: string; listen_address: string; discovery: string };
type Peer = { device_id: string; addresses: string[] };

const labels: Record<string, { title: string; glyph: string }> = {
  input: { title: "Shared input", glyph: "↔" },
  shared_files: { title: "Shared Workspace", glyph: "▤" },
  extended_display: { title: "Extended display", glyph: "▣" },
  live_window_handoff: { title: "Live Window Handoff", glyph: "▱" },
};

export function App() {
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [workspace, setWorkspace] = useState<WorkspaceView | null>(null);
  const [directory, setDirectory] = useState<string | null>(null);
  const [network, setNetwork] = useState<NetworkStatus | null>(null);
  const [peers, setPeers] = useState<Peer[]>([]);

  useEffect(() => {
    invoke<LocalStatus>("get_local_status")
      .then(setStatus)
      .catch((reason: unknown) => setError(String(reason)));
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
      setError(null);
      for (let attempt = 0; attempt < 10; attempt += 1) {
        await new Promise((resolve) => window.setTimeout(resolve, 300));
        const found = await invoke<Peer[]>("discover_local_peers");
        if (found.length) setPeers((current) => [...new Map([...current, ...found].map((peer) => [peer.device_id, peer])).values()]);
      }
    } catch (reason) { setError(String(reason)); }
  }

  return (
    <main className="shell">
      <aside className="rail" aria-label="Main navigation">
        <div className="brand-mark" aria-hidden="true">U</div>
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
          <div className="breadcrumb"><span>Unified Workspace</span><span className="slash">/</span><strong>Overview</strong></div>
          <div className="top-actions"><span className="status-pill"><i /> Disconnected</span><button className="icon-button" disabled aria-label="Notifications, not available yet">♧</button></div>
        </header>

        <div className="page">
          <div className="heading-row">
            <div>
              <p className="eyebrow">YOUR CROSS-PC SETUP</p>
              <h1>Workspace overview</h1>
              <p className="lede">One session for your screens, input, files, and live windows.</p>
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
            <div className="connection-state"><span className="state-dot" /> Not paired</div>
            <div className="device-divider" />
            <div className="peer-placeholder"><span className="add-peer">＋</span><span><strong>{peers.length ? `${peers.length} peer${peers.length === 1 ? "" : "s"} discovered` : network ? "Listening for peers" : "No peer session"}</strong><small>{network ? `${network.discovery} · ${network.listen_address}` : "Start local discovery to advertise this device and find others."}</small></span></div>
          </section>

          {network && <section className="setup-panel"><div className="setup-symbol">⌁</div><div className="workspace-content"><strong>Device identity and LAN transport</strong><p>Identity is saved with Windows DPAPI. Discovery advertisements are hints only; authenticated pairing and remote capabilities are not connected to this UI yet.</p><div className="workspace-root">Device fingerprint · {network.device_id}</div>{peers.map((peer) => <div className="workspace-root" key={peer.device_id}>Untrusted discovered peer · {peer.device_id} · {peer.addresses.join(", ")}</div>)}</div></section>}

          <div className="section-heading"><div><h2>Connected capabilities</h2><p>Each service will report its own live state.</p></div><span className="all-state">0 of 4 available</span></div>
          <section className="capability-grid" aria-label="Capability status">
            {(status?.capabilities ?? Object.keys(labels).map((capability) => ({ capability, state: "checking", detail: "Loading local status." }))).map((capability) => {
              const label = labels[capability.capability] ?? { title: capability.capability, glyph: "•" };
              return (
                <article className="capability-card" key={capability.capability}>
                  <div className="capability-top"><span className="capability-icon">{label.glyph}</span><span className="state-tag"><i /> Unavailable</span></div>
                  <h3>{label.title}</h3>
                  <p>{capability.detail}</p>
                  <div className="card-footer"><span>Inactive</span><span aria-hidden="true">↗</span></div>
                </article>
              );
            })}
          </section>

          {error && <div className="error-banner" role="alert">Local status could not be loaded: {error}</div>}

          <section className="setup-panel workspace-panel">
            <div className="setup-symbol">✦</div>
            <div className="workspace-content"><strong>Shared Workspace</strong><p>This device's selected folder. Remote access requires a paired connection and is not active yet.</p>
              <div className="workspace-actions"><button onClick={chooseWorkspace}>{workspace ? "Change authorized folder" : "Choose a folder"}</button>{workspace && <button onClick={createFolder}>New folder</button>}</div>
              {workspace && <><div className="workspace-root">Local only · {workspace.root}</div><div className="file-list">
                {directory && <button className="file-row folder-row" onClick={() => openDirectory(directory.includes("/") ? directory.slice(0, directory.lastIndexOf("/")) : null)}>↰ <span>Parent folder</span></button>}
                {workspace.entries.map((entry) => <button className="file-row" key={entry.relative_path} disabled={entry.kind !== "directory"} onClick={() => openDirectory(entry.relative_path)}><span>{entry.kind === "directory" ? "▰" : "▤"} {entry.name}</span><small>{entry.kind === "directory" ? "Folder" : `${entry.size.toLocaleString()} bytes`}</small></button>)}
                {workspace.entries.length === 0 && <span className="empty-files">This folder is empty.</span>}
              </div></>}
            </div>
            <span className="setup-progress">{workspace ? "Local folder authorized" : "Choose storage location"}</span>
          </section>

          <footer><span>Unified Workspace <b>0.1.0 · Development</b></span><span>Local network only <i className="secure-mark">⌑</i></span></footer>
        </div>
      </section>
    </main>
  );
}
