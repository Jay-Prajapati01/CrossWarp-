# Durable trusted-peer store evidence

**Host:** JACK, Windows 11 Home Single Language x64, build 26300.  
**Scope:** persistent authorization primitive only. No network listener or remote endpoint participated.

`TrustedPeerStore` uses Windows DPAPI with app-specific entropy distinct from the device signing identity store. It persists only a peer's stable device ID and public key, checks that the ID is the SHA-256 fingerprint of that key, and atomically replaces the protected store. A peer can be added only from a `PairingApproval` whose local user approval exists and whose peer signature has been verified over the same fresh pairing transcript. Removal supports revoking a trusted peer.

The Windows test verifies: unilateral approval is rejected; two verified approvals can be persisted; the stored file does not contain the raw 32-byte peer key; loading after writing returns the same peer record; and revocation removes it.

Checks on JACK:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Results: **25 Rust tests passed, 0 failed**; Clippy passed with warnings denied; formatting passed. The peer-store test ran on Windows and exercised DPAPI.

No peer store is populated in the user's actual profile by tests (tests use a unique temporary directory). No pairing dialog, discovery, encrypted network channel, or two-device connection has been implemented by this increment.

The Tauri release was rebuilt after adding this store. The updated executable launched as PID 29676; Windows reported `Responding = true` and title `Unified Workspace` after three seconds.
