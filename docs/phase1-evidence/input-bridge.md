# Win32 input bridge evidence

**Status:** Native capture/injection bridge compiles and links into the Windows Rust agent. The desktop does not yet start capture or route/inject events during a session; no physical input-sharing proof is claimed.

## Implementation

- `native/windows-bridge/input-bridge` creates a message-only Raw Input sink for mouse and keyboard under an explicit start/stop API. It does not suppress normal Windows input or install a process-wide hook.
- The bridge exposes relative/absolute pointer movement, five mouse buttons, wheel, and virtual-key transitions through a C ABI.
- Injection uses `SendInput` and has no integrity-level bypass. UIPI, UAC secure desktop, elevated-process boundaries, and protected sessions remain limitations.
- `crates/agent/build.rs` compiles the native static library on Windows targets. `crates/agent/src/windows_input.rs` wraps the ABI and places input into a bounded 4096-event queue. Queue overflow is explicit; the integration caller must stop routing and release held inputs.
- The wrapper is not yet connected to Rust `InputRouter`, focus ownership, transport messages, session consent UI, or disconnect recovery.

## Checks on JACK

- CMake/Visual Studio 2022 x64 configure and release build succeeded with Windows SDK 10.0.26100.0.
- `ctest --test-dir artifacts/input-bridge-build -C Release --output-on-failure`: **1 passed**. The contract test checks invalid events and deliberately injects no input.
- `cargo test --workspace`: **39 passed, 0 failed**, including Rust-side input validation and the existing authenticated RPC test.
- Workspace and Tauri Clippy, Rust formatting, Tauri Rust check, frontend typecheck, and production Tauri build passed.
- The built executable launched and responded as process **PID 30056**.

## Not tested

No Raw Input capture was started, no `SendInput` event was sent to the user's desktop, and no cross-PC input was exercised. The bridge test does not prove correct pointer ownership, key balancing across a real disconnect, high-integrity process behavior, or simultaneous file/display/window operation.
