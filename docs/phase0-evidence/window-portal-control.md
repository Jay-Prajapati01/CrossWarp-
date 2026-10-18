# Interactive window proxy control increment

**Date:** 2026-10-18  
**Protocol:** 6  
**Scope:** Source and receiver implementation plus local validation; not physical acceptance.

## Implemented

- The source user's explicit share confirmation scopes consent to the selected visible HWND and forwarding pointer/keyboard events. The receiver must separately approve the incoming live window before the preview or control UI appears.
- The proxy UI sends normalized pointer movement, pointer buttons, wheel, and keyboard transitions over authenticated WindowControl frames. The request is bound to the currently displayed source stream and is rejected if the frame is stale.
- The source checks that the stream ID matches its active consented HWND before passing events to the native bridge.
- The native bridge rejects invalid, hidden, minimized, or invalid-handle targets, maps normalized pointer coordinates into the HWND client rectangle, requests foreground ownership, verifies it, and calls ordinary `SendInput`. UIPI, foreground restrictions, secure desktops, and protected content are not bypassed.
- Protocol version is 6 because adding control actions is incompatible with the prior stop-only WindowControl schema.

## Local validation

- Rust agent: 65 tests passed; Tauri desktop: 6 tests passed.
- Agent and Tauri Clippy with warnings denied passed.
- Native CMake Release build passed; CTest contract: 1/1 passed (including invalid target/action rejection without injecting desktop input).
- Frontend TypeScript check and Vite production build passed.
- A v6 NSIS test installer was built at `artifacts/window-portal-v6/CrossWarp_0.1.0_x64-setup.exe`: 2,894,519 bytes; SHA-256 `559A6A36EB226A4371931568D35D77DDC33C629E470F79AE4619F858EC95DC55`; Authenticode `NotSigned`.\n- No user input was synthesized during validation.

## Not proven / known follow-up

- No paired physical PCs have run the matching v6 build; browser text entry, scrolling, pointer mapping, latency, or event ordering are not physically verified.
- No display driver or shared input feature is integrated; concurrent four-feature acceptance remains 0/4.
- The current implementation can still lose a release event on abrupt network teardown. Add held-input reconciliation and a remote rescue path before production acceptance.
- Foreground activation may be denied by Windows; in that case the bridge refuses injection. Elevated/UAC/secure desktop and protected windows remain unsupported.
- Broad Windows-version support and mixed-version pairs remain unverified.
