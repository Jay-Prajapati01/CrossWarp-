# Unified Cross-PC Workspace
## Product Requirements, Architecture, Repository Blueprint, and Codex Implementation Specification

**Document version:** 1.0  
**Status:** Build specification / proposed architecture; not a completed or validated implementation  
**Target platforms:** Windows 11 primary (JACK), Windows 10 secondary (DESKTOP-T13BKT8)  
**Connection:** Same trusted Wi-Fi or local Ethernet network, with no special cable and no cloud account required  
**Audience:** Product owner, Codex engineering agent, developers, and testers

---

## 1. Executive summary

Build **one installable Windows application**, installed on both computers, that unifies four core capabilities:

1. **Universal mouse and keyboard:** One physical keyboard and mouse control both independent Windows operating systems.
2. **Instant shared workspace:** A shared folder and integrated two-computer file explorer, accessible from both computers without manual Windows share configuration.
3. **True extended display:** Windows on JACK recognizes a second monitor, whose image is streamed to the screen of DESKTOP-T13BKT8. JACK applications can be dragged there normally.
4. **Live cross-computer application/browser handoff:** A running window originating on either PC can be *presented* and interacted with on the other's screen, preserving the original app session. This is **not** URL sync, relaunching Chrome, or transferring a process. A browser tab moved across PCs remains interactive through a live remote view of its original browser session.

**Mandatory product invariant:** After connecting the two devices, all four capabilities remain **enabled, available, and able to operate concurrently**. Initiating extended display must not turn off the input-sharing service, file workspace, or window handoff. Live window handoff must not tear down extended display. One unified control/routing system coordinates the features instead of running conflicting independent mouse/display utilities.

The initial product is Windows-to-Windows and works locally without cloud infrastructure. It **does not require PowerToys, spacedesk, Chrome account sync, or manual SMB shares** at runtime. The application may study official APIs and integrate appropriately licensed components internally, but the delivered user experience is one installer and one dashboard per machine.

### 1.1 What 'simultaneously' specifically means

- A paired session establishes **one persistent device relationship**.
- The input, files, display, and window-portal subsystems are initialized in the same session and remain operational together.
- At any moment the user can copy a file, control a secondary-origin app, display a primary-origin app on screen B, and invoke browser-window handoff without changing a mutually exclusive software mode.
- A foreground video receiver on B does **not** shut down B's local agent or its Windows applications. B applications can be surfaced through an overlay/window portal and controlled via the same router.
- If an individual capability encounters an error, the other capabilities remain functional while the dashboard reports the affected subsystem. No fabricated “all healthy” status.

### 1.2 Important OS and hardware truth

- **A live Windows process cannot be relocated unchanged into another independent Windows installation.** Cross-OS “moving” is achieved by a remotely rendered, interactive **window proxy** while the process continues running on its original PC. This must look and act like handoff to the user but should be described accurately.
- **One physical B monitor cannot simultaneously show two separate full-screen Windows desktops in the same pixels.** The design resolves this by compositing JACK's extended-desktop stream with individually surfaced System B windows in a unified workspace. There must be a clear focus/ownership rule for every visible window.
- **Zero latency cannot be guaranteed over Wi-Fi.** The engineering goal is consistently responsive interaction, adaptive frame delivery, minimal contention, and measurable performance; the app must never claim physical zero lag.
- Driver development, compatible Windows builds, capture permissions, secure-desktop boundaries, and signed distribution are genuine engineering requirements.

---

## 2. The exact problem being solved

Today a user must separately install and configure PowerToys Mouse Without Borders, Windows network sharing, a wireless display utility, and Chrome account sync or browser extensions. Setup involves firewall rules, IP addresses, passwords, conflicting mouse/display behavior, and incompatible mental models. Worse, traditional mouse-sharing alone does not let users drag a live application window between operating systems.

**Desired experience:**

> Install one app on both PCs, see nearby devices, approve a short pairing code, click **Connect Workspace**, and immediately see one coordinated environment for mouse/keyboard input, shared files, extended display, and interactive cross-PC windows.

### 2.1 First test devices

| Attribute | PC A | PC B |
|---|---|---|
| Device name | JACK | DESKTOP-T13BKT8 |
| OS | Windows 11 | Windows 10 |
| Role | Primary workstation and display source | Secondary computer and display receiver |
| Example LAN IP | 192.168.1.5 | 192.168.1.10 |
| Installed software | Same Unified Workspace app | Same Unified Workspace app |

**Never hard-code these IP addresses into the product.** LAN discovery and pair identity must keep working when DHCP changes them. Treat these as lab fixtures, not production configuration.

---

## 3. Non-negotiable feature requirements

### F1 — Universal mouse, keyboard, and clipboard

**User outcome:** Use one mouse and keyboard to operate windows belonging to either PC, without manually switching peripherals.

**Required behavior:**

- Automatic pointer movement and keyboard focus between logical devices and screens.
- Support keyboard shortcuts, wheel, multi-button mice, mixed resolutions, and multi-monitor layouts.
- Bidirectional text and common image clipboard formats, with an explicit privacy-aware clipboard history policy.
- Copy/paste small and large files via the unified file subsystem, **not** an arbitrary 100 MB clipboard cap.
- Correct focus routing whether the pointer is over (a) JACK's native screen, (b) JACK's virtual screen on B, or (c) a remotely presented B application on either physical screen.
- Smooth transition across screen boundaries; no duplicated clicks, stuck keys, accidental double pointers, or keyboard input to the wrong OS.
- Hotkey to recover focus immediately if networking stalls; hardware on each PC must remain usable.
- No privileged desktop/UAC bypass: elevated and secure-desktop boundaries are respected unless a separately authorized Windows-supported approach has been implemented and security reviewed.

