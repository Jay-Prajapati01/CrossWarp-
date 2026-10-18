# Windows compatibility objective and evidence

**Status:** Broad Windows compatibility is a product goal; no Windows release is yet certified for the complete product.  
**Scope update:** The user expanded the original Windows 10/11 fixtures into a Windows-wide compatibility requirement. JACK and DESKTOP-T13BKT8 remain the first physical test pair only.

## Product contract

1. Keep one product, one pairing/session model, and all four concurrent capabilities on every OS/build declared fully supported.
2. Select native implementations by runtime OS build, architecture, API presence, driver state, and permission. Do not switch the user into mutually exclusive modes.
3. A version is fully supported only after AT-01 through AT-08 pass with all four functions active together on that OS and in mixed-version pairings.
4. If a native API is missing, implement and validate a backend that preserves the same user outcome. Do not silently disable extended display, input, files, or a live origin-window portal, and do not substitute URL sharing or whole-desktop mirroring.
5. Publish a compatibility table with exact Windows edition, build, architecture, graphics hardware, driver version, feature results, and test evidence for every release.

## Verified Microsoft API boundaries

| Platform component | Verified minimum / range | Engineering consequence |
|---|---|---|
| `IGraphicsCaptureItemInterop::CreateForWindow` | Windows 10 version 1903, build 18362 | The current per-HWND capture spike cannot be the only live-window capture backend if earlier versions are in the product goal. Implement and validate a distinct per-window fallback before declaring earlier builds full-feature compatible. |
| IddCx 1.11 driver binary | Microsoft documents one IddCx 1.11 IDD binary can run on Windows 10 1803 and later with runtime checks for newer DDIs | One IddCx binary may cover many Win10/11 builds, but the sample is not yet proven or signed/installed here; this does not cover earlier Windows versions. |
| Current sample package declaration | Sample project builds against IddCx 1.6 with minimum required IddCx 1.4 and UMDF 2.25; INF now declares build 17134 (Windows 10 1803) | Compile/link and Inf2Cat pass locally. This is only a candidate package floor; no installation/load/runtime test has passed. |
| Windows.Graphics.Capture family | The `GraphicsCaptureItem` API family starts in Windows 10 1803; the Win32 `CreateForWindow` interop has the later 1903 floor above | API family presence alone is insufficient to promise HWND capture. Probe the exact API and the user-consent/capture behavior. |
| Tauri/WebView2 frontend | Tauri prerequisites say WebView2 is included in Windows 10 1803 and later. Tauri 2.12 release notes say the latest WebView2 installer path dropped Windows 7 | Frontend availability and installer/runtime behavior need their own tested range. Do not infer that this repository's currently locked Tauri dependencies work on a build merely because a current Tauri guide mentions it. |
| Driver signing | Microsoft applies version-specific signing policies; new Windows 10 1607+ kernel driver releases require Microsoft signing through the Hardware Dev Center | Driver support is a separate installer/release gate. Do not bypass Secure Boot or signing policy. Older OS releases require separate signing/package validation. |

Primary references: [CreateForWindow requirements](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow), [IddCx 1.4 multi-version guidance](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/building-iddcx1.4-drivers), [IddCx 1.11 compatibility](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/iddcx1-dot-11-updates), [UMDF version history](https://learn.microsoft.com/en-us/windows-hardware/drivers/wdf/umdf-version-history), [GraphicsCaptureItem API history](https://learn.microsoft.com/en-us/uwp/api/windows.graphics.capture.graphicscaptureitem), [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/), [Tauri 2.12 release notes](https://v2.tauri.app/blog/tauri-2.12/), [driver signing policy](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-requirements--windows-vista-and-later-).

## Current implementation compatibility matrix

“Candidate” means API documentation suggests a possible implementation floor. It is **not** a tested or supported product claim.

| Windows range | Shared UI/session | True extended display | Live window portal | Full four-feature status |
|---|---|---|---|---|
| Windows 11 and Windows 10 1903+ | Current Tauri/Rust build runs on JACK (Windows 11 build 26300); older build coverage is untested | IDD sample compiles and its INF includes these builds; unsigned and unloaded | Local WGC HWND capture smoke ran on JACK only | Candidate target; **not passed** |
| Windows 10 1803–1809 | Not tested | Sample INF now declares build 17134 and its UMDF/IddCx build settings have a documented candidate path; package/load/runtime unverified | `CreateForWindow` unavailable before 1903; a tested alternative backend is required | Backend gap; **not passed** |
| Windows 10 1709 and earlier | Not tested | IddCx minimums and driver compatibility vary by release; no project backend has been validated | Required portal implementation unvalidated | Legacy spike required; **not passed** |
| Windows 8.1, 8, 7, Vista, XP, and earlier | Not tested; frontend/runtime and Rust dependency compatibility must be investigated separately | Current IddCx path does not establish support here; a separate, supported legacy display-driver design would be required | No validated per-window capture backend in this project | No claim; research and feasibility spikes required |
| Windows on ARM / ARM64, 32-bit x86, Windows Server | Not tested | Driver packages and hardware-specific backends need architecture/edition validation | Capture and input adapters need architecture validation | No claim; expand matrix after x64 baseline |

The user asked for broad Windows compatibility; the pre-Windows 10 rows are not dropped from the goal. They are explicit engineering risks because the current official IddCx route and chosen window-capture call do not span every Windows release. A compatible implementation for those rows still has to be discovered and physically validated. If Windows itself provides no supported route for a required behavior on a release, document the precise OS/API limit and a technically valid product path rather than claiming that release works.

## Validation sequence

1. Complete Phase 0 on the existing two fixtures, then add matrix machines or isolated test images for Windows 10 1903, 1809, 1803, 1709, 8.1, 7, and any earlier release that remains technically viable.
2. Compile and run the native API probes on each build. Detect API presence at runtime and persist a diagnostic report; do not trust version strings alone.
3. For every proposed compatibility layer, validate all four requirements in one session and repeat with mixed-version endpoints. Maintain a tested-pair matrix, not just a per-PC checklist.
4. Test installer prerequisites, WebView2 presence, architecture, driver installation/signature, update, rollback, uninstall, and recovery on each declared supported group.
5. Keep all unsupported or untested states visible in the product diagnostics and in `docs/IMPLEMENTATION_STATUS.md`.

## Current limitation

This workspace has no legacy Windows hardware/VM validation, no signed/loaded IDD, and no authorized deployment/command route to the second physical PC at the time this matrix was last updated. The compile/test checks on JACK do not prove cross-version behavior. The user-provided GitHub origin is `https://github.com/Jay-Prajapati01/CrossWarp-.git`. As checked on 2026-10-18 before the current IDD target spike edits, local `main` was 17 commits ahead of `origin/main`; `git ls-remote` reported remote `main` at `90765376697581a20d7b468702286c1e5c42bf7f`. The IDD target spike is currently uncommitted. The current implementation has not been pushed because product acceptance is incomplete. A configured remote or eventual push does not establish compatibility or acceptance.
