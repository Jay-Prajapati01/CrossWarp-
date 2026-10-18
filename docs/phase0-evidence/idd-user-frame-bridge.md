# IDD user-mode frame bridge — local implementation evidence

**Date:** 2026-10-18 (Asia/Calcutta)  
**Scope:** bounded frame IOCTL contract, user-mode client, Rust ABI wrapper, consent-gated display lane.  
**Acceptance status:** partial implementation only; no physical display test passed.

## Implemented

- Added a shared IOCTL header for capture enable/disable and copying the newest frame.
- The IDD driver registers a user-mode device-I/O callback, accepts one authorized client at a time, disables capture and scrubs snapshots when that client closes, and restricts the device interface to SYSTEM, Administrators, and interactive users with read/write access.
- Added a C++ SetupAPI client and Rust wrapper. Frame dimensions and the bounded BGRA buffer are checked before encoding.
- The sender sends a typed `DisplayControl::Request` over the authenticated display service lane and waits for a matching decision before opening the client and enabling capture. The receiver presents the request and rejects pixel frames unless that stream was approved.
- JPEG output is bounded by the encrypted frame payload budget. Wire protocol is v7.

## Local checks

Commands run from the repository root:

- `cargo fmt --all -- --check` — passed.
- `cargo test --workspace` — 67 passed, 0 failed.
- `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` — 8 passed, 0 failed.
- `cmake -S native/idd/tests -B artifacts/idd-frame-buffer-build -A x64`, build Release, and CTest — 1/1 passed.
- `cmake -S native/windows-bridge/idd-client -B artifacts/idd-client-build -A x64`, build Release, and CTest — 1/1 passed.
- `scripts/build-idd-sample.ps1 -Configuration Release` — x64 driver and sample app compile/link passed; Inf2Cat reported no errors or warnings and generated an unsigned catalog.
- Frontend `npm run typecheck` and Vite production build — passed. The first Tauri packaging attempt failed in CMake's compiler smoke test for the window-capture bridge (`FileTracker FTK1011`, missing temporary `.tlog` path) using the long workspace target path. Retrying with `CARGO_TARGET_DIR=J:\cw\crosswarp-v7-target` passed and produced an x64 NSIS installer (2.83 MiB).
- The installer was verified as unsigned (`Get-AuthenticodeSignature`: `NotSigned`), installed silently on JACK with exit code 0, and the installed desktop process remained alive after a five-second launch smoke check. Installer SHA-256: `09D2DB5361B6AC25F9E52520298EAF893644FA39DF425DF2A985830273C46C55`.

## Limits and blockers

- These tests do not open the driver interface, enable real IDD capture, or verify the IOCTL callback under a running driver.
- The IDD package remains unsigned, uninstalled, and unloaded. Secure Boot state is unknown. No signing or boot-security policy was changed.
- No remote display frame was captured or rendered; no Windows 10 runtime behavior or simultaneous four-feature operation was tested.
- The second PC must have this matching v7 installer before protocol-level test results are meaningful. LAN reachability alone is not acceptance evidence.
- All four end-to-end acceptance features remain **0/4 passed**.