**Implementation sketch:** Capture authorized input events with Windows native APIs, feed them to a session-wide `InputRouter`, maintain an `OwnershipMap` for the visible surfaces, inject only on the correct target with Windows-supported APIs, and reconcile key/button state after reconnect. Avoid Windows process-wide hooks unless necessary and carefully scoped. `SendInput` is subject to Windows UIPI restrictions.

**Interfaces:** `InputEvent`, `PointerOwner`, `FocusChanged`, `ClipboardChanged`, `SetScreenLayout`, `EmergencyReleaseInput`.

### F2 — Shared Workspace folder and remote file management

**User outcome:** Both PCs can see and edit the same authorized files, using a folder clearly called **Shared Workspace**, with no manual creation of SMB users/passwords.

**Required behavior:**

- On first setup, the user selects a local storage location for the Shared Workspace (default created by the app on JACK); the app obtains explicit permission.
- Both PCs show **Shared Workspace** in the app's file browser. Files placed there are available immediately to the other PC **while connected**.
- Folder operations: enumerate, preview, open, read, write, rename, move, delete (with confirmation for destructive operations), drag files, and bulk transfer.
- Reliable writes: stream to temporary file, verify integrity, then atomically replace/commit where possible. Detect concurrent edits.
- Progress indicator, cancellation, integrity checking, reconnect/retry, and resume of interrupted transfers where safely implementable.
- Access policies: only approved roots, per-device authorization, no unrestricted access to C:\ or other drives by default.
- User may add further shared folders by explicit authorization; a separate **Remote Devices** browser shows only permitted roots.
- Do not block mouse, video streaming, or window handoff during large file transfers. Throttle file throughput under contention.
- File may be opened by either machine using the appropriate editor on that machine; save returns changes to source-of-truth storage.

**Default data model:** One authoritative `Shared Workspace` hosted on JACK and exposed to B over the authenticated LAN transport. This is **not** automatically a permanently synced duplicate on both drives. Optional offline mirroring/caching is a later improvement and must clearly indicate synchronization conflicts.

**Implementation:** App-managed authenticated file service over an encrypted connection; app-integrated explorer first. Optional Windows File Explorer drive mounting is a later integration (such as a reviewed filesystem adapter) rather than a prerequisite. This avoids forcing the user to configure SMB manually. If SMB is ever used internally, its installation and authorization must be managed safely and transparently.

**Interfaces:** `ListDirectory`, `ReadFileRange`, `BeginUpload`, `UploadChunk`, `CommitUpload`, `CreateDirectory`, `MoveEntry`, `DeleteEntry`, `GetTransferStatus`, `CancelTransfer`, `GrantFolderAccess`.

### F3 — True extended monitor, wirelessly

**User outcome:** Windows 11 on JACK **recognizes a real logical second display** and users can naturally drag a JACK application to System B's physical monitor.

**Required behavior:**

- An IDD/IddCx virtual display integration on JACK registers a monitor with Windows.
- Windows Display Settings exposes the virtual display, and the user's intended mode is **Extend** (not merely screen duplication or a remote desktop window).
- Render the virtual display to the full-screen receiver surface on System B, over the local network.
- JACK's normal Windows dragging works for JACK's Chrome, VS Code, File Explorer, video editors, and other normal desktop windows.
- Resolution handling, DPI awareness, physical monitor arrangement, pointer alignment, reconnect, resize, power events, and fallback encoding.
- Stream using hardware-assisted low-latency H.264 where available, with reasonable software fallback; measured frame rate, dropped frames, RTT, and bitrate.
- Input from the physical keyboard/mouse maps to the correct origin OS and window despite the virtual receiver.
- **Crucially: keep the B agent, file service, and B-origin window presenter running even while the receiver is displaying the extended screen.**
- Ensure an explicit user escape control and safe driver/display detach without disappearing app windows or leaving an inaccessible ghost monitor.

**Implementation:** Windows IddCx UMDF Indirect Display Driver on JACK; a privileged installer component for safe driver installation; a user-session capture/encoding bridge or the IDD swapchain-processing path; encrypted real-time media transport to a receiver on System B. The IDD does **not** run UI code in Session 0. Refer to Microsoft's IDD sample, but do not ship sample code as production driver without completing its TODOs, security review, and signing.

**Interfaces:** `CreateVirtualMonitor`, `AttachReceiver`, `SetDisplayTopology`, `SetDisplayMode`, `StreamFrame`, `ReportStreamStats`, `DetachVirtualMonitor`.

### F4 — Interactive cross-device Browser & App Window Portal

**User outcome:** Move or reveal a running application or browser window from either computer onto the other screen and continue interacting with that **same live session**. No dependency on Google's tab sync.

**Two supported paths, united in one experience:**

