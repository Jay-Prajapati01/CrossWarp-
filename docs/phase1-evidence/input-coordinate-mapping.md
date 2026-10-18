# Phase 1 evidence: native input coordinate mapping

**Result:** local conversion contract implemented and unit-tested. This is not shared input acceptance.

## Change

The Win32 Raw Input event structure now includes the Windows screen-space cursor position captured at event time and a validity flag. The Rust Windows adapter maps pointer motion and buttons to those screen coordinates, retains wheel direction and extended-key state, and rejects pointer events if the native position is unavailable. Raw relative deltas remain in the native record for diagnostics/future pointer processing. No input is suppressed, sent over the session, or injected by this adapter.

## Verification

- `cargo test --workspace`: **57 passed, 0 failed** on the JACK Windows 11 x64 development host. New cases cover move/button event-time coordinates, horizontal wheel direction, extended key-up state, and missing-coordinate rejection.
- `cmake --build artifacts/input-bridge-build --config Release --parallel 4`: passed; the updated C ABI compiled and linked.
- `ctest --test-dir artifacts/input-bridge-build -C Release --output-on-failure`: **1/1 passed**. This existing native test validates invalid injection arguments; it does not synthesize input.

## Remaining work

The adapter is not integrated with `InputRouter`, the encrypted input lane, remote-consent controls, a suppressible ownership-aware capture mechanism, or receiving-side `SendInput`. The local diagnostic remains observational. No cross-PC input behavior is claimed, and AT-04 remains unpassed.
