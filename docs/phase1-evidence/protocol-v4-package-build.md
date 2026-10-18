# Phase 1 evidence: protocol-v4 Windows package build

**Result:** current source built successfully as a Windows x64 release executable and NSIS installer. The packages are unsigned and have not been installed or run. This is build evidence only, not feature or hardware acceptance.

## Reproduction

From `apps/desktop` in PowerShell, use a separate Cargo target directory so an already-running older CrossWarp process cannot lock the release executable:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "J:\TECH STARTUP GOAL\Unified Connected system\artifacts\protocol-v4-build"
npm run build
```

The Vite web build and optimized Rust release build completed; Tauri produced the NSIS x64 bundle. Source commit: `eb58ab39fec19d4d106531eb13af1c7406af44e5` (includes wire protocol v4 and agent-side ownership controller).

## Artifacts

| Artifact | Size | SHA-256 | Signature |
|---|---:|---|---|
| `artifacts/protocol-v4-build/release/crosswarp-desktop.exe` | 12,303,360 bytes | `29A44B3D2241FEEDCDA7F15A11D8C84E28F805E4F0F7CDBD3AB2DDEEE571CB81` | Not signed |
| `artifacts/protocol-v4-build/release/bundle/nsis/CrossWarp_0.1.0_x64-setup.exe` | 2,781,101 bytes | `EC52EFF200C157A45874678CF7ACF121671D1C1C62062CF22FB8B2DAA09E134E` | Not signed |

Both files report product/file version `0.1.0`. An earlier attempt to build into the default target failed because the user-facing protocol-v3 app process (PID 28376) held the old `crosswarp-desktop.exe` open. The alternate target completed without closing or replacing that running app.

## Validation and limits

- Vite production build: passed.
- Rust release compile and Tauri packaging: passed.
- Agent tests: 59 passed; Tauri tests: 3 passed; agent/Tauri Clippy and Rust formatting passed on this source.
- Package signature: absent. Driver is not bundled, signed, installed, or loaded.
- Installer was not executed; the new executable was not launched. Both physical PCs have not been updated to protocol v4. No pairing, input, file, display, portal, or concurrent physical acceptance is claimed.

The current user-facing app remains the earlier executable until the user installs/runs this package. See [implementation status](../IMPLEMENTATION_STATUS.md) and [release guide](../RELEASE_AND_INSTALLATION.md).