**Path A — Same-OS native window drag (JACK → B extended monitor).** Because B's monitor is a logical JACK display, dragging a JACK window to that display is ordinary Windows behavior. Chrome tabs can be moved as Windows windows or detached from their source browser window and dragged to the extended display without Chrome sign-in.

**Path B — Cross-OS live window portal (B-origin window → JACK screen, or other cross-host placement).** A selected window is captured on its origin PC, streamed as an interactive remote-window surface, rendered inside a local movable/resizable **window proxy** on the destination display, and receives correctly mapped keyboard/mouse events. The original process and browser session **stay on the source PC**. This is the required replacement for “Send URL to other computer.”

**Required behavior:**

- Detect eligible application windows, show an ownership badge in the in-app Window Navigator, and expose a **Move to Other Screen** command.
- Support drag-from-window to destination when the native app's window style and security level permit; integrate a small draggable window grip/portal handle and a keyboard shortcut where the operating system cannot safely intercept arbitrary third-party title-bar drags.
- Moving a B-origin browser session onto JACK **must display the existing live browser state** (open page, scroll position, active website session), not just recreate the URL.
- Streaming and input feedback must remain responsive without deactivating file access, cross-PC keyboard/mouse, or virtual display.
- Include window move, resize, focus, minimize and close behavior with correct source-window ownership. Closing a proxy must **not** silently terminate the origin app; closing the origin app may be a separate explicit action.
- User can bring B's own windows above the spacedesk-like receiver on B using **native overlay surfaces**, without stopping the virtual display stream. Local System B windows remain truly local and interactive; the workspace controller routes focus based on what is visibly on top.
- For browser tabs: support moving a **detached tab window** via the same live-window portal. Chrome/Edge extension may add a tab picker and “Present tab on other device,” but tab handoff must not secretly become URL sync. Non-Chromium browsers still work at the general window level when capture permits.
- Explicit capture permission and visible active-capture indicator. Respect operating-system and browser restrictions; DRM/protected frames, incognito/private content, minimized windows, secure desktops, and some accelerated applications may be unavailable. Surface a precise supported/unsupported state.
- No covert screen recording and no unattended remote capture by an unapproved peer.

**Implementation:** Windows.Graphics.Capture for per-window capture when supported; native window discovery; low-latency remote rendering; window proxy and overlay surface; session-wide input routing. Chrome/Edge Manifest V3 companion extension only for *tab-level UX and metadata* using tabs/windows APIs and native messaging. General app/window streaming does **not** require a browser account or extension.

**Interfaces:** `EnumerateWindows`, `BeginWindowPortal`, `PresentWindow`, `MoveWindowProxy`, `ResizeWindowProxy`, `FocusWindowProxy`, `ForwardWindowInput`, `StopWindowPortal`, `ListBrowserTabs`, `PresentBrowserTab`.

**Definition of handoff:** A live interactive visual session, **not** native process migration. Support a best-effort drag gesture plus guaranteed dedicated in-app drag grip/hotkey on supported windows. The engineering team must not claim arbitrary app title-bar drag works universally until independently demonstrated across the acceptance suite.

---

## 4. Single-session architecture: four capabilities running together

```text
   USER'S PHYSICAL MOUSE + KEYBOARD (connected to either PC)
                          |
                          v
                +------------------------+
                | Workspace Coordinator  |
                | Pairing / Session      |
                | Screen & Window Owners |
                | Unified Input Router   |
                +-------------+----------+
                              |
        +---------------------+----------------------+-------------------+
        |                     |                      |                   |
        v                     v                      v                   v
 +-------------+       +--------------+       +---------------+   +----------------+
 | INPUT       |       | FILE SERVICE |       | VIRTUAL       |   | WINDOW PORTAL  |
 | mouse, keys |       | Shared       |       | DISPLAY       |   | native apps +  |
 | clipboard   |       | Workspace    |       | IDD + video   |   | live browsers  |
 +-------------+       +--------------+       +---------------+   +----------------+
        |                     |                      |                   |
        +---------------------+----------------------+-------------------+
                              |
                   AUTHENTICATED LOCAL TRANSPORT
                     Control + file + media QoS
                              |
               +--------------+--------------+
               |                             |
          PC A: JACK                    PC B: DESKTOP-T13BKT8
          Windows 11                    Windows 10
          agent + UI                    agent + UI
          virtual display              full-screen receiver
          file authority               native window overlays
          window source                 window source/proxies
```

### 4.1 Architecture principles

1. **One input owner per visible pixel region.** The compositor/routing layer knows whether a click targets JACK's native/extended desktop, B's native overlay, or a remote window proxy. No second independent “mouse-share engine.”
2. **One pairing trust relationship.** All subsystems use the same authenticated device identities and approved capability grants, with distinct authorization scopes.
3. **Independent workers, coordinated session.** Input, file, media, and window streams run concurrently in separate task pools, but share connection status, QoS, and lifecycle management.
4. **Background file transfer has lower QoS priority than input and interactive video.** Throttle bulk copying when stream quality is threatened.
5. **No feature exclusivity flags.** A virtual-display lifecycle action cannot call `disableInput()`, `disableFiles()`, or `disableWindowPortal()`.
6. **Graceful degradation is not secretly a mode switch.** If a GPU cannot encode, choose compatible software mode and show performance; if a capability fails entirely, report it explicitly and preserve other capabilities while recovering it.
7. **System B OS remains alive under receiver.** The receiver is a render surface in the B user session, not a substitute OS; B's apps are either shown as native overlay windows or surfaced through window proxies.

