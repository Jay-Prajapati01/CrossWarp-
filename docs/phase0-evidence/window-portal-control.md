# Interactive window proxy control increment

**Date:** 2026-10-18  
**Protocol:** 6  
**Scope:** Source and receiver implementation plus local validation; not physical acceptance.

## Implemented

- The source user's explicit share confirmation scopes consent to the selected visible HWND and forwarding pointer/keyboard events. The receiver must separately approve the incoming live window before the preview or control UI appears.
- The proxy UI sends normalized pointer movement, pointer buttons, wheel, and keyboard transitions over authenticated WindowControl frames. The request is bound to the currently displayed source stream and is rejected if the frame is stale.
- The receiver's proxy panel can be dragged and resized inside the CrossWarp dashboard. Pointer coordinates are normalized against the resized image bounds. Focus loss and pointer cancellation send best-effort release transitions.
- The source checks that the stream ID matches its active consented HWND before passing events to the native bridge.
- The native bridge rejects invalid, hidden, minimized, or invalid-handle targets, maps normalized pointer coordinates into the HWND client rectangle, requests foreground ownership, verifies it, and calls ordinary `SendInput`. UIPI, foreground restrictions, secure desktops, and protected content are not bypassed.
- Protocol version is 6 because adding control actions is incompatible with the prior stop-only WindowControl schema.

## Local validation

- Rust agent: 65 tests passed; Tauri desktop: 7 tests passed.
- Agent and Tauri Clippy with warnings denied passed.
- Native CMake Release build passed; CTest contract: 1/1 passed (including invalid target/action rejection without injecting desktop input).
- Frontend TypeScript check and Vite production build passed.
- A v6 NSIS test installer was built at `artifacts/window-portal-v6/CrossWarp_0.1.0_x64-setup.exe`: 2,903,609 bytes; SHA-256 `79E72E90CE3E643A484D282587214ED9BE2E48F105647F710CED4978A54855D0`; Authenticode `NotSigned`.
- No user input was synthesized during validation.

## Not proven / known follow-up

- The panel is not yet a top-level native window that can be freely dragged across the operating system desktop; current movement is inside the CrossWarp dashboard.
- No paired physical PCs have run the matching v6 build; browser text entry, scrolling, pointer mapping, latency, or event ordering are not physically verified.
- No display driver or shared input feature is integrated; concurrent four-feature acceptance remains 0/4.
- Button/key transitions are balanced; the sender queues events in UI order; the receiver caps controls at 180 events per second; stop/disconnect attempts to release held keys/buttons. Verify release behavior on physical PCs and add a local rescue path before production acceptance.
- Foreground activation may be denied by Windows; in that case the bridge refuses injection. Elevated/UAC/secure desktop and protected windows remain unsupported.
- Broad Windows-version support and mixed-version pairs remain unverified.
