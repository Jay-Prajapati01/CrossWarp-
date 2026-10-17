# Codex instructions — Unified Cross-PC Workspace

## Mission
Build the Windows-first Unified Cross-PC Workspace described in `docs/PRODUCT_SPEC.md`. Read the entire product specification before modifying or generating application code. The four features are **concurrent requirements**, not mutually exclusive modes.

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
- Start with **Phase 0 feasibility spikes**: real IDD extended display, live B-window proxy on JACK, concurrent focus ownership. These are highest risk; verify on Windows 10/11 before spending time on visual polish.
- Then execute Phases 1–5 in the full spec, producing working increments and test logs.
- Preserve clean modules: Tauri/React UI, Rust agent/session/security/files/transport, C++ Windows input/graphics/IDD, browser extension + native host, installer and tests.
- Use strongly typed messages, authenticated connections, explicit consent, scoped filesystem permissions, and backpressure/QoS.
- Do not claim a tool or native API accomplishes something it cannot. No OS-security bypass and no dummy success UI.
- Run available tests and builds after each phase. Maintain `docs/IMPLEMENTATION_STATUS.md`: done, partial, blocked, proof links, and next steps.
- If hardware, signing, or environment blocks a real feature, report the concrete blocker and a technically valid implementation path. **Do not silently relax any of the four requirements.**

## Hardware test fixtures
- JACK: Windows 11, initial LAN address `192.168.1.5`.
- DESKTOP-T13BKT8: Windows 10, initial LAN address `192.168.1.10`.
- Use auto-discovered identities in code, never hardcode these values.
- No special cable, no cloud account, and no user-managed third-party software setup.

## Definition of Done
Pass AT-01 through AT-08 in `docs/PRODUCT_SPEC.md` on physical hardware; demonstrate live cross-OS Chrome window portal **while** JACK's extended monitor, shared files, and mouse/keyboard routing continue functioning. Provide reproducible build/install instructions, tests, performance logs, honest limitations, and source code. Unit tests alone are insufficient.

## First action
1. Create the monorepo and put the supplied spec at `docs/PRODUCT_SPEC.md`.
2. Record a feasibility plan and Windows build prerequisites.
3. Implement Phase 0 proofs; report exact results before proceeding.
