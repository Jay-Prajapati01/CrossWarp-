# Codex instructions — Unified Cross-PC Workspace

## Mission
Build the Windows-first Unified Cross-PC Workspace described in `docs/PRODUCT_SPEC.md`. Read the entire product specification before modifying or generating application code. The four features are **concurrent requirements**, not mutually exclusive modes. Product scope is broad compatibility across Windows client versions and mixed-version PC pairs; JACK and DESKTOP-T13BKT8 running Windows 11/10 are initial fixtures, not the compatibility ceiling. See `docs/WINDOWS_COMPATIBILITY.md` for verified API constraints and the validation matrix.

## Required behaviors
1. One device pairing and one application installed on each PC.
2. Shared mouse/keyboard/clipboard across native and remote-presented windows.
3. A user-authorized Shared Workspace visible and editable on both PCs.
4. A **true Windows extended display**, backed by a supported IDD, from JACK to the B monitor.
5. **Interactive live window portals** for applications and browser sessions originating from either PC. Do not substitute URL sending, tab re-opening, screen mirroring, or Chrome account sync for this requirement.
6. All functions remain active in the same connected session, even during file transfers, window movement, and video streaming.

## Important physical/OS constraints
A process remains on its origin OS. When crossing computers, show a low-latency, interactive remote-window proxy with input forwarded to the process owner. JACK-owned windows on B's extended display remain ordinary local JACK windows. A single physical monitor cannot display two full desktops in the same pixels: coordinate the JACK receiver with optional B native-window overlays. Respect capture permission, UAC/UIPI, DRM, driver signing and protected content.

## Working method
- Start with **Phase 0 feasibility spikes**: real IDD extended display, live B-window proxy on JACK, concurrent focus ownership. These are highest risk; first verify on the two available fixtures, then expand to version-specific backends and mixed-version compatibility tests across the declared support matrix.
- Then execute Phases 1–5 in the full spec, producing working increments and test logs.
- Preserve clean modules: Tauri/React UI, Rust agent/session/security/files/transport, C++ Windows input/graphics/IDD, browser extension + native host, installer and tests.
- Use strongly typed messages, authenticated connections, explicit consent, scoped filesystem permissions, and backpressure/QoS.
- Do not claim a tool or native API accomplishes something it cannot. No OS-security bypass and no dummy success UI.
- Run available tests and builds after each phase. Maintain `docs/IMPLEMENTATION_STATUS.md`: done, partial, blocked, proof links, and next steps.
- If hardware, signing, or environment blocks a real feature, report the concrete blocker and a technically valid implementation path. **Do not silently relax any of the four requirements.**
- Do not equate “runs on Windows” with “fully compatible.” Detect OS build, architecture, APIs, driver/signing state, and runtime prerequisites; version-specific implementations must preserve the same four-feature concurrent behavior. Keep older Windows releases in the engineering target until feasibility evidence establishes their exact support path.

## Hardware test fixtures
- JACK: Windows 11, initial LAN address `192.168.1.5`.
- DESKTOP-T13BKT8: Windows 10, initial LAN address `192.168.1.10`.
- Use auto-discovered identities in code, never hardcode these values.
- No special cable, no cloud account, and no user-managed third-party software setup.

## Definition of Done
Pass AT-01 through AT-08 on physical hardware; demonstrate live cross-OS Chrome window portal **while** the extended display, shared files, and mouse/keyboard routing continue functioning. Then pass the same concurrent acceptance matrix on every Windows version/architecture group claimed as supported, including mixed-version pairs. Windows 10/11 fixtures alone do not establish broad Windows compatibility. Provide reproducible build/install instructions, tests, per-version performance logs, honest limitations, and source code. Unit tests alone are insufficient.

## First action
1. Create the monorepo and put the supplied spec at `docs/PRODUCT_SPEC.md`.
2. Record a feasibility plan and Windows build prerequisites.
3. Implement Phase 0 proofs; report exact results before proceeding.
