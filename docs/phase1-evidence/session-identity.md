# Authenticated shared session identity

## Implementation

After a fresh pairing or trusted reconnect succeeds, `PairedTransport` derives a session correlation ID on each endpoint from the same canonical signed transcript. The derivation is SHA-256 over the domain separator `crosswarp-session-id-v1\0` and transcript bytes, taking the first 16 bytes as the UUID. Because the transcript already contains each side's device identity, fresh nonce, and ephemeral public key, the endpoints agree without an additional handshake message and each reconnect gets a different ID. The local and peer device identities are now available from the paired transport. The desktop includes the session ID in its active trusted-peer diagnostics so subsequent input/media envelopes can bind to this real transport.

The ID is not an authorization secret. The authenticated transport and service-specific consent remain the trust boundary.

## Verification

The `transport` tests now assert that:

- a first-pairing initiator and responder derive the same session ID and report the correct local device identity;
- a trusted reconnect initiator and acceptor derive the same session ID;
- derivation is stable for one transcript and changes when a transcript nonce changes.

On JACK, `cargo test --workspace` passed **60 tests**, and `cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml` passed **3 tests** after this change. Rust formatting and both workspace/Tauri Clippy checks passed. The desktop TypeScript typecheck and Vite production build passed.

## Limits

This is session plumbing, not a feature acceptance result. Input frames and media frames are still not connected to desktop capture/injection/rendering, and no physical two-PC acceptance criterion was passed by these tests.