### 4.2 Window ownership and focus rules

| Visible surface | Process executes on | Rendering | Keyboard/mouse destination |
|---|---|---|---|
| JACK app on JACK monitor | JACK | Native JACK | JACK |
| JACK app on B extended screen | JACK | JACK IDD stream → B receiver | JACK |
| B app floating over receiver on B | B | Native B overlay | B |
| B app proxy shown on JACK | B | Captured B window → JACK proxy | B (transformed coordinates) |
| JACK app remote proxy shown in B-origin window context | JACK | Captured JACK window → B proxy | JACK |

**Focus owner transitions are atomic.** The router must release stale key/buttons, update ownership, and deliver events once; latency cannot cause both machines to act on one click.

### 4.3 How the important simultaneous example works

1. Both users' agents are connected. All four modules remain initialized.
2. JACK's Windows desktop gets logical display #2 through IDD. Its video is streamed to the B monitor.
3. User drags JACK's VS Code onto B's monitor: it remains a JACK process; no browser sync is involved.
4. B continues running Chrome locally. User selects its live Chrome window in the Window Navigator and drags its portal grip onto JACK's screen.
5. A low-latency proxy displays that same B Chrome session on JACK, forwards input to B, and preserves cookies/state on B because Chrome never closed or relocated.
6. User simultaneously transfers a file into Shared Workspace. File traffic throttles if needed; mouse events and video continue.
7. To interact with a B native app on B's monitor, user surfaces it as a local overlay above the streaming receiver; pointer ownership switches to B for that native overlay without disconnecting JACK's extended monitor.

**This, not “turn off PowerToys then turn on spacedesk,” is the mandatory system design.**

---

## 5. Suggested technology stack (keep it focused)

| Area | Default choice | Reason |
|---|---|---|
| Desktop app and dashboard | Tauri 2 + React + TypeScript | Lightweight native desktop UI, IPC to Rust |
| Core agent/networking | Rust (Tokio async runtime) | Shared logic, secure state handling, concurrency |
| Windows native adapters | C++ with Win32/WinRT/DirectX | Input APIs, window capture, graphics, IDD integration |
| LAN discovery | mDNS plus manual IP fallback | Find PCs despite changing IP addresses |
| Pairing/security | Device keypairs + one-time verification code + mutually authenticated encryption | One pairing reused across capabilities |
| Control & file RPC | Rust HTTPS/HTTP2 or QUIC with typed messages | Reliable request/response and file streaming |
| Low-latency media | Reviewed WebRTC stack with hardware H.264 integration | Congestion handling, low-latency video and media/input channels |
| True Windows display | IddCx / UMDF Indirect Display Driver (C++/WDK) | Windows recognizes extendable monitor |
| Remote app/window capture | Windows.Graphics.Capture + Direct3D11 | Captures chosen app windows with Windows integration |
| Browser support | Chrome/Edge Manifest V3 extension + Native Messaging | Tab metadata and authorized tab-selection UX |
| Preferences + share metadata | SQLite and OS-protected credential storage | Local configuration; no cloud dependency |
| Installer | MSI/WiX or signed installer + dedicated driver component | Single guided install, safe upgrade/rollback |
| Tests | Rust tests, Vitest, Playwright UI tests, Windows integration harness | Automated & two-machine validation |

**Architecture note:** The WebRTC library/binding is a decision to verify in the first technical spike, not an excuse to build a custom video codec or transport. Prefer the simplest actively maintained, license-compatible integration that works reliably on both OS versions. Avoid unnecessary servers, accounts, Docker, Redis, or cloud services.

---

## 6. Repository structure and file responsibilities

