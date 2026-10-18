# Trusted peer reconnect

**Status:** Implemented in source and covered by loopback protocol tests; physical two-PC reconnect remains unverified.

## Behavior

- The desktop reads saved peers from the current user's DPAPI-protected `TrustedPeerStore` and lists their device fingerprints in the UI.
- A discovered saved peer is offered **Reconnect**. The initiator loads the saved signing-key pin, checks that the authenticated peer identity matches the discovered ID, and verifies a fresh signature over a transcript containing fresh nonces and ephemeral X25519 keys.
- The listener loads its own trusted-peer store. A known returning identity must pass the stored-key signature check. Store read failures fail closed; an unknown or changed key does not get silently trusted.
- A new peer continues through the existing two-sided six-digit approval path. The listener test verifies that an empty trusted-peer set does not bypass the UI approval callback.
- The handshake discriminator changes the wire protocol to version 3. Older versions are rejected; both devices need compatible builds.

## Verification

`cargo test --workspace`: **55 passed**, including listener-accepted trusted reconnect without a pairing prompt, explicit approval for new peers, unknown reconnect rejection, and changed signing-key rejection. Tauri desktop tests: **2 passed**; Tauri Clippy and frontend typecheck passed.

## Limits

This is not yet manually exercised through both desktop UIs or on the physical PCs. Both devices must have completed an original mutual approval and have a saved key pin. Reconnect is initiated from a discovered peer entry; background auto-reconnect is not implemented. A protocol-v3 Windows NSIS installer has now been built, but it has not been smoke-installed or tested on a second PC.
