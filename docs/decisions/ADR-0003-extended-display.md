# ADR-0003: A true Windows extended monitor requires an IDD backend

- **Status:** Accepted; implementation and signing blocked
- **Date:** 2026-10-18

## Context

The requirement is a real additional Windows logical display in Extend topology. Showing a video inside an ordinary application window or mirroring a desktop does not meet that requirement.

## Decision

Use the supported Windows Indirect Display Driver model for virtual-monitor creation and swapchain delivery. Keep UMDF/Session 0 driver work separate from user-session networking, UI and encoding. The receiver renders the display stream on the other PC and coordinates optional native-window overlays.

## Consequences and status

Requires a compatible driver package, signing, installation privileges, OS-specific validation, robust detach/rollback and a media pipeline. Current proof is only an unsigned Microsoft sample build; it is not a CrossWarp driver and is not installed or loaded. The broad Windows-version request exceeds the range proved by the current IddCx route; see [Windows compatibility](../WINDOWS_COMPATIBILITY.md). Never bypass platform signing policy.