```text
unified-cross-pc-workspace/
|
|-- README.md                       # user install and run instructions
|-- AGENTS.md                       # Codex guardrails and completion requirements
|-- Cargo.toml                      # Rust workspace
|-- package.json                    # frontend scripts
|-- pnpm-workspace.yaml
|-- .github/workflows/
|   |-- ci.yml                      # checks, tests, build
|   `-- windows-release.yml         # release, sign, verify artifacts
|
|-- docs/
|   |-- PRODUCT_SPEC.md             # canonical product requirements (this document)
|   |-- ARCHITECTURE.md             # diagrams + module boundaries
|   |-- PROTOCOL.md                 # typed messages and security
|   |-- WINDOWS_DRIVER.md           # IDD build, signing and install strategy
|   |-- UI_FLOWS.md                 # setup and everyday use
|   |-- SECURITY.md                 # pairing, consent, threat model
|   |-- ACCEPTANCE_TESTS.md         # four-at-once tests
|   `-- LIMITATIONS.md              # verified Windows/API constraints
|
|-- apps/desktop/
|   |-- src/
|   |   |-- main.tsx
|   |   |-- App.tsx
|   |   |-- pages/
|   |   |   |-- Onboarding.tsx
|   |   |   |-- Dashboard.tsx
|   |   |   |-- Devices.tsx
|   |   |   |-- SharedFiles.tsx
|   |   |   |-- WindowNavigator.tsx
|   |   |   |-- Displays.tsx
|   |   |   `-- Diagnostics.tsx
|   |   |-- components/
|   |   |   |-- DeviceCard.tsx
|   |   |   |-- ScreenLayout.tsx
|   |   |   |-- FeatureStatus.tsx
|   |   |   |-- FileBrowser.tsx
|   |   |   `-- WindowCard.tsx
|   |   `-- api/workspace.ts         # typed Tauri command adapters
|   |-- src-tauri/
|   |   |-- src/main.rs
|   |   `-- src/commands.rs          # UI IPC, no hardware logic
|   `-- tauri.conf.json
|
|-- crates/
|   |-- workspace-agent/
|   |   `-- src/main.rs              # background user agent, on both PCs
|   |-- core/
|   |   `-- src/
|   |       |-- session.rs           # one four-feature session
|   |       |-- device.rs            # device identity, capabilities
|   |       |-- layout.rs            # logical displays and pixel owners
|   |       `-- state.rs             # state machine and lifecycle
|   |-- discovery/
|   |   `-- src/lib.rs               # mDNS, LAN peer lookup
|   |-- pairing/
|   |   `-- src/lib.rs               # key exchange and trust store
|   |-- transport/
|   |   `-- src/
|   |       |-- control.rs           # authenticated control RPC
|   |       |-- media.rs             # WebRTC transport adapter
|   |       `-- qos.rs               # traffic budgets and priority
|   |-- input-router/
|   |   `-- src/
|   |       |-- capture.rs           # native input adapter interface
|   |       |-- ownership.rs         # mouse/keyboard target arbitration
|   |       |-- forwarding.rs        # sequence/ack, release/recovery
|   |       `-- clipboard.rs
|   |-- shared-files/
|   |   `-- src/
|   |       |-- server.rs            # authorized workspace service
|   |       |-- client.rs            # remote file browser API
|   |       |-- transfers.rs         # progress, resume, hashing
|   |       `-- permissions.rs
|   |-- display-stream/
|   |   `-- src/
|   |       |-- session.rs           # virtual display lifecycle
|   |       |-- encoder.rs           # hardware/software codec adapter
|   |       |-- receiver.rs          # viewer and decode integration
|   |       `-- metrics.rs           # FPS, latency, jitter
|   |-- window-portal/
|   |   `-- src/
|   |       |-- catalog.rs           # eligible HWND inventory
|   |       |-- portals.rs           # source/proxy lifecycle
|   |       |-- focus.rs             # remote focus and coordinates
|   |       `-- browser.rs           # browser metadata bridge
|   `-- diagnostics/
|       `-- src/lib.rs               # subsystem health and safe fixes
|
|-- native/windows/
|   |-- input/
|   |   |-- input_capture.cpp
|   |   |-- input_inject.cpp
|   |   `-- window_focus.cpp
|   |-- capture/
|   |   |-- graphics_capture.cpp     # per-window WGC capture
|   |   `-- window_enumeration.cpp
|   |-- receiver/
|   |   |-- receiver_window.cpp      # stream presentation on PC B
|   |   `-- local_overlay.cpp        # native B-window overlay manager
|   |-- display-driver/
|   |   |-- WorkspaceDisplay.sln
|   |   |-- WorkspaceDisplay.inf
|   |   |-- Driver.cpp               # IddCx driver implementation
|   |   |-- SwapchainProcessor.cpp
|   |   `-- README.md                # WDK + sign/installation notes
|   `-- bridge/
|       `-- rust_ffi.cpp             # narrow FFI bridge to Rust core
|
|-- extensions/
|   |-- chromium-handoff/
|   |   |-- manifest.json
|   |   |-- background.ts            # tab selection/metadata
|   |   |-- popup.tsx                # Present live tab / window UX
|   |   `-- native-messaging/host.json
|   `-- native-host/
|       `-- src/main.rs              # extension ↔ local agent protocol
|
|-- installer/
|   |-- installer.wxs                # WiX packaging
|   |-- install-driver.ps1           # privileged, consented component
|   |-- register-native-host.ps1     # exact extension ID, browser manifests
|   `-- rollback.ps1
|
|-- tests/
|   |-- unit/
|   |-- integration/
|   |   |-- pairing_test.rs
|   |   |-- routing_test.rs
|   |   |-- files_test.rs
|   |   `-- simultaneous_session_test.rs
|   |-- windows-e2e/
|   |   |-- two_pc_smoke.md
|   |   |-- native_display.md
|   |   `-- live_window_handoff.md
|   `-- performance/
|       `-- wifi_benchmark.md
|
`-- scripts/
    |-- dev-setup.ps1
    |-- run-local.ps1
    |-- diagnostics.ps1
    `-- release-check.ps1
```

**Build order rule:** Create the specified module boundaries early; each phase fills in the implementation and tests. Keep driver build tooling separate from normal frontend iteration. Every native bridge must have an explicit typed interface; don't put DirectX/Win32 code inside React.

---

## 7. Local APIs, events and contracts

**Internal APIs only.** No public internet API or cloud backend required. Use typed schema definitions (`serde` and TypeScript interfaces) generated or validated from one authoritative contract. Version all protocol messages.

### 7.1 Main desktop-to-agent commands

