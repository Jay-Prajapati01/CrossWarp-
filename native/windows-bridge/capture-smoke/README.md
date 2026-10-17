# Per-window capture smoke test

This Windows 10 1903+ / Windows 11 console spike uses `IGraphicsCaptureItemInterop::CreateForWindow`, `Direct3D11CaptureFramePool::CreateFreeThreaded`, and D3D11 to acquire one window frame and write a BMP. It does not stream the frame or inject remote input. Target only a window you own or are authorized to capture; production needs an explicit per-window consent flow and must surface Windows capture/security restrictions.

Build with Visual Studio 2022 and Windows SDK 26100:

```powershell
cmake -S native/windows-bridge/capture-smoke -B artifacts/capture-smoke-build -G "Visual Studio 17 2022" -A x64 "-DCMAKE_SYSTEM_VERSION=10.0.26100.0"
cmake --build artifacts/capture-smoke-build --config Release --parallel 4
```

For a visible app window, obtain its HWND from a process you own, then capture one frame:

```powershell
$p = Get-Process -Id <your-window-process-id>
$hwnd = '0x{0:X}' -f [Int64]$p.MainWindowHandle
.\artifacts\capture-smoke-build\Release\window-capture-smoke.exe $hwnd artifacts\capture.bmp
```

It waits at most 10 seconds for a frame. A timeout may mean that Windows or the target app blocks capture. Do not interpret a successful local frame as evidence of cross-PC window presentation or input routing.
