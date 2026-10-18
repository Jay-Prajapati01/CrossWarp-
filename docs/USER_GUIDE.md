# CrossWarp user guide

**Audience:** testers using the current Windows development build.  
**Status:** feature preview. The app is not a finished four-feature product.

## Before you start

- Use the same CrossWarp build on each Windows PC. The current protocol v6 build is not compatible with older wire-protocol builds.
- For the LAN path, place both PCs on a trusted local network where multicast discovery and the advertised TCP connection are allowed. IP addresses and TCP ports are discovered; do not hard-code the sample lab addresses.
- Pair only devices you control. Compare the six-digit code on both screens before approving.
- Keep this version's claims in mind: the Shared Workspace path has code and tests but has not been proved between the two physical PCs; shared input is not implemented; extended display is not working; live interactive window portals are not implemented.
- The NSIS package is an unsigned development installer. It does not install the display driver. Check with the project owner before installing it on a daily-use PC.

## Build and open the current app

Follow [Development and build](DEVELOPMENT_AND_BUILD.md). The release executable and installer are created under:

```text
apps/desktop/src-tauri/target/release/crosswarp-desktop.exe
apps/desktop/src-tauri/target/release/bundle/nsis/CrossWarp_0.1.0_x64-setup.exe
```

The installer build succeeding does not mean it has been installed, signed, or verified on a second computer. Current artifact fingerprint and package state are in [Implementation status](IMPLEMENTATION_STATUS.md).

## First setup and device pairing

1. Open CrossWarp on each computer.
2. Choose **Start local discovery** on each computer. The app binds a TCP listener and publishes/browses the `_uwspace._tcp.local.` mDNS service. Discovery is best effort and may be blocked by firewall/network isolation.
3. When a peer appears, select **Request pairing**. On first connection both computers display an approval dialog.
4. Compare the six-digit codes out of band using both screens. Approve only if the codes match and the device is expected.
5. After mutual approval, each side stores the trusted peer key protected by Windows DPAPI for the current user. Reconnection appears as **Reconnect** when that peer is discovered later.
6. Use **Disconnect peer** to end the current session.

First pairing and reconnect are implemented in source, but this workflow has not yet been manually accepted on the two physical PCs. Treat success/failure in a development UI as a test result, not a production guarantee.

## Select a Shared Workspace folder

1. Select **Choose a folder** and approve the OS folder picker.
2. The app grants access to that folder only. It does not grant all of `C:\` or other drives.
3. The local workspace browser lists entries and creates folders.
4. While paired, the peer can request the granted root through encrypted file RPC. The peer file view supports browse, folder creation, download, chunked upload, confirmed rename, and confirmed delete.
5. Replacing an existing file requires an explicit confirmation. Downloads are written to a same-folder temporary file and validated against peer metadata and SHA-256 before the destination is replaced.

This behavior is unit/loopback tested; physical two-PC operation remains unverified. The current model serves the host's authorized root while the session is open. It is not an offline mirror or conflict-resolving multi-master filesystem.

## Local input capture diagnostic

This is **not shared input**. It only checks whether the native Raw Input sink starts and observes event traffic on the local PC.

1. In the **Local input capture diagnostic · not input sharing** card, click **Start local diagnostic**.
2. Read the disclosure. Raw Input data is briefly buffered in memory while the worker counts event types. Input is not suppressed; local applications still receive it. Event contents are not written to disk or sent to the peer.
3. If you choose to continue, avoid passwords and sensitive text. Move the pointer and use only nonsensitive test input.
4. Click **Stop diagnostic** and record the counters and any error. Stopping unregisters the Raw Input devices.
5. Send the result to the project maintainer with the CrossWarp build, Windows build, and whether the UI showed an error.

The diagnostic does not prove ownership routing, input suppression, remote injection, or cross-PC operation. `SendInput` and Raw Input do not bypass UIPI/UAC boundaries.

## What the dashboard means

- **0 of 4 available** means none of the product capabilities is currently reported available. A responsive peer Shared Workspace can raise this count to **1 of 4**.
- The local input diagnostic is intentionally not counted as shared input.
- OS API-floor labels only indicate documented build boundaries. They do not prove the API is active, a driver is loaded, a window is capturable, or the feature works end to end.
- The four cards must not be interpreted as four complete products simply because their UI entries exist.

## Supported use and known limits

- No Windows build is currently certified for the full product. The product goal is broad Windows-version and mixed-version coverage; see [Windows compatibility](WINDOWS_COMPATIBILITY.md).
- This preview is x64 Windows desktop only. No ARM64, x86, Server, or legacy release has passed the app/driver/API matrix.
- The IDD driver package is unsigned and unloaded. Do not enable test signing, disable Secure Boot, or install the sample as a workaround.
- One physical monitor cannot show two separate desktops in the same pixels. The intended product composes the IDD receiver view with individual app-window proxies.
- A remote-presented app remains running on its origin computer. CrossWarp does not teleport a Windows process.
- Protected content, secure desktop/UAC prompts, elevated windows, exclusive-fullscreen apps, and capture-denied windows may be unavailable under Windows security rules.
- Network latency cannot be zero; measurable responsiveness and congestion behavior are required before release.

## Reporting a problem

Include CrossWarp commit/build, Windows edition/version/build, architecture, whether pairing completed, the visible error text, and any diagnostic counters. Do not send identity-store blobs, private keys, captured screenshots containing private windows, or sensitive input. Use [Troubleshooting](TROUBLESHOOTING.md) and [Testing and acceptance](TESTING_AND_ACCEPTANCE.md) for reproducible reports.
