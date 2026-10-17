# Phase 0 Feasibility Plan

**Status:** Phase 0 gate is **NOT PASSED**. The JACK local per-window Windows.Graphics.Capture smoke subtest builds and captures a frame ([evidence](phase0-evidence/wgc-capture-smoke.md)); this is only one component of 0C. The IDD remains unsigned/unloaded, and B-origin portal, unified focus/input, and coexistence are blocked/unverified. No Phase 0 acceptance proof is reported as passed.

## Gate

Demonstrate on JACK (Windows 11) and DESKTOP-T13BKT8 (Windows 10) that the following are simultaneously viable in one paired session:

1. A genuine Windows extended display registered by a UMDF IddCx IDD on JACK and streamed over the LAN to B.
2. A selected B-origin Chrome window captured as a live window surface on JACK, preserving the exact originating browser state, with pointer and keyboard input returned to that same process.
3. One ownership/focus router controlling JACK's native desktop, JACK's virtual display at B, a B-native overlay, and the B-window proxy, without stopping the display receiver or disabling either peer's agent.

The gate is binary per mechanism: collect machine/OS/build identifiers, signed build/package hashes, exact setup and commands, traces/logs, a short redacted recording where practical, measured decoder FPS and input-action latency, and observed limitations. Mark each result pass, fail, or blocked. Do not proceed into product UI polish or claim an architecture proof if any required path is substituted with screen mirroring, URL reopening, or an unrelated mouse-sharing utility.

## Spike sequence

### 0A — Toolchain and fixture inventory

- Confirm both physical fixture identities and Windows editions/builds; collect GPU, display topology, network adapter/band, and driver-signing policy state.
- Install/locate Visual Studio 2022 with Desktop development with C++, Windows 10/11 SDK, a compatible WDK/IddCx toolset and UMDF components, MSVC, CMake, Rust MSVC toolchain, Node/npm, and WebView2 runtime/build prerequisites for the eventual Tauri shell.
- Confirm administrator access for isolated driver installation, test-signing strategy, Secure Boot state/policy, and a recoverable display rollback path. Do not disable signature enforcement or alter Secure Boot on the user's daily systems as an implicit setup step.
- Capture actual LAN link quality and verify peer discovery without relying on fixture IP addresses.

### 0B — IDD/extended display

- Start from Microsoft's official IndirectDisplay sample and record its license and upstream commit. Build a minimal UMDF IDD with a monitor mode and swapchain processing; do not ship the sample unchanged.
- In a dedicated test setup, install a correctly test-signed package through the supported Windows driver workflow. Verify JACK Display Settings enumerates a second display and Windows topology is **Extend**.
- Send swapchain frames through a separate user-session transport/encoder bridge to B; the UMDF driver runs in Session 0 and must not host UI/window APIs. Verify receiver decode, unplug/reconnect, safe detach, and no stranded topology.
- Record whether the available signing route allows the test while preserving the intended Secure Boot policy. Record the production signing/HLK path separately.

### 0C — B-origin live window portal

- On B, explicitly select and consent to a Chrome top-level window. Use Windows.Graphics.Capture `CreateForWindow` where supported; keep Chrome's process and page session on B.
- Stream captured window frames to a movable proxy on JACK. Map proxy click, scroll, keyboard, focus, resize and minimize events back to B's existing HWND/process. Confirm unsaved page state and scroll position survive; do not navigate/relaunch the URL.
- Report protected/unsupported content precisely. Test a non-Chromium window as a second whole-window case. Capture consent and visible active-capture indicator are required.

### 0D — Concurrent ownership and QoS

- With JACK's extended display receiver still streaming, place a B-native window over part of B's receiver and place the B-origin proxy on JACK.
- Exercise pointer/keyboard focus across JACK native, virtual-display receiver, B native overlay, and remote proxy. Confirm a single owner for every hit-tested surface, balanced key/button release after disconnect, and a local recovery hotkey.
- Run a sustained file-like bulk network load while display and portal streams and control packets run; measure p50/p95 action latency, FPS, queue depth and drops. The later file implementation can replace the load generator, but this spike cannot imply file-service completion.

## Technical findings to validate during spikes

- IddCx is the supported Windows model for a virtual/remote display. Microsoft's [IDD overview](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview) describes a third-party UMDF driver that reports monitors/modes and processes desktop images; the IDD runs in Session 0. Keep user-session UI and network/session coordination outside the driver.
- Windows.Graphics.Capture provides user-consented capture APIs; per-window capture uses `IGraphicsCaptureItemInterop::CreateForWindow` where supported ([capture overview](https://learn.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture), [CreateForWindow](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow)). Input forwarding uses supported APIs and does not bypass [SendInput UIPI limits](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput); elevated/secure desktop input is out of scope unless separately supported and reviewed.
- Driver development signing and retail distribution are distinct. Development can use an isolated test-signed package; production requires an appropriate Microsoft signing program and validation. Attestation signing is a testing route with restrictions and does not itself establish HLK compatibility or retail Windows Update distribution.
- These API-level facts do not establish hardware compatibility, acceptable latency, driver stability, simultaneous focus correctness, or a successful two-PC run. Only the physical spikes do.

## Windows build prerequisites

Required before the spike build:

- Windows 11 JACK and Windows 10 B, both x64, user-session accounts available, and a trusted common LAN.
- Visual Studio 2022 Build Tools/IDE with MSVC C++ desktop workload, Windows SDK, and a WDK version compatible with the targeted OS and IddCx sample. Microsoft's current compatibility table lists WDK 26100.6584 with VS 2022; SDK and WDK build numbers must match. Install the matching WDK Visual Studio extension/build integration and UMDF/driver packaging components ([supported kits](https://learn.microsoft.com/en-us/windows-hardware/drivers/other-wdk-downloads), [kit versioning](https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk)).
- CMake and Windows SDK tooling (`msbuild`, `cl`, `inf2cat`, `signtool`, `pnputil`, driver verifier/debug tools as appropriate).
- Rust stable MSVC target (`x86_64-pc-windows-msvc`) and Cargo for the agent; Node.js/npm and Tauri's Windows prerequisites for the later desktop UI; WebView2 runtime/SDK as required by the chosen Tauri release.
- Administrator access on a dedicated/recoverable test machine for driver installation and display rollback. Test certificate/policy configuration is explicit and logged. Never use F8 enforcement disable as a product install path.
- For release readiness, a registered Hardware Developer Center account and eligible signing credentials/submission path; HLK hardware and test logs for the intended certified distribution path. Confirm exact current program requirements before release.

## Exit evidence

Store spike source/build scripts under `spikes/phase0/`, and per-run artifacts under `docs/phase0-evidence/` (hardware inventory, steps, logs, signed package identity, telemetry, known limitations). Update [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) with only evidenced results. If a proof is blocked by missing signing, toolchain, or hardware, record the missing prerequisite and the next technically valid path; keep the gate closed.

Current observed Phase 0 state is summarized in [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md); no hardware acceptance criterion has passed. Local capture source and evidence are under `native/windows-bridge/capture-smoke/` and `docs/phase0-evidence/wgc-capture-smoke.md`.