| Command | Parameters | Result |
|---|---|---|
| `discover_devices` | none | list of nearby device identities |
| `begin_pairing` | remote device ID | challenge / pairing code |
| `confirm_pairing` | challenge ID + approved code | durable trusted-device ID |
| `connect_workspace` | paired device ID | `session_id`, capability states |
| `get_session_state` | session ID | peers, stream stats, 4 statuses |
| `set_screen_layout` | ordered display geometry | new ownership map |
| `get_shared_files` | path, page cursor | approved directory entries |
| `start_file_transfer` | source, destination | transfer ID |
| `list_windows` | peer ID | eligible visible windows |
| `present_window` | peer ID, window ID, destination geometry | portal ID |
| `move_window_proxy` | portal ID, bounds | updated portal bounds |
| `get_display_state` | none | IDD / receiver state |
| `set_display_geometry` | resolution, layout | updated Windows display topology |
| `get_diagnostics` | none | per-subsystem checks and remediation hints |

### 7.2 Transport events

```json
{
  "protocol_version": 1,
  "session_id": "opaque-session-identifier",
  "event": "workspace.status",
  "from_device": "JACK-device-id",
  "capabilities": {
    "input": "ready",
    "files": "ready",
    "display": "streaming",
    "window_portal": "ready"
  }
}
```

```json
{
  "protocol_version": 1,
  "session_id": "opaque-session-identifier",
  "event": "window.portal.begin",
  "portal_id": "opaque-portal-id",
  "origin_device": "DESKTOP-T13BKT8-device-id",
  "source_window_id": "opaque-window-token",
  "destination_device": "JACK-device-id",
  "placement": { "x": 120, "y": 180, "width": 960, "height": 600 },
  "capture_consent_id": "approved-consent-token"
}
```

```json
{
  "protocol_version": 1,
  "session_id": "opaque-session-identifier",
  "event": "input.pointer",
  "sequence": 101,
  "portal_id": "opaque-portal-id",
  "x": 0.52,
  "y": 0.31,
  "buttons": 0,
  "timestamp_us": 10005000
}
```

**Rules:** device names are friendly labels only; use stable cryptographic device identities. Remote HWNDs are not exposed as trusted pointers across processes. Validate coordinates, frame sizes, rate limits, origin rights, and permission tokens. No executable shell commands through RPC. Clipboard and input packets only within authorized sessions. Never log pairing secrets or user file contents.

### 7.3 Session lifecycle

```text
Unpaired -> Discovered -> UserApproved -> Authenticated -> Connected
                                                        |
                                               Four services READY
                                                        |
                                   Display STREAMING + Portals PRESENTED
                                                        |
                Network interrupted -> Reconnecting -> Same trusted session
                                                        |
                               User disconnect -> resources released safely
```

Display connection or disconnection changes **only the display state**; it does not invalidate pairing, shared files, or input service. A remote portal may be opened or closed independently while the rest stay available.

---

## 8. Dashboard and UX requirements

**Visual direction:** Professional developer-friendly interface; neutral background, restrained blue/teal state accent, legible device cards, clear icons. No gamer aesthetic or clutter. Responsive for 1080p displays; quick tray controls; setup wizard readable on Windows 10 and 11.

### 8.1 Primary screens

1. **Welcome & Pair:** “Find devices on my Wi-Fi” → select detected PC → compare code on both → approve → connect once. Do not ask the user to enter IPs, usernames, Windows share passwords, or browser accounts during standard setup.
2. **Unified Dashboard:** Two connected device cards with one main label **Workspace Active**, and four simultaneously live indicators: Input, Shared Files, Extended Display, Live Window Handoff. Display each subsystem's actual status and recent diagnostics.
3. **Screen Layout:** Drag representations of physical/logical displays and active portals. Show ownership labels (JACK / B) and current focus visually, not intrusive overlays everywhere.
4. **Shared Files:** Two-column local/remote explorer, pinned Shared Workspace, transfers drawer, permission grants.
5. **Windows & Tabs:** Live list of eligible windows, browser tabs when extension is installed; drag a window card or click **Present on Other Screen**; explicit live/remote badge.
6. **Display & Performance:** Resolution, refresh target, actual FPS, latency, Wi-Fi quality, selected codec. Stream starts automatically on workspace connect when approved by the user.
7. **Diagnostics:** “What failed?”, last known good connection, ability to run safe tests and apply user-approved repairs. Not just a red dot.
8. **Tray Controls:** Open dashboard, show/hide remote overlays, rescue mouse focus, disconnect safely, view session status.

### 8.2 Standard first-use journey

```text
Install on JACK + System B
  -> first run explains access and local-network permissions
  -> discovery detects the other computer
  -> verify short code on both computers
  -> click Connect Workspace once
  -> shared workspace created after confirming storage location
  -> extended display driver installed once with required elevation/signing
  -> display appears in Windows Extend mode and B receiver opens
  -> input owner and window portal continue active
  -> all four dashboard statuses healthy simultaneously
```

The display driver may require an explicit administrator-approved installation and an OS restart. **Do not hide or bypass that security requirement.** This is still one guided app setup, not four unrelated third-party setup processes.

---

## 9. Security, permissions, and operational constraints

