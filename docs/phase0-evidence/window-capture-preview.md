# HWND capture and window-video preview increment

**Date:** 2026-10-18  
**Host:** JACK, Windows 11 Home Single Language, build 26300, x64.  
**Scope:** Local per-window capture and encoding smoke proof; not two-PC acceptance.

## Local source proof

The new static WGC bridge built with Visual Studio 2022/MSVC and Windows SDK 10.0.26100. A visible animated WinForms fixture was captured from the Rust agent wrapper:

    CAPTURED hwnd=0x1E0CCA sequence=1 source=782x516 preview=782x516 jpeg_bytes=24498 sha256=14d30b10a21a4782810e4a332699298cd12fb31fcd274986e1facd14fa7c4c05

The first attempt launched the WinForms fixture host hidden; Windows reported that the target window was not visible and capture rejected it. Re-running with the fixture visible succeeded. That is a test-fixture correction, not a capture fallback.

The desktop now exposes visible-window enumeration, source-side confirmation, a persistent capture worker, a bounded BGRA-to-JPEG encoder, transmission on the authenticated window-video lane, independent receiver approval, and a stop/revoke control. The receive side validates session/destination/order and restricts preview frames to JPEG at or below 3840×2160. Window capture is limited to 20 frames per second before encoding, with a one-frame handoff queue and the encrypted lane's bounded stale-frame policy.

## Source validation

- Rust agent: **65 passed**.
- Tauri desktop crate: **5 passed**.
- Workspace and Tauri Clippy with warnings denied: passed.
- Rust formatting, frontend TypeScript check, and Vite production build: passed.
- CMake Release build of the capture bridge and its smoke executable: passed.
- Current protocol-v5 NSIS test package built: J:\cw-v5-test-build\release\bundle\nsis\CrossWarp_0.1.0_x64-setup.exe, 2,880,675 bytes, SHA-256 7B221E8C773F5CC36523E2244AEF9541FE7273924B0901CEB7278FB17CA55347; Authenticode status NotSigned.

## Not proven

- The package has not been installed/launched on either fixture in this increment.
- No source-to-peer frame delivery, destination consent prompt, remote stop/revoke, or network measurement has been physically tested.
- DESKTOP-T13BKT8 / Windows 10 runtime remains untested and needs a matching protocol-v5 build.
- This is a view-only preview. It is not an interactive window proxy; no remote input is forwarded. True extended display and shared mouse/keyboard remain incomplete.
- The installer and display driver are unsigned. No AT-01…AT-08 acceptance result changes.
