# CrossWarp HWND capture bridge

This native bridge enumerates visible, titled top-level windows and captures one selected HWND with Windows.Graphics.Capture. The Rust wrapper owns the opaque HWND locally, polls fresh BGRA frames, and encodes a bounded JPEG preview. The desktop starts capture only after a source-side confirmation; the receiver separately approves frames before display. A stop command or peer revocation ends capture. The encrypted window lane is shared with typed control frames for revocation.

The bridge uses the documented Win32 CreateForWindow interop path, which has a Windows 10 version 1903 / build 18362 floor. Runtime API checks and compile success do not prove older Windows compatibility. A legacy capture backend is still required to claim support below that floor.

## Build and local smoke probe

The normal Windows Rust agent build compiles this CMake static library using Visual Studio 2022 and the installed Windows SDK. To build the visible-window probe directly:

    cmake -S native/windows-bridge/window-capture -B J:\cw-capture-cmake -G "Visual Studio 17 2022" -A x64
    cmake --build J:\cw-capture-cmake --config Release --parallel 4

Start the animated WinForms fixture from native/windows-bridge/capture-smoke/animated_test_window.ps1, read the HWND from its ready file, then run:

    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    cargo run -p unified-workspace-agent --example capture_probe -- 0x<HWND>

The probe reports source dimensions, encoded preview dimensions, JPEG size, and SHA-256. Only capture windows you own or have permission to capture.

## Boundaries

- A visible window and source consent are required. Protected/secure content may be unavailable; this bridge does not bypass capture policy.
- Captured frames are resized to at most 1280×720 and quality-stepped to fit the media payload cap.
- This produces a view-only preview. It does not forward pointer/keyboard input or create a movable remote window portal.
- Local Windows 11 capture is evidenced in docs/phase0-evidence/window-capture-preview.md; Windows 10 runtime and two-PC streaming remain unverified.
