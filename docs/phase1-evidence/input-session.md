# Typed input-session contract

**Status:** Implemented and unit-tested as a protocol/routing component. Not integrated into live desktop input capture or native injection.

## Contract

- Each serialized input frame contains the active session ID, a monotonically increasing sequence number, the destination device ID, and a typed input event.
- The sender accepts only the multiplexer input lane and advances its sequence only after a successful bounded-lane send.
- The receiver rejects oversized or malformed frames, unexpected sessions/devices, replay or out-of-order sequence numbers, and invalid or unbalanced key/button transitions.
- Disconnect cleanup emits key-up and button-up events for all currently held inputs.
- Local event ownership routes corresponding key/button releases to the same surface that received the press, even if focus changes in between.

## Verification

`cargo test --workspace` passed **49 tests** after this increment. New tests verify ordered key down/up frames over the authenticated encrypted input lane, rejection of replay/wrong-target/unbalanced events, and disconnect release plus original-owner routing. `cargo clippy --workspace --all-targets -- -D warnings` passed. Tauri tests and Tauri Clippy also passed with the new agent module.

## Limits

This is not proof of unified live input. The desktop does not start a global capture source or require/collect the runtime consent needed to route events. The C++ Raw Input/SendInput bridge remains disconnected. Raw Input alone does not suppress local events, and no OS security boundary is bypassed. No physical peer session has exercised routed input concurrently with display, portals, and file operations. Those remain required before acceptance.
