# Windows.Graphics.Capture per-window smoke evidence

**Host:** JACK, Windows 11 Home Single Language x64, build 26300.
**SDK/toolchain:** Windows SDK 10.0.26100.0, Visual Studio 2022 Community/MSVC 19.44.35229.0, CMake Visual Studio 17 2022 generator.
**Target:** the running Unified Workspace window owned by the same user; latest capture HWND `0x520DFE` (an earlier run used `0x190D20`). No other application content was captured.

The new console spike creates a `GraphicsCaptureItem` for the selected HWND, acquires a D3D11-backed frame, copies it to CPU-readable staging memory, and writes one BMP. The generated source image was visually inspected and shows the actual dashboard frame.

Commands:

```powershell
cmake -S native/windows-bridge/capture-smoke -B artifacts/capture-smoke-build -G "Visual Studio 17 2022" -A x64 "-DCMAKE_SYSTEM_VERSION=10.0.26100.0"
cmake --build artifacts/capture-smoke-build --config Release --parallel 4
$p = Get-Process -Id 12044
$hwnd = '0x{0:X}' -f [Int64]$p.MainWindowHandle
.\artifacts\capture-smoke-build\Release\window-capture-smoke.exe $hwnd artifacts\phase0-local-window-capture.bmp
```

Results: CMake configure passed; MSVC release compile/link passed; runtime exited **0** and reported `CAPTURED hwnd=0x520DFE size=1477x1014`; BMP size was 4,494,102 bytes. The visually inspected PNG conversion is [jack-wgc-window-capture.png](jack-wgc-window-capture.png) (77,991 bytes). The exact uncompressed BMP remains in the ignored local `artifacts/` directory.

SHA-256: PNG `50B6A310838A2125A2F53746411D5405249DD19CB4159EB79F9FC8EAFA69D4CA`; final smoke executable `87CCA355119BCFF8AFC0AA49D36C22E4F9042A64B1F130AEEE49E0CA15F16F5E`.

This verifies one local window-capture component on JACK only. It does not show a B-origin window on JACK, forward input, demonstrate live portal focus, exercise capture consent for another app, or coexist with the IDD extended display and file/input services. It does not prove Win10 runtime compatibility. Microsoft's API documentation lists `CreateForWindow` as available starting in Windows 10 version 1903: [CreateForWindow documentation](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow).
