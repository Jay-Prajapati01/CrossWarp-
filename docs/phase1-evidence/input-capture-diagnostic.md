# Local input capture diagnostic

**Status:** Desktop opt-in is implemented and the Windows application compiles with the native bridge linked. No live user-input capture result has been recorded yet. This is a local diagnostic, not shared input.

## Behavior

- The user must explicitly confirm a disclosure before `start_input_capture_diagnostic` starts the Raw Input sink.
- While it runs, captured event structs are briefly buffered in memory and the worker increments only aggregate counters for pointer movement, pointer buttons, wheel events, and key transitions. Events are not serialized, written to disk, or sent to a peer.
- The user can stop capture in the UI. Dropping the capture owner unregisters mouse and keyboard Raw Input devices.
- Raw Input observes events but does not suppress them. They continue to reach local applications. Avoid entering sensitive text during the diagnostic.
- No test has yet started this diagnostic and observed deliberate physical input in the running desktop UI. The UI's displayed counter is not runtime proof until that test is done.

## Verification

- `cargo test --workspace`: 55 passed.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml`: 3 passed.
- Workspace and Tauri Clippy with `-D warnings`, Rust formatting, and `npm --prefix apps/desktop run typecheck`: passed.
- `cmake --build artifacts/input-bridge-build --config Release --parallel 4`: passed; `ctest --test-dir artifacts/input-bridge-build -C Release --output-on-failure`: 1 passed. The native capture thread now creates its Windows message queue before signaling ready, avoiding an immediate-stop startup race.
- `npm --prefix apps/desktop run build`: passed; the fresh x64 release app and NSIS installer were produced.
- Installer installation, live input observation, transport forwarding, and two-PC routing are not verified here.

## Remaining input-sharing work

The diagnostic does not solve surface-aware focus ownership or duplicate local input. Production input sharing still requires an approved ownership model, consent on both PCs, safe suppression only over remote-owned surfaces, authenticated input-lane routing, native injection subject to UIPI, and balanced release/recovery behavior. Do not count the diagnostic as an integrated feature.

Installer SHA-256: 53AD203F3C5BF536E0AC0F6B57C69C2736A805563AFA4A4557DC7F94CCBE95C3. The release app was launched successfully as process 28376 with window title CrossWarp; it has been left open for the requested manual diagnostic check.
