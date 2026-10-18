# Win32 input bridge

This Windows-only bridge implements a message-only Raw Input sink for mouse and keyboard events and injects a single normalized event through `SendInput`. It does not install a process-wide hook, suppress legacy input, elevate itself, bypass UIPI, or claim that a rejected `SendInput` succeeded.

The host must obtain explicit user consent before calling `uw_input_capture_start`, start capture only while a paired input-sharing session is active, queue callback data quickly, and call `uw_input_capture_stop` during disconnect/consent revocation. The callback runs on the bridge message thread and must not block. `WM_INPUT` packets include only device reports. The desktop's explicit local diagnostic currently starts/stops capture and aggregates event-kind counters in memory; it does not send event content to disk or across the network. The Rust session still needs to apply `InputRouter` ownership, route approved events, and release held keys/buttons on disconnect. That session integration is not included yet. See [diagnostic evidence](../../../docs/phase1-evidence/input-capture-diagnostic.md).

`uw_input_inject` uses relative mouse movement unless `UW_INPUT_FLAG_ABSOLUTE` is set. Absolute coordinates are expected to use Win32's normalized 0–65535 virtual-desktop range. Keyboard events carry a virtual-key code, scan code, and up/down state. UIPI may block injection into a higher-integrity process; Windows may not identify UIPI through `GetLastError`. Elevated desktops, UAC secure desktop, protected sessions, and other Windows security boundaries remain unsupported by this bridge.

## Build and contract check

```powershell
cmake -S native/windows-bridge/input-bridge -B artifacts/input-bridge-build -G "Visual Studio 17 2022" -A x64
cmake --build artifacts/input-bridge-build --config Release --parallel 4
ctest --test-dir artifacts/input-bridge-build -C Release --output-on-failure
```

The contract test validates invalid arguments only and deliberately does not synthesize input into the user's desktop. A separate explicit-consent interactive test is required to validate Raw Input delivery and `SendInput` behavior on a recoverable test account. The user's physical diagnostic result remains pending until reported. This bridge alone does not satisfy shared mouse/keyboard acceptance.
