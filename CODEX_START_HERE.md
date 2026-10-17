# Start Here — Unified Cross-PC Workspace

## What this product is
A single Windows desktop application, installed on **both** PCs, which combines these **four concurrently active** capabilities:

1. Shared mouse and keyboard (one cursor/focus router for both machines).
2. A shared folder and file explorer accessible from both machines.
3. True, wire-free extended display (JACK's Windows 11 desktop extends to the Windows 10 PC's monitor).
4. Interactive **live app/browser window dragging and presentation**, including browser windows running on the other OS. This must not be reimplemented as sending URLs to another browser.

The four subsystems are NOT alternative modes. A user should not have to switch off one to use another.

## How it works in simple terms
- Two apps recognize and pair with each other over a trusted Wi-Fi LAN.
- A unified coordinator tracks which computer owns each visible window and where keyboard and mouse inputs belong.
- An Indirect Display Driver makes Windows 11 recognize a true second display. Its pixels are streamed onto PC B's monitor.
- Native apps on JACK move onto this extended display normally.
- For apps actually running on PC B, the software shows a low-latency, interactive **remote window proxy** on JACK. The original app remains running on PC B, keeping its login state, open tabs and unsaved page state.
- PC B's original applications may be surfaced as interactive native overlays over the receiver, under unified input control.
- The shared folder stays available and background file transfers don't interrupt mouse control or streaming.

## The key technical limit
Windows does not support teleporting a running third-party application process from one Windows installation to another. The application must create a genuine-feeling, interactive portal instead. Any UI should describe this accurately. Likewise, real Wi-Fi has latency: measure and optimize it; never guarantee zero latency.

## Direct Codex handoff prompt

> You are the lead Windows desktop engineer. Read `docs/PRODUCT_SPEC.md` and `AGENTS.md` in full. Build the specified unified application with the four requirements active concurrently. First create the repository and a practical Phase 0 feasibility plan, then implement and demonstrate (1) a real IddCx-backed extended Windows display over Wi-Fi, (2) a live interactive window proxy from Windows 10 to Windows 11 preserving the originating app's actual session, and (3) correct mouse/keyboard routing while those surfaces coexist. Do not silently replace the remote window portal with browser URL syncing or ordinary screen mirroring. After verifying the hard parts, implement authenticated pairing, shared files, unified dashboard, diagnostics and packaging. Commit working code and tests after each milestone, report genuine blockers, and do not claim completion until the two-machine simultaneous acceptance test passes.

## Files in the documentation package
- `docs/PRODUCT_SPEC.md` — complete requirements, technical blueprint, APIs, file tree, UX, security, phases and acceptance tests.
- `AGENTS.md` — Codex guardrails and definition of done.
- `CODEX_START_HERE.md` — this plain-language guide and kickoff prompt.

## Test machines
- Main: JACK, Windows 11, currently 192.168.1.5.
- Second: DESKTOP-T13BKT8, Windows 10, currently 192.168.1.10.
- Both on the same trusted Wi-Fi. Address assignments may change and must never be hardcoded.
