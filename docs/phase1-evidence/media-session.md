# Typed media-session contract

**Status:** Implemented and unit-tested at the agent transport boundary. No capture or playback integration is claimed.

## Contract

The window-video and display-video service channels accept a typed frame envelope containing session ID, target device, stream ID, sequence, source timestamp, dimensions, codec, key-frame flag, and encoded payload. H.264, HEVC, AV1, and raw BGRA8 payloads are representable. The transport sender verifies the selected service lane and payload bounds, then uses the lossy bounded queue API; queue pressure returns an error so a real-time producer can discard stale frames rather than block input or files.

The receiver validates frame size, dimensions, session, destination, and monotonic sequence/timestamp. Sequence gaps are valid because the video queues intentionally retain only the newest frames. Replayed, reordered, cross-stream, malformed, or wrong-destination frames are rejected.

## Verification

`cargo test --workspace`: **51 passed**. New tests verify forward progress across dropped-frame sequence gaps and rejection of wrong targets, replay, and invalid dimensions. Workspace Clippy passed with `-D warnings`; Tauri tests: **2 passed**; Tauri Clippy passed with `-D warnings`.

## Limits

This does not capture a window or display, encode or decode frames, render a proxy/receiver, negotiate codec support, provide capture consent, or connect the desktop to either video lane. The four features have not been demonstrated operating together on physical PCs. The contract is a foundation for producers/consumers, not evidence of live display extension or window portals.
