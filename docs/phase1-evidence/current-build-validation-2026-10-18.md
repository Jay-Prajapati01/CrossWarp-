# Current source validation and installer

**Date:** 2026-10-18 (Asia/Calcutta)  
**Source commit:** `be1033c1ee96d307ca19abebc5211c8b86c3fa89`  
**Git state:** local `main` is 15 commits ahead of `origin/main`; no push was made.

## Automated checks run against this checkout

| Check | Result | Scope and limit |
|---|---:|---|
| `cargo test --locked --workspace` | 65 passed, 0 failed | Rust agent unit/loopback tests; no physical peer acceptance |
| `cargo test --locked --manifest-path apps/desktop/src-tauri/Cargo.toml` | 7 passed, 0 failed | Desktop command and validation tests; no UI installation/runtime test |
| `npm run typecheck` | Passed | React/TypeScript static check |
| `npm run web:build` | Passed | Vite frontend production bundle |
| `cargo fmt --all -- --check` | Passed | Rust formatting check |
| Agent and Tauri Clippy with `-D warnings` | Passed | Lint checks over workspace and desktop targets |
| CMake Release build + CTest for `native/windows-bridge/input-bridge` | 1 passed, 0 failed | Native bridge contract test; deliberately does not inject input into the desktop |
| `npm run build` | Passed | x64 Tauri NSIS package build; not installed or launched |

The Cargo executable was invoked by absolute path from `C:\Users\jay19\.cargo\bin\cargo.exe` because that directory is not currently on this shell's `PATH`.

## Installer artifact

- Path: `J:\cw\release\bundle\nsis\CrossWarp_0.1.0_x64-setup.exe`
- Size: 2.78 MiB (2,914,304 bytes)
- SHA-256: `7F3CF5196A1149E119868FEBEEBB61E7E645091289BDEFDE3512363B2CFC3C8F`
- Authenticode: `NotSigned`
- Target: Windows x64

This is a development test package. It is not a signed production installer and does not install, sign, or load the IDD. It has not been installed on either physical PC. Wire compatibility is protocol v6, so both endpoints must use a matching build before attempting a paired portal test.

## Physical acceptance

No AT-01 through AT-08 acceptance test was run in this validation. The second fixture is reachable at the network layer, but this shell still has no authorized remote command/deployment route or share enumeration access. The four-feature simultaneous acceptance remains **0/4**; these results prove local source/build health only.
