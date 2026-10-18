# CrossWarp security and privacy

**Purpose:** record the current trust boundaries and protections, and distinguish implemented controls from planned protections. This is not a security audit or production approval.

## Security goals

- Authenticate each device independently of network discovery.
- Require explicit mutual approval for first-time pairing and retain only mutually approved peer pins.
- Encrypt session content in transit and protect device identity/trust material at rest on Windows.
- Limit filesystem operations to user-granted roots and validate all remote paths/records.
- Collect or route input only after deliberate user consent; never bypass UAC, UIPI, protected content, or the secure desktop.
- Fail clearly and fail closed when an identity, trust store, or protocol check fails.

## Trust boundaries

| Boundary | Current treatment | Remaining work |
|---|---|---|
| LAN discovery/mDNS | Untrusted address and peer hint only | Firewall/network behavior and physical spoofing tests |
| First pairing | Six-digit transcript-derived code; user approval on both endpoints; identity signatures | Physical UX tests, timeout/rate-limit review, security review |
| Reconnect | Saved peer fingerprint/key pin; fresh ephemeral exchange/signature proof | Revocation UI, physical reconnect test, compatibility/security review |
| Session records | Directional ChaCha20-Poly1305 with sequence-derived nonce and authenticated sequence | Fuzzing, formal protocol review, robust reconnect/rekey strategy |
| Filesystem | Explicit chosen root, capability-relative paths, bounded operations | Concurrent-edit conflict resolution, quota policy, cancellation/resume, hardware tests |
| Local input diagnostic | Explicit local prompt; Raw Input event structs briefly buffered; only counts sent to UI; no disk/network persistence | Manual test; production sharing consent/suppression/injection design is absent |
| Remote input injection | Not implemented in active session | Per-device consent, target/surface policy, UIPI behavior, release/recovery proof |
| Display driver | Official sample build only, unsigned and unloaded | Supported signed production IDD, safe install/update/rollback, testing on each OS group |
| Window capture | Local single-frame HWND capture spike only | Per-window consent/lifecycle, continuous streaming, protected-content behavior, review |

## Device identity and trust storage

- `DeviceIdentity` derives a stable device ID from the Ed25519 public key.
- The identity store writes a Windows DPAPI-protected blob at `%LOCALAPPDATA%\UnifiedWorkspace\identity.dpapi`; plaintext private key bytes are zeroized after protection/unprotection use.
- Mutually approved peer keys are stored in a DPAPI-protected file, `trusted-peers.dpapi`, under the same application data directory. A peer ID is recomputed from its public key when reading the store.
- DPAPI is per Windows user context; identity material is not synchronized between computers by this store. Loss of the Windows profile/store can require re-pairing.
- The stable device ID is an identity fingerprint, not a secret. Do not use it as a password or authorization token.

## Pairing and transport protections

Handshake messages have strict length/time limits. Pairing verifies role, distinct IDs, key/fingerprint consistency, user approval, and signed transcript proof. Ephemeral X25519 key agreement feeds HKDF-SHA-256; separate directional keys protect a reliable session with ChaCha20-Poly1305. Record sequences are authenticated and replay/out-of-order records are rejected by the receiver sequence state. Session keys are zeroized when their owner drops.

These controls protect the current transport implementation; they do not prove that every RPC authorization behavior is correct or that the design has undergone external cryptographic review. Do not expose the TCP listener to untrusted networks or the public internet. The product is designed for a trusted local network and has no cloud relay/authentication model.

## Filesystem safety

- A user selects an allowed root using the native folder dialog. The app does not grant unrestricted disk access by default.
- Operations use a capability directory and relative paths. Traversal and alternate-stream components are rejected; host-absolute paths are not accepted over RPC.
- Enumeration is capped; read/upload chunks and encrypted messages are bounded; transfer queues apply backpressure.
- New uploads stage to a temporary file and verify length and SHA-256 before publication. Existing files are not overwritten by ordinary upload. A separate confirmed replacement operation is used for overwrite.
- Rename/delete requests require explicit confirmation. Downloads use a local save path and a temporary sibling file; metadata/digest mismatches prevent replacement.
- Remaining risks: filesystem race/TOCTOU review across OS/filesystem variants, quota/resource exhaustion policy, conflict resolution, cancellation/resume correctness, and physical malicious-peer testing.

## Input and capture privacy

- The current local diagnostic must be explicitly started in the UI. Raw Input does not suppress events; local applications still receive them. The native callback copies event structs through a bounded in-memory queue, while the diagnostic reads only event kind and increments aggregate counters.
- The diagnostic does not write event contents to disk or send them to the peer. Users should still avoid sensitive typing while it is active.
- The diagnostic is not the production input-sharing feature. There is no active remote injection/routing. Future sharing requires explicit consent at the capture origin and receiving device and suppression limited to surfaces owned remotely.
- `SendInput` is subject to Windows UIPI. Elevated integrity, secure desktop/UAC, and protected windows are not bypassed. A failed `SendInput` count may not identify UIPI as the reason.
- Window capture must be authorized and must show clear failure for protected or blocked content. DRM/protected frames may not be captured; no bypass is allowed.

## UI and installer boundaries

- The Tauri webview uses a restrictive CSP and the default capability currently grants core permissions plus folder selection. Do not add remote web content or broader filesystem permissions without revisiting the threat model.
- The current NSIS installer is unsigned and does not install the IDD. It is a development artifact, not a trusted production release.
- No driver install should run until a supported signing path, Secure Boot state, administrator flow, rollback, and recovery behavior are validated. Never instruct users to disable Secure Boot or use test-signing as a production workaround.

## Security review checklist before release

1. Review handshake transcript binding, role/downgrade behavior, key lifecycle, nonce/sequence failure, and protocol version handling.
2. Fuzz handshake, encrypted framing, input/media/file decoders, file path validation, and all RPC dispatch variants.
3. Review file concurrent writes, symlink/reparse-point behavior, quota exhaustion, atomic replace semantics, cancellation, and crash recovery.
4. Review input ownership race, hook/suppression approach, injection targets, consent revocation, emergency local recovery, UIPI, and held-state cleanup.
5. Review capture consent, frame retention, privacy/protected-content handling, logs, and memory clearing.
6. Review signed driver catalog, installer elevation scope, rollback/uninstall, and all supported OS builds.
7. Publish a security contact/report process and complete an independent review before production distribution.
