import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

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

const labels: Record<string, { title: string; glyph: string }> = {
  input: { title: "Shared input", glyph: "↔" },
  shared_files: { title: "Shared Workspace", glyph: "▤" },
  extended_display: { title: "Extended display", glyph: "▣" },
  live_window_handoff: { title: "Live Window Handoff", glyph: "▱" },
};

export function App() {
  const [status, setStatus] = useState<LocalStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<LocalStatus>("get_local_status")
      .then(setStatus)
      .catch((reason: unknown) => setError(String(reason)));
  }, []);

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
            <button className="primary-button" disabled title="Pairing transport is not implemented yet">Connect workspace <span>→</span></button>
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
            <div className="peer-placeholder"><span className="add-peer">＋</span><span><strong>Peer discovery unavailable</strong><small>Discovery and pairing are not implemented yet</small></span></div>
          </section>

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

          <section className="setup-panel">
            <div className="setup-symbol">✦</div>
            <div><strong>Set up your paired workspace</strong><p>The dashboard is a local shell right now. Pairing, file access, display streaming, and live portals are still being implemented.</p></div>
            <span className="setup-progress">Foundation in progress</span>
          </section>

          <footer><span>Unified Workspace <b>0.1.0 · Development</b></span><span>Local network only <i className="secure-mark">⌑</i></span></footer>
        </div>
      </section>
    </main>
  );
}
