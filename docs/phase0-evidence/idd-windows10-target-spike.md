# Phase 0 evidence: IDD Windows 10 target declaration

**Date:** 2026-10-18 (Asia/Calcutta)  
**Status:** Compile/package validation passed; install and runtime compatibility unverified.

## Change

The checked-in Microsoft IddSampleDriver INF previously declared an OS decoration starting at Windows build 22000, which excludes the Windows 10 fixture (build 19045). The INF decoration is now `NT$ARCH$.10.0...17134`, declaring Windows 10 1803 as the package's minimum target. The project builds against IddCx 1.6 with minimum required IddCx version 1.4 (`IDDCX_MINIMUM_VERSION_REQUIRED=4`) and UMDF 2.25.

Microsoft's compatibility guidance says IddCx 1.4+ drivers can use runtime checks to span Windows 10 1803 and later; UMDF 2.25 is available starting with Windows 10 1803. These documented mechanisms support choosing build 17134 as a candidate package floor, but do not guarantee this sample loads on every such build. Required runtime checks, old-build INF validation, signing, and physical testing remain.

References: [Building IddCx 1.4 drivers](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/building-iddcx1.4-drivers), [UMDF version history](https://learn.microsoft.com/en-us/windows-hardware/drivers/wdf/umdf-version-history), [Building WDF drivers for multiple Windows versions](https://learn.microsoft.com/en-us/windows-hardware/drivers/wdf/building-a-wdf-driver-for-multiple-versions-of-windows).

## Build result

Command: `scripts/build-idd-sample.ps1 -Configuration Release`

- Exit code: 0
- x64 driver compile/link: passed
- sample application compile/link: passed
- Inf2Cat signability check: **0 errors, 0 warnings**; unsigned catalog generated
- Stamped INF: `spikes/phase0/microsoft-idd-sample/x64/Release/IddSampleDriver.inf`
- Stamped INF target: `NTamd64.10.0...17134`
- Stamped UMDF library: `2.25.0`
- Driver DLL SHA-256: `CE775B06F22A1B74ACB6173BFA995ED01F0EEB760BD8AC18602DBC910C62DDA7`

## Limits

The source is still Microsoft's sample driver: its swapchain loop releases each surface without encoding or exporting frames. The package is unsigned and was not installed or loaded; the Windows 10 peer was not used for this test. This does not prove true extended display, CrossWarp frame transport, or any of AT-01 through AT-08.
