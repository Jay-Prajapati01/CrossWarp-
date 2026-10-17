# Protocol-v2 session multiplexer contract

Date: 2026-10-18 (Asia/Calcutta)

## Implemented

- `spawn_session_transport` owns one authenticated encrypted stream and dispatches each encrypted record by an authenticated service tag to four bounded channels: input, window video, extended-display video, and files.
- Shared Workspace RPC now uses the file channel. The desktop retains the other service endpoints for later native input/media integration; those endpoints do not make input or media active by themselves.
- Payload limits and per-service message capacities bound queued data. Input/control receives a smaller payload cap; video producers have nonblocking sends to avoid blocking capture on a full queue. Video receive is best-effort; input and files use lossless backpressure.
- The writer prioritizes input and interactive video, gives display video bounded opportunities while input/window traffic continues, and forces a file turn after at most eight non-file records when file work is queued.
- Session cancellation aborts and closes both transport workers, including when cancellation happens before their supervisor futures are first polled.
- The wire format changed, so the authenticated protocol is version 2 and v1 peers fail the protocol-version check during handshake validation. A regression test verifies the rejection. Both endpoints must run compatible v2 builds.

## Verification

`cargo test --workspace` passed **46 tests**. Added loopback tests verify authenticated encrypted delivery on all four lanes, Shared Workspace RPC while input/window/display frames are concurrently flowing over the same session, file progress under continuous input, display progress under continuous input and window traffic, queue limits, cancellation closure, and old protocol-version rejection. `cargo clippy --workspace --all-targets -- -D warnings` passed. Tauri desktop tests: **2 passed**; Tauri Clippy passed.

## Limits

This is an in-process authenticated loopback proof of the service framing and scheduling contract. No physical peer, native input event, window capture frame, IDD swapchain frame, or codec output has yet been connected to the non-file lanes. The transport is still a single reliable TCP stream: TCP head-of-line blocking and one in-flight encrypted frame can delay interactive packets behind network congestion. It is not a latency, frame-rate, or simultaneous hardware acceptance proof. A low-latency media transport still needs to replace or augment this data plane before Phase 0/AT-05 can pass.
