# Phase 1 evidence: native input coordinate mapping

**Result:** local conversion contract implemented and unit-tested. This is not shared input acceptance.

## Change

The Win32 Raw Input event structure now includes the Windows screen-space cursor position captured at event time and a validity flag. The Rust Windows adapter maps pointer motion, buttons, and wheel events to those screen coordinates, retains wheel direction and extended-key state, and rejects pointer events if the native position is unavailable. Raw relative deltas remain in the native record for diagnostics/future pointer processing. Wheel events hit-test the surface under the pointer without changing keyboard focus. That wire schema change is recorded as protocol version 4 so v3 peers are rejected. `InputSessionController` combines the ownership router and ordered sender: local-owner events are returned for local handling, remote-owner events are sent on the encrypted input lane, unknown owners are rejected, and key-up remains bound to the owner that received key-down.

## Verification

- `cargo test --workspace`: **59 passed, 0 failed** on the JACK Windows 11 x64 development host. New cases cover move/button/wheel event-time coordinates, horizontal wheel direction, extended key-up state, missing-coordinate rejection, hover routing that preserves existing keyboard focus, and local-versus-remote dispatch plus balanced release over an authenticated encrypted stream.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`: **3 passed, 0 failed**; Tauri Clippy with warnings denied passed.
- `cmake --build artifacts/input-bridge-build --config Release --parallel 4`: passed; the updated C ABI compiled and linked.
- `ctest --test-dir artifacts/input-bridge-build -C Release --output-on-failure`: **1/1 passed**. This existing native test validates invalid injection arguments; it does not synthesize input.

## Remaining work

The native callback is not integrated with `InputSessionController`; the desktop has no live surface registration or consent controls for shared input; Raw Input does not suppress local delivery, so a suppressible ownership-aware capture mechanism is required before remote-owned input can be safely forwarded. A receiving-side encrypted input consumer and `SendInput` path are not connected. The local diagnostic remains observational. No cross-PC input behavior is claimed, and AT-04 remains unpassed.