- Trusted local networks only by default; firewall rules apply to private networks and the app executable/ports rather than globally opening Windows services.
- Require approval on **both PCs** for first-time pairing. Verify cryptographic identities and use secure device-to-device transport.
- Enumerate/share only user-approved folders. Protect keys using OS-backed credential storage and limit agent privileges.
- Show visible indicators and controls whenever a live remote window is captured or presented; users may revoke sessions/devices.
- Do not capture/forward passwords in a special way, defeat DRM, bypass login screens, UAC, Windows secure desktop, elevated apps, or protected window restrictions.
- Support local app windows when the user is logged into that Windows session. A locked/disconnected Windows user session may require graceful pause of input/video/windows; file service authorization must be independently evaluated.
- Validate safe uninstallation, firewall cleanup, driver removal, and application update rollback.
- Windows 10 standard support has ended; prioritize security updates, compatibility checks, and a future OS upgrade plan.
- Avoid reusing external code without checking copyright, redistributability, and compatibility with the chosen project license.
- Target LAN-only operation. No user data should be uploaded to an external server by default.

---

## 10. Performance requirements and network coexistence

The goal is low-latency practical usability on **one reliable 5 GHz/6 GHz Wi-Fi LAN**; 2.4 GHz may be insufficient depending on congestion.

**Initial engineering targets, not universal guarantees:**

| Metric | Lab target | How to verify |
|---|---|---|
| Primary extension | 1920x1080, nominal 60 FPS | Windows display settings + frame telemetry |
| Video smoothness | >= 55 decoded FPS in an idle desktop test on reference hardware | 5-minute sample, frame statistics |
| Perceived pointer responsiveness | p95 visible-action latency under 120 ms on reference LAN | LED/high-speed video or synchronized test rig |
| Input correctness | no duplicate input or stuck keys in 30-minute test | injected test sequences + real use |
| File fairness | file transfer may slow; interactive stream and input must not starve | transfer of multi-GB test folder while active |
| Reconnection | recover session automatically after transient Wi-Fi drop when feasible | unplug/rejoin network tests |
| CPU/GPU | configurable quality limits, avoid sustained overload | Windows perf counters and app telemetry |

Latency, FPS and resolution depend on Wi-Fi quality, GPU encoding/decoding support, app content, power mode, and hardware. Provide user-visible measured values rather than unconditional “no lag” marketing.

**QoS policy:** input/control highest priority → live/window video → clipboard and small files → bulk transfers. Encoders adapt bitrate/resolution; large transfers are chunked and throttled. Bound memory queues; discard outdated video frames instead of buffering seconds of delay. Maintain timestamps and measure end-to-end performance.

---

## 11. Testable acceptance criteria: all four functions ON together

**Release is rejected if only two or three of the four features function during the same authenticated session.** The following test is mandatory on actual JACK and DESKTOP-T13BKT8 hardware; mocked unit tests alone cannot satisfy it.

### AT-01: Pair once
- Install the same application on both PCs; pair by local discovery and mutual approval.
- Restart either PC. Trusted pairing persists; reconnection requires no new secret key or Windows sharing password.
- Show four accurate subsystem states in the dashboard.

### AT-02: Native extended drag (JACK app → B monitor)
- Windows on JACK lists a secondary extended display.
- Start VS Code and Chrome on JACK; drag their **actual local Windows windows** from JACK's primary monitor to B's monitor.
- Move back; resize; maximize; switch apps; ensure Windows still owns the running apps on JACK.

### AT-03: Live B browser on JACK — NOT URL copy
- Start Chrome on B, sign in to a test site, scroll, enter unsaved text in a test form.
- With JACK's extended display **still active**, use the window-portal UI or supported drag grip to place B's existing Chrome window on JACK's physical display.
- Verify the exact live state persists without reopening the URL. Type and scroll in the proxy and confirm B's original instance responds.
- Test an alternative browser (e.g., Firefox) at whole-window level without a Chrome-specific extension.
- Verify private/DRM/protected windows fail clearly and do not leak content.

### AT-04: Input router with all layers active
- While JACK's extended output remains streaming, move pointer across JACK screen, B extended-receiver surface, a B-native overlay on B, and the B-window proxy on JACK.
- Keyboard goes only to the active owner; physical mouse remains functional; no cursor warping, duplicate clicking or stuck modifiers.
- Unplug or stop stream: hotkey restores safe local focus.

### AT-05: Shared files concurrently
- While AT-02/03/04 remain operational, copy a multi-GB folder into Shared Workspace from B, browse from JACK, open/edit/save a file, and confirm changes.
- Verify conflict handling, upload integrity, cancellation and progress.
- Video and keyboard continue operating during transfer.

### AT-06: Stability and recovery
- Sustain all four working together for 30 minutes while using Chrome, VS Code, file transfers, and pointer movement.
- Test Wi-Fi interruption/reconnection; verify no corruption, orphan window proxy, or stranded display.
- Test application crash and restart on either PC; resources cleaned and pairing retained.

### AT-07: Installation and trust
- New install works from one installer per PC, with permission prompts only when necessary.
- No user-created SMB accounts, no PowerToys/spacedesk manual setup, no cloud login, no browser sync dependency.
- Valid signature and supported driver installation path for intended Windows releases. Reject unknown/unsigned driver loading in standard-user production flow.

### AT-08: Browser terminology
- The UI must say **Live Window Handoff** / **Present Live Tab**, not imply a native app process is transferred.
- Full native app drag between independent operating systems is never falsely claimed if portal grip/hotkey is required.

