# Remote Shared Workspace RPC evidence

**Status:** Implementation checks passed on JACK; the two-PC pairing and file-operation workflow remains unverified.

## Implemented path

- A pair of endpoints complete the existing explicit two-sided verification-code handshake before the transport is kept alive.
- A bidirectional session actor owns the encrypted sender and receiver. It correlates requests and responses while serving each endpoint's own granted workspace, so peer UI calls share the stream without competing for its reader.
- The peer view lists and navigates directories, creates directories, downloads files in bounded ranges, and uploads a user-selected file using bounded chunks and SHA-256 verification.
- The actor reads the current workspace grant for each inbound request. If no folder is granted, it returns an explicit error and does not access a host path.

## Checks on JACK

- `cargo test --workspace`: **38 passed, 0 failed**. The RPC test pairs identities over a loopback duplex transport, starts an actor on both endpoints, verifies a no-grant response, updates the host grant on the live connection, verifies a listing, and issues requests concurrently from both endpoints.
- `cargo clippy --workspace --all-targets -- -D warnings`: passed.
- `cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check`: passed.
- `cargo check --manifest-path apps/desktop/src-tauri/Cargo.toml`: passed.
- `npm run typecheck` from `apps/desktop`: passed.
- `npm run build` from `apps/desktop`: production Tauri build succeeded.
- The built desktop executable launched and responded as process **PID 28544**. UI click-through and live peer operation were not tested.

## Remaining proof

No authenticated session was established with DESKTOP-T13BKT8. The UI does not yet expose rename/delete, upload progress, trusted-peer reconnection, or disconnect controls. This increment proves neither SMB/LAN mDNS reliability nor the four simultaneous product capabilities. Acceptance tests AT-01 through AT-08 remain unpassed.
