# CrossWarp troubleshooting

This guide covers the current development build. For privacy-sensitive diagnostics, share errors and counters, not secrets or private content.

## Device not discovered

Confirm both devices run the same protocol-v7 build and share a trusted LAN that permits multicast DNS and the advertised TCP connection. Guest Wi-Fi, client isolation, firewall policy, VPN routing, or blocked multicast can prevent discovery or connection. The app uses discovery addresses only as hints; pairing authentication is separate. Do not hard-code lab fixture addresses as a workaround. Record both app versions, network type, firewall profile and visible error.

## Pairing or reconnect fails

First-time pairing requires matching short codes and approval on both endpoints. Reject unexpected peers or mismatched codes. Reconnect requires a saved key identity; an identity-store or peer-store error fails closed. Ensure both devices run protocol v7. Do not delete protected identity data to bypass trust failures; use an explicit revoke/reset workflow when available.

## Shared Workspace unavailable or file transfer fails

Choose a local folder explicitly and verify the selected path still exists and is accessible to the current user. The remote device can see only the granted root. Current preview operations are bounded; very large files may require chunked transfer. Existing destination replacement, rename and delete require confirmation. Preserve the original file if a transfer fails; report transfer ID, operation, size, displayed error and whether the source hash matches. Physical two-PC file behavior has not yet been accepted.

## Local input diagnostic reports no events or an error

Check that the diagnostic was explicitly started and that mouse movement or nonsensitive keystrokes occurred after it began. Stop it and capture the displayed counters/error. It observes locally and does not suppress or forward input. Avoid typing credentials during the diagnostic. Do not use the diagnostic result to infer remote control works.

## Extended display unavailable

This preview has no signed CrossWarp display driver installed. The unsigned Microsoft sample package is a compile artifact only. There is no supported user-side setting that makes the current build create an extended monitor. Do not disable Secure Boot/signature enforcement or install the sample on a daily-use PC. Driver signing and isolated hardware validation are release blockers.

## Live window portal unavailable

The local WGC smoke captured one frame on the development host; there is no continuous cross-PC window portal yet. URL sharing, Chrome sync, re-opening a tab, and screen mirroring are not equivalent workarounds. Protected windows and secure desktop content are subject to OS restrictions.

## Build failures

Check [Development and build](DEVELOPMENT_AND_BUILD.md) for tool versions and command context. For WDK errors, confirm the Visual Studio WDK integration and use the 64-bit MSBuild host as documented in the IDD evidence. Include the first meaningful compiler/linker diagnostic, tool version and exit code. Do not paste private tokens or environment dumps.

## What to include in a bug report

CrossWarp commit/build, Windows edition/version/build/architecture for each device, exact steps, expected and actual result, pairing state, feature statuses, error text, GPU/network adapter details, relevant sanitized log, and artifact hash. See [Testing and acceptance](TESTING_AND_ACCEPTANCE.md) for physical-test telemetry.
