# ADR-0003: A true Windows extended monitor requires an IDD backend

- **Status:** Accepted; implementation and signing blocked
- **Date:** 2026-10-18

## Context

The requirement is a real additional Windows logical display in Extend topology. Showing a video inside an ordinary application window or mirroring a desktop does not meet that requirement.

## Decision

Use the supported Windows Indirect Display Driver model for virtual-monitor creation and swapchain delivery. Keep UMDF/Session 0 driver work separate from user-session networking, UI and encoding. The receiver renders the display stream on the other PC and coordinates optional native-window overlays.

## Consequences and status

Requires a compatible driver package, signing, installation privileges, OS-specific validation, robust detach/rollback and a media pipeline. Current proof is only an unsigned Microsoft sample build; it is not a CrossWarp driver and is not installed or loaded. The broad Windows-version request exceeds the range proved by the current IddCx route; see [Windows compatibility](../WINDOWS_COMPATIBILITY.md). Never bypass platform signing policy.

## Frame bridge research (2026-10-18)

Microsoft documents that an IDD runs in UMDF Session 0, receives the desktop image as a DirectX surface, and must process it in the IDD's supported user-mode driver model. `IDD_CX_CLIENT_CONFIG::EvtIddCxDeviceIoControl` is explicitly a supported communication callback between a user-session process and the driver; the checked-in sample leaves this callback unset. This gives CrossWarp a supported control/data-request boundary to investigate, but it does not itself provide a network encoder, a session-safe shared GPU resource, or a proven high-throughput pixel transport. The next spike must compare a bounded asynchronous IOCTL frame copy with a supported GPU/shared-resource handoff, measure frame latency and driver callback time, and keep network sockets/consent/UI in the normal user session. No frame bridge has been implemented or tested yet.

References: [IddCx model overview](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/indirect-display-driver-model-overview), [IddCx device callback configuration](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/iddcx/ns-iddcx-idd_cx_client_config), [UMDF application/driver request flow](https://learn.microsoft.com/en-us/windows-hardware/drivers/wdf/detailed-view-of-the-umdf-architecture), [IddCx swapchain objects](https://learn.microsoft.com/en-us/windows-hardware/drivers/display/iddcx-objects).
