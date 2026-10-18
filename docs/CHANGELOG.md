# CrossWarp engineering changelog

This log summarizes repository milestones for maintainers. Git history is the source for exact diffs; evidence pages are the source for what was actually measured. A code milestone does not imply physical acceptance.

## Current development line

### Shared authenticated-session identity (current working increment)

Each successful pairing or trusted reconnect now derives the same transport `SessionId` on both endpoints from the authenticated fresh handshake transcript. The desktop reports the ID beside its trusted-peer state so future input and media records can bind to the actual live connection. Pairing and reconnect loopback tests verify both sides agree. This adds session metadata only; it does not activate input routing, display streaming, window portals, or physical acceptance.

### Input hit-testing correction (current working increment)

Native wheel events now retain their cursor coordinates, and ownership routing hit-tests the wheel location without stealing keyboard focus. The wire compatibility number advances to protocol v4 so older clients are rejected instead of silently misreading the event. A new agent controller combines ownership decisions with encrypted sending; it leaves local events local and releases held keys to the original remote owner. The desktop capture path, capture suppression, remote injection and user consent are still outstanding.

### Branding refinement (current working increment)

Replaced the small-size filled-screen icon with a cleaner outlined dual-display mark and a continuous teal transfer path. This is a vector asset used by the desktop rail; it does not change feature status. Frontend validation is recorded with the increment.

### `9076537` — Local input diagnostic and lifecycle fix

Added explicit-consent local Raw Input event-kind counters, with event data held briefly in memory and never sent to the peer or persisted. Corrected native message-queue startup ordering and capability aggregation. This is a local diagnostic only; no cross-PC input routing.

### `9a418c3` — Trusted reconnect and product branding

Added protocol-v3 pairing/reconnect discriminator, pinned-peer proof and reconnect controls, and the CrossWarp name. First pairing still requires mutual approval. Reconnect and branding have not been physically accepted on two PCs.

### `819cd6b` through `ea65915` — Multiplexed session and service contracts

Added one encrypted transport with four bounded service lanes, typed input session ownership/release contract, binary window/display media frame envelope, and in-process encrypted-lane tests. These solve protocol shape and queue validation; desktop input/video producers and consumers are not wired.

### `eeb51bd` through `b420d59` — Safer file behavior

Added verified temporary download and replacement, transfer progress, and confirmed remote rename/delete/replace operations. Integrity and filesystem behavior have automated tests; multi-GB transfer with the other three live features is untested.

### `6054a67` through `9113bb2` — LAN and encrypted file RPC

Added authenticated TCP transport, mDNS discovery groundwork, capability-rooted file RPC and desktop pairing/client flows. LAN discovery and file operations remain unverified between the physical fixtures.

### `e4da6e2` through `564d08b` — Identity, workspace, input bridge

Added DPAPI-protected device identity/trusted peers, local Shared Workspace UI/service, local per-window capture feasibility spike, and the native Raw Input/SendInput bridge. Input capture/injection is not yet integrated into shared ownership routing; the capture spike is one local frame only.

### `eef905c` — Repository foundation and Phase 0 evidence

Created the monorepo/build foundation and recorded the initial WDK/sample feasibility result. Later evidence reports official sample compilation, but no signed or loaded CrossWarp IDD.

## How to read earlier evidence

Evidence notes are snapshots from the date of their increment. Some explicitly say controls not yet added at that time; later changes may supersede those details. Preserve the historical result and use the latest [Implementation status](IMPLEMENTATION_STATUS.md), [Feature status](FEATURE_STATUS.md), and linked evidence for current state. In particular, the early remote-workspace evidence predates trusted reconnect and later desktop controls.

## Naming and release notes

Product display name: **CrossWarp**. Tagline: *Jump between PCs without breaking flow.* The application and installer remain development artifacts. Do not publish a production release note until the release checklist in [Release and installation](RELEASE_AND_INSTALLATION.md) is complete.