**Test report must include** video/screen recording of the two-machine test (with private content redacted), build version, GPU and Wi-Fi details, p50/p95 latency, average FPS, logs, pass/fail per test, and any unsupported content types.

---

## 12. Development plan for Codex (implement end-to-end, no feature substitution)

### Phase 0 — Feasibility spikes and technical proof

Before building a polished dashboard, validate the two hardest mechanisms on the real PCs:

1. Build a minimal IDD proof of concept, register an extendable Windows display and draw a live stream on B. Document Windows driver signing requirements and any hardware restrictions.
2. Capture a selected B-origin Chrome window using Windows.Graphics.Capture; show an interactive low-latency proxy on JACK and send input back to the B process.
3. Build a focus/ownership proof demonstrating JACK's extended content and B-native overlay/proxy control without disabling mouse routing.
4. Check viability of the selected WebRTC/Windows capture/native bridge libraries and licenses.

**Gate:** If real devices cannot demonstrate these, stop and report exactly what remains blocked. Do **not** quietly substitute screen mirroring, Chrome URL sync, or a mouse-only tool.

### Phase 1 — Repository foundation and security

- Create monorepo and documented build scripts for Windows.
- Implement device IDs, mDNS discovery, pairing verification, encrypted control channel, connection health, logging.
- Start the same agent on both PCs; dashboard shows peer and capability health.
- Deliver tested end-to-end secure connection without hard-coded IPs.

### Phase 2 — Unified Input Router + Shared Workspace

- Capture/input forwarding; global screen layout and ownership; clipboard.
- Shared Workspace creates a local root with user consent and exposes an authorized file service.
- Add direct browsing and transfer progress while mouse still works.
- Tests for secure boundaries and reconnection.

### Phase 3 — Display driver and live extended output

- Implement IDD, installer integration, swapchain processing/encoding, receiver and display settings.
- Ensure input, clipboard, and file service remain active during streaming.
- Wire nonblocking media telemetry and cleanup.

### Phase 4 — Universal Window Portal + Browser UX

- Enumerate Windows windows, capture per approved HWND, stream into movable proxy surfaces, forward focus/input, manage origin ownership.
- Add B native overlay layer over B receiver with proper z-order/input routing.
- Implement window-portal grip plus cross-boundary gesture on supported window types.
- Build Chrome/Edge companion extension only for tab lists/tab-to-live-window affordances; auto-install/register Native Messaging host when appropriate. No URL-only fallback presented as success.

### Phase 5 — Full simultaneous acceptance

- Run AT-01 through AT-08 on the two physical PCs.
- Benchmark QoS with transfers while video and live window portals remain active.
- Fix focus, privacy, hardware compatibility, recovery, driver cleanup, UI polish, installer issues.
- Package reproducible artifacts and produce a signed/release-ready plan. Do not claim fully production-ready if driver signing or compatibility testing has not actually been completed.

**Completion rule:** Codex must deliver executable source, reproducible Windows builds, installers where feasible, API/docs, automated tests, and a two-machine verification report. A mocked React dashboard with nonworking buttons is not a functional delivery.

---

## 13. Engineering decisions to document early

1. **No external tools requirement:** This is a single user-facing product. Internal dependencies are fine if license-compliant and embedded correctly, but the user must not set up PowerToys, spacedesk, or another utility separately.
2. **Screen authority:** During extend, JACK is owner of the virtual display pixels; B retains its OS and can expose its native windows as overlays. Document limitations for exclusive-fullscreen games, protected windows and OS secure desktop.
3. **Origin ownership:** Every portal includes origin device, origin window ID, destination window and consent ID. Closing the proxy is separate from closing the origin process.
4. **Browser behaviors:** Chrome sync, tab re-opening, and URL-copy are optional unrelated conveniences, **not replacements** for the required live browser-window portal.
5. **Shared Workspace semantics:** Live source-of-truth shared store on JACK, not implicitly mirrored storage. The UI should say when a source device is offline.
6. **Reliability:** No code path must intentionally disable input/file/window services when virtual display starts; errors remain independently recoverable.
7. **Security:** Secure desktop/UAC restrictions, explicit capture consent, minimal filesystem scope, pair revocation, and validated signed driver installation are nonnegotiable.

---

## 14. Technical references (use official documentation)

- Windows Indirect Display Driver overview: https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview
- Microsoft's sample IDD: https://github.com/microsoft/Windows-driver-samples/tree/main/video/IndirectDisplay
- Windows.Graphics.Capture: https://learn.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture
- CreateForWindow: https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows.graphics.capture.interop-igraphicscaptureiteminterop-createforwindow
- Microsoft Capture HWND sample: https://github.com/microsoft/Windows.UI.Composition-Win32-Samples/tree/master/cpp/ScreenCaptureforHWND
- Windows SendInput/UIPI limitations: https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput
- Chrome Native Messaging: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging
- Chrome extensions API: https://developer.chrome.com/docs/extensions/reference/api/
- Windows hardware driver signing: https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings
- PowerToys mouse-sharing baseline: https://learn.microsoft.com/en-us/windows/powertoys/mouse-without-borders

### Final product promise

**One installed application. One paired session. Four concurrent capabilities: shared input, shared files, true extended display, and interactive live browser/application window portals.**

A build is **not complete** until all four work together on actual supported Windows devices, with clear and measured performance and without asking the user to change incompatible operational modes.
