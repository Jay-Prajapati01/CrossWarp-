# Start Here — Unified Cross-PC Workspace

## What this product is
A single Windows desktop application, installed on **both** PCs, which targets broad compatibility across Windows versions and combines these **four concurrently active** capabilities. JACK/Windows 11 and DESKTOP-T13BKT8/Windows 10 are the first test fixtures, not the product ceiling:

1. Shared mouse and keyboard (one cursor/focus router for both machines).
2. A shared folder and file explorer accessible from both machines.
3. True, wire-free extended display (JACK's Windows 11 desktop extends to the Windows 10 PC's monitor).
4. Interactive **live app/browser window dragging and presentation**, including browser windows running on the other OS. This must not be reimplemented as sending URLs to another browser.

The four subsystems are NOT alternative modes. A user should not have to switch off one to use another.

## How it works in simple terms
- Two apps recognize and pair with each other over a trusted Wi-Fi LAN.
- A unified coordinator tracks which computer owns each visible window and where keyboard and mouse inputs belong.
- A version-appropriate supported display-driver backend makes the source Windows installation recognize a true second display. Its pixels are streamed onto the other PC's monitor.
- Native apps on JACK move onto this extended display normally.
- For apps actually running on PC B, the software shows a low-latency, interactive **remote window proxy** on JACK. The original app remains running on PC B, keeping its login state, open tabs and unsaved page state.
- PC B's original applications may be surfaced as interactive native overlays over the receiver, under unified input control.
- The shared folder stays available and background file transfers don't interrupt mouse control or streaming.

## The key technical limits
Windows does not support teleporting a running third-party application process from one installation to another. The application must create a genuine-feeling, interactive remote-window proxy instead. OS releases expose different capture and display APIs; inspect `docs/WINDOWS_COMPATIBILITY.md`, implement runtime-selected adapters, and validate the complete four-feature session on every OS group before claiming support. Likewise, real Wi-Fi has latency: measure and optimize it; never guarantee zero latency.

## Compatibility commitment
The user wants this product to work across Windows versions, including mixed-version pairs. Windows 10 and 11 are the initial physical fixtures. Older releases are not silently excluded, but their API/driver limits must be proven and recorded; the product must not claim full support until every capability works concurrently on that version.

## Direct Codex handoff prompt

> You are the lead Windows desktop engineer. Read `docs/PRODUCT_SPEC.md`, `AGENTS.md`, and `docs/WINDOWS_COMPATIBILITY.md` in full. Build broad Windows-version compatibility, including mixed-version pairs, with all four requirements active concurrently. Treat JACK/Windows 11 and DESKTOP-T13BKT8/Windows 10 as initial fixtures, not the product ceiling. First prove a true extended display through a supported per-version driver backend, a live interactive cross-PC window proxy preserving its original app session, and correct unified input ownership while files transfer. Do not substitute URL sharing, screen mirroring, or mutually exclusive modes. Then complete the shared workspace, desktop, networking, installer/security, and matrix testing. Commit working code and tests after each milestone, report real compatibility/API limits, and do not claim completion until concurrent physical acceptance passes.

## Files in the documentation package
- `docs/PRODUCT_SPEC.md` — complete requirements, technical blueprint, APIs, file tree, UX, security, phases and acceptance tests.
- `AGENTS.md` — Codex guardrails and definition of done.
- `CODEX_START_HERE.md` — this plain-language guide and kickoff prompt.

## Test machines
- Main: JACK, Windows 11, currently 192.168.1.5.
- Second: DESKTOP-T13BKT8, Windows 10, currently 192.168.1.10.
- Both on the same trusted Wi-Fi. Address assignments may change and must never be hardcoded.
