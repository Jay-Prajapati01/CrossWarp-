# Architecture research notes

**Purpose:** Use public behavior and official Windows API documentation to inform CrossWarp's design. These notes are not claims of feature completion and do not copy third-party implementation code.

## Similar product boundaries

- Microsoft PowerToys Mouse Without Borders documents keyboard/mouse, clipboard, and file transfer between up to four computers, and requires an initial connection/security-key setup. Its documented limitations include single-file drag/drop and a clipboard file-size limit. CrossWarp keeps the useful unified-peripheral and explicit-pairing concepts while designing an app-managed, scoped Shared Workspace and a separate real display/window path.
- Microsoft's IDD documentation describes a user-mode indirect display driver for virtual/remote displays. The operating system supplies desktop frames through the driver's DirectX swapchain; user-mode application code must perform network transport and receiver presentation. CrossWarp therefore separates the UMDF display adapter from the interactive app/session and does not treat a normal window mirror as an extended monitor.
- Windows.Graphics.Capture supports user-approved capture of an application window or display and exposes frame-arrival surfaces. `CreateForWindow` has a documented Windows 10 version 1903 floor. This is a strong backend for eligible live portals on supported systems; older releases need a different capture backend or an honest unsupported report.

## Design consequences for CrossWarp

- Keep pairing identity and explicit consent in the session coordinator; discovery advertisements remain hints, not identity proofs.
- Keep input, window media, display media, and files as independent bounded lanes. Input is lossless and prioritized; real-time video may discard stale frames; file traffic applies backpressure and continues to make bounded progress.
- Keep global Raw Input capture consent-gated and in the user's session. Raw Input is delivered to registered windows and does not itself suppress normal input; `SendInput` remains subject to UIPI. Focus ownership, release recovery, and local fallback must be implemented above those APIs.
- Treat window portals as remote proxies for the original process, not URL synchronization or process migration. Capture permission and protected-content boundaries are visible product states.
- Treat IDD driver signing, installation, rollback, and OS version support as release requirements. Sample compilation alone is not display functionality.

## Sources

- [PowerToys Mouse Without Borders](https://learn.microsoft.com/en-us/windows/powertoys/mouse-without-borders)
- [Indirect Display Driver Model overview](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview)
- [IddCx objects and swapchain lifecycle](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/iddcx-objects)
- [Windows Graphics Capture overview](https://learn.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture)
- [CreateForWindow API and minimum OS requirement](https://learn.microsoft.com/en-us/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow)
- [Raw Input registration](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerrawinputdevices)
- [SendInput and UIPI limitations](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-sendinput)
