# Authenticated stream handshake evidence

**Host:** JACK, Windows 11 Home Single Language x64, build 26300.  
**Scope:** authenticated stream protocol tested over both in-process Tokio duplex endpoints and a real loopback TCP connection accepted by the agent's TCP listener. No second PC participated.

## Protocol implemented

- Pairing endpoints exchange protocol version, stable Ed25519 public-key fingerprint, random 32-byte nonce, and one-use X25519 ephemeral public key.
- Both derive the same six-digit verification code from the ordered transcript. Each local approval callback must return true after the user compares the code on both computers. A rejection is exchanged explicitly and aborts pairing.
- Each endpoint signs the complete transcript, including both ephemeral keys; the other endpoint verifies that signature against the advertised fingerprint before the connection is returned.
- X25519 shared secret plus the transcript hash feeds HKDF-SHA-256, which derives separate initiator-to-responder and responder-to-initiator keys.
- Reliable stream frames use ChaCha20-Poly1305 with per-direction monotonic sequence-derived nonces, sequence AAD, and a 1 MiB plaintext maximum. Lengths are bounded before allocation.
- Reconnect mode accepts only a `TrustedPeer` key pin, signs a fresh transcript, verifies the peer signature, and derives new ephemeral session keys without pairing-code approval. A changed key is rejected.
- A successful new-pair result returns the mutual `PairingApproval` object that the caller can persist via `TrustedPeerStore::add_approved`; this crate does not persist approvals implicitly.

## Checks

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Results on JACK: **32 tests passed, 0 failed**; Clippy passed with warnings denied; formatting passed. Tests cover two-sided approved pairing, encrypted send/receive in both directions, explicit rejection, role mismatch, wrong-key authentication failure, pinned reconnect, changed-key rejection, real TCP pairing, and the existing file, identity, and trust tests.

The LAN endpoint binds an ephemeral TCP port and requests mDNS publication using automatically discovered interface addresses; the browser consumes resolved advertisements as untrusted hints and filters invalid/self device IDs. A Rust test connected through a real loopback TCP listener, completed the mutual signed pairing handshake, and exchanged an authenticated encrypted frame. This proves TCP integration locally, not multicast discovery between the two fixtures.

The desktop now includes a **Start local discovery** action. Its backend loads/creates the device identity in Windows DPAPI, binds the TCP endpoint, registers/browses the mDNS service, and can return resolved peer hints. `npm run typecheck` passed; Vite production build passed; `npm run build` passed. The resulting Tauri executable launched as PID 27276 and Windows reported `Responding = true` after three seconds. The UI action itself was not manually exercised. The connection indicator remains disconnected because there is no pairing dialog, accept/connect handshake orchestration, remote RPC, or active session wired to this UI.

## Limits

There is now a TCP listener and mDNS registration/browser primitive. Multicast advertisement/resolution across JACK and DESKTOP-T13BKT8, Windows firewall behavior, listener acceptance on a second PC, and the Tauri discovery action have not been exercised. There is still no pairing approval dialog, accepted-connection handshake orchestration, session supervisor, reconnect loop, or capability RPC. Therefore no paired session is operational in the desktop app, and none of the four features is reported as connected. The six-digit comparison is a human step: the UI must clearly show both device names and require explicit local approval before invoking the callback.
