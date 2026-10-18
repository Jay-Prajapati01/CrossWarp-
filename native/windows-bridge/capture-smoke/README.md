# Per-window capture smoke test

This Windows 10 1903+ / Windows 11 console spike uses `IGraphicsCaptureItemInterop::CreateForWindow`, `Direct3D11CaptureFramePool::CreateFreeThreaded`, and D3D11 to acquire one frame or a bounded burst of up to 16 changed frames and write BMP samples. A burst is limited to 256 MiB of retained CPU frame data and times out after 10 seconds. It is a capture diagnostic, not the product's network stream or remote window proxy, and it does not inject remote input. Target only a window you own or are authorized to capture; production needs an explicit per-window consent flow and must surface Windows capture/security restrictions.

Build with Visual Studio 2022 and Windows SDK 26100:

```powershell
cmake -S native/windows-bridge/capture-smoke -B artifacts/capture-smoke-build -G "Visual Studio 17 2022" -A x64 "-DCMAKE_SYSTEM_VERSION=10.0.26100.0"
cmake --build artifacts/capture-smoke-build --config Release --parallel 4
```

For a visible app window, obtain its HWND from a process you own, then capture one frame:

```powershell
$p = Get-Process -Id 1234 # Replace 1234 with a process ID for a visible window you own.
$hwnd = '0x{0:X}' -f [Int64]$p.MainWindowHandle
.\artifacts\capture-smoke-build\Release\window-capture-smoke.exe $hwnd artifacts\capture.bmp
```

For a changed-frame burst, pass a frame count from 1 to 16. When more than one frame is requested, the output argument is treated as a filename prefix and numbered BMP files are written. The repository's `animated_test_window.ps1` creates a small, self-closing WinForms fixture for this test. The capture waits at most 10 seconds for the requested number of frames; static content may produce fewer frame-arrival events than an animated target.

Do not interpret a successful local capture as evidence of cross-PC window presentation or input routing.
