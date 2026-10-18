# CrossWarp feature-by-feature status

**As of:** 2026-10-18  
**Release state:** development build; not ready for production or full-feature use.  
**Acceptance summary:** 0 of 4 core capabilities have passed the required simultaneous physical two-PC acceptance test. No AT-01 through AT-08 test has passed on the fixtures.

This is a user-readable summary. Detailed evidence, exact test counts, blockers, and next actions are in [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md). Requirements remain defined by [PRODUCT_SPEC.md](PRODUCT_SPEC.md).

## 1. Shared mouse, keyboard, and clipboard

**Goal:** one physical mouse/keyboard moves across local, extended-display, and remote-window surfaces with correct focus and no duplicate/stuck input; clipboard sharing follows an explicit privacy policy.

**Built:** typed input events, session/device/sequence validation, lane framing, surface ownership model, balanced key/button transitions, disconnect release representation, a Windows Raw Input/SendInput bridge, and a local opt-in Raw Input diagnostic that displays aggregate event-type counts. Native input records now include event-time screen coordinates; a Windows adapter maps move, button, vertical/horizontal wheel, and extended-key records into the typed session contract.

**Verified:** 58 agent tests pass, including native-to-session event conversion and rejection of pointer events without a valid cursor position; native bridge rebuilds and its invalid-argument contract test passes; desktop diagnostic compiles and reports aggregate counters by unit test. See [input coordinate mapping evidence](phase1-evidence/input-coordinate-mapping.md).

**Not built or proved:** live ownership surface registration, input suppression over remote-owned surfaces, event routing from the native callback into the session, remote receiving/injection, consent on both devices for sharing, clipboard integration, real disconnect release delivery, or cross-PC input. The current diagnostic deliberately does not share input and must not be counted as the feature.

**Acceptance:** not passed. Physical diagnostic event counts are also still awaiting a manual run on JACK.

## 2. Shared Workspace

**Goal:** one user-authorized root is browseable/editable on both PCs in the connected session with safe transfer semantics.

**Built:** capability-rooted local storage; path traversal/alternate-stream protections; local folder grant; file/directory listing, range reads, directory creation, uploads, checksum verification, explicit-confirmation replacement, rename/delete confirmation; encrypted bidirectional RPC; remote browse and file actions in the desktop; temporary-file downloads with metadata/digest verification before destination replacement.

**Verified:** filesystem and identity tests, encrypted RPC loopback tests, Tauri download atomic-replacement and corruption-preservation tests, agent/desktop compilation and lint checks.

**Not proved:** pairing, folder grant, browse, upload, download, rename, or delete between the two physical PCs. Conflict handling, multi-user edits, cancellation/resume, and all required UX/acceptance behavior remain incomplete.

**Acceptance:** code foundations are the furthest along, but the required physical concurrent test has not passed.

## 3. True extended display

**Goal:** Windows on the source PC recognizes a real additional logical monitor backed by an IDD; its pixels are presented on the other PC's physical display, while source-owned apps remain ordinary source processes/windows.

**Built:** official Microsoft IddSampleDriver compile/catalog-generation spike and a typed display-video transport lane/envelope.

**Verified:** sample builds in the current WDK/SDK toolchain; the package is unsigned. Display-lane frames cross an in-process encrypted loopback test with file-lane work.

**Not built or proved:** a signed CrossWarp IDD package, installation/load, Secure Boot/signing path, production display adapter, desktop frame production/encoding, receiver decode/render, topology management, or a true added monitor on either fixture.

**Acceptance:** blocked. Do not install the unsigned sample or change Secure Boot/test-signing as a workaround.

## 4. Interactive live window portals

**Goal:** a running application/browser window from either PC appears as an interactive proxy on the other PC; the original process, login, and session stay on its source PC.

**Built:** a local Windows.Graphics.Capture HWND single-frame spike and typed window-video transport envelope/lane.

**Verified:** one local frame capture on JACK was recorded in [WGC evidence](phase0-evidence/wgc-capture-smoke.md); typed media bytes and headers are validated in agent loopback tests.

**Not built or proved:** window selection/consent UI, continuous capture, encoder/decoder, remote rendering, resize/close/focus/window lifecycle, input forwarding, browser-session interoperability, or a live B-to-JACK window proxy.

**Acceptance:** not passed. URL sending, tab reopening, Chrome sync, and whole-screen mirroring do not satisfy it.

## Concurrent-session requirement

The protocol has separate input, window-video, display-video, and file queues in one authenticated session. This is transport groundwork only. The desktop presently consumes the file channel; the other feature producers/consumers are not integrated. No physical test has demonstrated all four operating together during transfer, window movement, and video streaming.

## Compatibility

JACK is Windows 11 x64. DESKTOP-T13BKT8 is the initial Windows 10 peer but has not been remotely inventoried or deployed from this workspace. No Windows release is fully supported. The user wants broad Windows-version coverage; the tested matrix is empty, with API/driver gaps called out in [WINDOWS_COMPATIBILITY.md](WINDOWS_COMPATIBILITY.md).

## Project increment just recorded

The current development increment adds the opt-in local input diagnostic, corrects the capability aggregate count when remote files actually respond, ensures the native message queue exists before Raw Input reports startup, and documents known limitations. It does **not** change the 0/4 end-to-end acceptance result. See [input diagnostic evidence](phase1-evidence/input-capture-diagnostic.md).
