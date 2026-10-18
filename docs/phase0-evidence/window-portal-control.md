# Interactive window proxy control increment

**Date:** 2026-10-18  
**Protocol:** 6  
**Scope:** Source and receiver implementation plus local validation; not physical acceptance.

## Implemented

- The source user's explicit share confirmation scopes consent to the selected visible HWND and forwarding pointer/keyboard events. The receiver must separately approve the incoming live window before the preview or control UI appears.
- The proxy UI sends normalized pointer movement, pointer buttons, wheel, and keyboard transitions over authenticated WindowControl frames. The request is bound to the currently displayed source stream and is rejected if the frame is stale.
- The dashboard preview panel can be moved and resized. A new command also creates a separate resizable native Tauri desktop window bound to the approved peer and stream; its image renderer maps clicks through letterboxing to the captured window and releases held events on focus loss. This native-window path compiles but has not been manually opened or tested.
- The source checks that the stream ID matches its active consented HWND before passing events to the native bridge.
- The native bridge rejects invalid, hidden, minimized, or invalid-handle targets, maps normalized pointer coordinates into the HWND client rectangle, requests foreground ownership, verifies it, and calls ordinary `SendInput`. UIPI, foreground restrictions, secure desktops, and protected content are not bypassed.
- Protocol version is 6 because adding control actions is incompatible with the prior stop-only WindowControl schema.

## Local validation

- Rust agent: 65 tests passed; Tauri desktop: 7 tests passed.
- Agent and Tauri Clippy with warnings denied passed.
- Native CMake Release build passed; CTest contract: 1/1 passed (including invalid target/action rejection without injecting desktop input).
- Frontend TypeScript check and Vite production build passed.
- A v6 NSIS test installer was built at `artifacts/window-portal-v6/CrossWarp_0.1.0_x64-setup.exe`: 2,918,041 bytes; SHA-256 `D3F96AAD1A8EE766D0597564B90E872A760339E20C9D545FF8057B60B7EB10FB`; Authenticode `NotSigned`.
- No user input was synthesized during validation.

## Not proven / known follow-up

- The native portal window creation is compile-tested only; Windows window placement, movement across displays, and its live rendering/input have not yet been run.
- No paired physical PCs have run the matching v6 build; browser text entry, scrolling, pointer mapping, latency, or event ordering are not physically verified.
- No display driver or shared input feature is integrated; concurrent four-feature acceptance remains 0/4.
- Button/key transitions are balanced; the sender queues events in UI order; the receiver caps controls at 180 events per second; stop/disconnect attempts to release held keys/buttons. Verify release behavior on physical PCs and add a local rescue path before production acceptance.
- Foreground activation may be denied by Windows; in that case the bridge refuses injection. Elevated/UAC/secure desktop and protected windows remain unsupported.
- Broad Windows-version support and mixed-version pairs remain unverified.
