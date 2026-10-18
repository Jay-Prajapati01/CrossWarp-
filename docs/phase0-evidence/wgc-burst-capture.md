# Windows.Graphics.Capture changed-frame burst proof

**Date:** 2026-10-18 (local)  
**Host:** JACK; `Win32_OperatingSystem.Caption` reported Windows 11 Home Single Language, version `10.0.26300`, x64.  
**Target:** a small, self-closing, animated WinForms test window created by `native/windows-bridge/capture-smoke/animated_test_window.ps1`. The fixture changes its background every 33 ms. It contains no user data.  
**Capture API:** `IGraphicsCaptureItemInterop::CreateForWindow`, `Direct3D11CaptureFramePool::CreateFreeThreaded`, D3D11 staging readback.  
**Build:** Windows SDK 10.0.26100.0, Visual Studio 2022/MSVC 19.44, CMake VS 17 2022 x64.

## Result

The updated local capture spike captured **8 distinct frames** at `782×516` and returned exit code 0. On the final run, elapsed capture time from `StartCapture()` until the eighth frame was **0.362247 s**, or **22.08 frames/s** for this one short CPU-readback burst. Each BMP was 1,211,622 bytes; the eight frames had eight unique SHA-256 hashes. This is a local changed-frame capture measurement, not a sustained network frame-rate or user-perceived latency result.

The same 8-frame request against the static CrossWarp UI did not reach eight frames within the 10-second timeout. This experiment distinguishes a changing capture target from a stationary UI; it does not prove that every application emits frames at a fixed cadence.

The first and last captured frames are [frame 0](wgc-burst-frame-first.png) and [frame 7](wgc-burst-frame-last.png). Their hashes are respectively `8704EDAEB4CFE40D4623E556A757634C6DAD01046733F5870115E763B4B7D9C4` and `567F7D4E8974CBE86A85E6D9C395BF230C6DA4FF6FD5599FFCCFFA00FFD542A9`. The final rebuilt executable hash is `12B58F2E942FC383C7A915834352D197F0BFA1BE30AD59D4072F459E224E25F1`; this version enforces the 256 MiB retained-frame cap. It also captured one frame from the actual CrossWarp window at `1920×1020` in 0.0134546 seconds. An invalid 17-frame request returned exit code 1 with the expected range error before touching the target HWND. Full numbered BMP samples remain under the ignored local path `artifacts/capture-burst/`.

## Reproduction

Build the smoke executable:

```powershell
cmake -S native/windows-bridge/capture-smoke -B artifacts/capture-smoke-build -G "Visual Studio 17 2022" -A x64 "-DCMAKE_SYSTEM_VERSION=10.0.26100.0"
cmake --build artifacts/capture-smoke-build --config Release --parallel 4
```

In one PowerShell window, start the visible fixture and publish its HWND:

```powershell
$ready = Join-Path (Get-Location) 'artifacts\capture-smoke-build\fixture-hwnd.txt'
$script = (Resolve-Path 'native\windows-bridge\capture-smoke\animated_test_window.ps1').Path
$argsText = "-NoProfile -STA -ExecutionPolicy Bypass -File `"$script`" -ReadyFile `"$ready`" -DurationSeconds 8"
$fixture = Start-Process -FilePath "$env:WINDIR\System32\WindowsPowerShell\v1.0\powershell.exe" -ArgumentList $argsText -PassThru
```

After `fixture-hwnd.txt` appears, capture eight frames:

```powershell
$hwnd = (Get-Content $ready -Raw).Trim()
.\artifacts\capture-smoke-build\Release\window-capture-smoke.exe $hwnd artifacts\capture-burst\animated.bmp 8
```

The fixture closes itself after eight seconds. The capture helper bounds a burst to 16 frames and 256 MiB of retained CPU data, and waits at most 10 seconds.

## Scope and remaining work

This proves a Windows 11 local per-window frame source and bounded burst behavior only. Frames are copied to CPU memory and written as diagnostic BMPs. They are not encoded, sent over the encrypted session, rendered on the second PC, or connected to an interactive portal. No B-origin window, input forwarding, capture-consent UX, simultaneous input/file/display load, Windows 10 runtime, or mixed-version pair has been tested by this proof. The currently built native spike still does not satisfy the window-portal feature acceptance criteria.
