# Live LAN observation — 2026-10-18

## Observation

On JACK (`192.168.1.5`), the running `crosswarp-desktop.exe` process (PID 28376, started 06:00:25 local time) owned an established TCP connection to `192.168.1.10:49843` and a listening socket on `0.0.0.0:4594`. ICMP to `192.168.1.10` also succeeded. The process path was `apps/desktop/src-tauri/target/release/crosswarp-desktop.exe`; its file size was 12,310,528 bytes. The separately built protocol-v4 executable is 12,303,360 bytes and is not the running process.

This confirms a live TCP connection owned by the local CrossWarp process to the second fixture. I also captured and visually inspected the running CrossWarp window through the local WGC smoke tool. Its UI showed **“Trusted peer · encrypted transport active”** and **“Both users approved”** for the authenticated peer. It explicitly said input, display, and live-window services were not active; the peer Shared Workspace view said the peer must grant a folder before it appears. This verifies a live trusted encrypted app session, but it does not verify a file operation or any of the four feature acceptance tests. The locally retained screenshot is `artifacts/crosswarp-live-capture-2026-10-18.bmp` (5,875,254 bytes, SHA-256 `2D3485A2DC4E78AC3F4D22A928FE23652409B0836764A1652BC94B8650B6663D`); it remains in the ignored artifacts directory and is not published because it contains device/network identifiers. The active process was left running to avoid interrupting the user's session.

## Local checks rerun

The following commands passed on JACK on 2026-10-18:

- `cargo test --workspace`: 59 passed.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`: 3 passed.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings`: passed.
- `npm run typecheck` and `npm run web:build` from `apps/desktop`: passed.
- CMake Release build and CTest for `native/windows-bridge/input-bridge`: passed (1 contract test).
- Updated `native/windows-bridge/capture-smoke` Release build: passed. Runtime captured 8 visually distinct animated-window frames at 782×516 in 0.333878 seconds (23.96 frames/s for this short diagnostic burst); screenshots and hashes are in [the burst capture evidence](../phase0-evidence/wgc-burst-capture.md).

These are local build/contract checks. They do not exercise Windows input suppression/injection, driver installation, a cross-PC file operation, a remote window proxy, or all four services simultaneously.

## Remaining live validation

The open JACK process predates the protocol-v4 artifact. The current source and v4 artifact cannot be assumed to match it, and the peer's exact installed build cannot be read from the available local shell. No authorized remote command or share access is configured for DESKTOP-T13BKT8. The active connection was not restarted or upgraded. Two-PC feature testing still requires both endpoints to run a compatible current build and visible confirmation/operation in the app; physical AT-01 through AT-08 remain unpassed.
