# CrossWarp Windows NSIS installer smoke check

Date: 2026-10-18 (Asia/Calcutta)

## Build

- Command: `apps/desktop/node_modules/.bin/tauri.cmd build -b nsis`
- Result: success; after the protocol-v2 session-multiplexer changes, Tauri produced `apps/desktop/src-tauri/target/release/bundle/nsis/CrossWarp_0.1.0_x64-setup.exe` (2,726,786 bytes).
- SHA-256: `950347C1E9F0790BCB532C166777EE6DE1DE9E352347889916EAFC0D6FD19D03`.
- Authenticode: `NotSigned`. The artifact is a development smoke package, not a production-signed release.
- Pinned NSIS 3.13 archive SHA-1: `DB14F8AF2EA346B786C6AE19343F61AAF5E09B8E`.
- NSIS executable SHA-1: `6D45D4E8CCF18FE1FDAC9164058F356102A2ECF2`.
- Tauri NSIS helper DLL SHA-1: `75197FEE3C6A814FE035788D1C34EAD39349B860`.

The NSIS archive and helper DLL were fetched from the release URLs pinned in the Tauri bundler and verified against those hashes before placing them in Tauri's local cache. No installer dependency or generated binary was added to the source repository.

## Install / launch / uninstall

- Installed silently as the current user to `%TEMP%\CrossWarp-Installer-Smoke` with NSIS `/S` and `/D=...`; installer exit code `0`.
- Installed executable: `crosswarp-desktop.exe` (12,094,976 bytes).
- Launched the installed executable: PID `20700`, visible title `CrossWarp`, nonzero window handle, and `Responding=True` after four seconds.
- Closed the app via its main window and ran the installed `uninstall.exe /S`; uninstaller exit code `0`.
- Verified the install directory was removed after uninstall.

This verifies the desktop installer lifecycle on the Windows 11 development host only. It does not install or sign the IDD driver, prove Windows 10 or broad-version compatibility, or pass any cross-PC acceptance test.
