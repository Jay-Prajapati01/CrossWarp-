# Phase 1 evidence: shared-session identity package build

**Result:** the shared-session-identity source compiled as a Windows x64 Tauri release executable and NSIS installer. The package is unsigned, has not been installed, and is not a complete product release.

## Reproduction

From `apps/desktop` in PowerShell:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
$env:CARGO_TARGET_DIR = "J:\cw"
npm run build
```

The first attempt with a deeply nested target path failed in the MSBuild CMake scratch test (`FTK1011`, could not create a file-tracking log). Repeating with the short `J:\cw` target path resolved that issue. Vite production build, the optimized Rust build, and NSIS bundling all completed.

## Artifacts

| Artifact | Size | SHA-256 | Signature |
|---|---:|---|---|
| `artifacts/session-identity-build/release/crosswarp-desktop.exe` | 12,304,384 bytes | `47B535EA63656988700C780FC76F0236920FA6E47300C1BDEA69DC0CFB605A0D` | Not signed |
| `artifacts/session-identity-build/release/bundle/nsis/CrossWarp_0.1.0_x64-setup.exe` | 2,780,267 bytes | `6D2F6CB64E2E91EB6C30E2FF091628016B72407BA091B39AAAEF3C975F8F1DFE` | Not signed |

`Get-AuthenticodeSignature` reported `NotSigned` for the installer. The current wire protocol remains v4; this increment changes local session diagnostics and uses the existing handshake transcript, without adding a wire field. The package has not been launched or installed, and neither PC has been updated to it. Driver signing/install, remote window rendering, input routing, and four-service physical acceptance remain outstanding.

The test results for this source are recorded in [shared session identity evidence](session-identity.md).
