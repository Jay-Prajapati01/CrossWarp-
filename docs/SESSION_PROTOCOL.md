# CrossWarp session protocol

**Wire protocol:** version 5.
**Compatibility:** both peers must use a compatible protocol version. Unknown/older versions are rejected. No released compatibility guarantee exists yet.

This guide describes the current implementation in `crates/agent/src`. For code-level types, see `protocol.rs`, `transport.rs`, `session_transport.rs`, `file_rpc.rs`, `input_session.rs`, and `media_session.rs`.

## Connection discovery

- Service type: `_uwspace._tcp.local.`
- The local agent binds a TCP listener to an ephemeral port and advertises the device fingerprint, display name, protocol hint, and port via mDNS.
- An advertisement is an untrusted address hint. It is not proof of device identity and must not authorize file or input access.
- The app verifies the authenticated remote device identity against the discovered device ID after handshake.
- LAN discovery is best-effort. Multicast filtering, host firewall rules, guest Wi-Fi/client isolation, VPN routing, and multiple active interfaces can prevent discovery or peer connectivity.

## Shared transport session identity

After the authenticated handshake succeeds, both endpoints derive the same `SessionId` by hashing the domain separator `crosswarp-session-id-v1\0` with the canonical signed pairing transcript and using the first 16 digest bytes as the UUID. The transcript contains both device identities, fresh endpoint nonces, and ephemeral key material, so a reconnect gets a fresh correlation ID without another wire round trip. The session ID is metadata for binding input/media records and diagnostics; it is not a secret or an authorization token. Authorization still comes from the authenticated encrypted transport and explicit capability consent.

## Version 5 input-event and media schema

Version 5 retains the version-3 pairing/reconnect discriminator and version-4 event-time pointer coordinates for wheel ownership. It adds JPEG as a window-video codec identifier for the current desktop receiver preview. Version 4 peers are rejected during handshake; both devices must run protocol v5 builds to interoperate.

## Protocol v3 pairing and reconnect handshake

Handshake messages are length-prefixed JSON with a 4096-byte maximum and a 15-second I/O timeout. A `Hello` carries protocol version, role, authentication kind, device ID, Ed25519 public key, fresh 32-byte nonce, and fresh ephemeral X25519 public key.

### New peer

1. Initiator and responder exchange `Hello` messages and verify protocol version, role, distinct identities, and device ID/public-key fingerprint consistency.
2. Both sides derive the same six-digit verification code from the authenticated pairing transcript (both device identities, both fresh nonces, and both ephemeral public keys).
3. Each app must show the code and receive explicit user approval. A rejection or timeout aborts the exchange.
4. Ed25519 signatures prove possession of the identity keys and bind the pairing transcript. A peer is saved only after both sides approve and verify the transcript.

### Trusted reconnect

- The initiator uses the reconnect authentication discriminator and a previously saved `TrustedPeer` pin.
- The responder compares the presented fingerprint and signing key with its DPAPI-protected trusted-peer list; it verifies fresh proof and key exchange without asking for the pairing code again.
- An unknown peer, changed signing key, mismatched ID, incompatible protocol, or invalid signature is rejected. A corrupt/unreadable trusted-peer store fails closed.
- Reconnect here means a user-selected reconnect for a discovered peer. Background reconnect is not implemented.

### Session keys and encrypted records

- Both sides derive an X25519 shared secret, hash the canonical exchange transcript with SHA-256, and derive separate initiator→responder and responder→initiator keys with HKDF-SHA-256.
- Each directional key protects one reliable stream with ChaCha20-Poly1305. Nonces derive from a monotonically increasing 64-bit sequence; the sequence is authenticated as additional data. Each direction has an independent sequence space.
- Ciphertext is length-prefixed with a 32-bit size. The plaintext payload maximum is 1 MiB; authentication failures, sequence exhaustion, malformed lengths, and truncation terminate the transport.
- Session keys are zeroized on drop. Identity private keys are protected at rest with Windows DPAPI; only public key pins are needed to authenticate a reconnect.

## Multiplexed service record

The encrypted payload begins with one service tag, followed by that service's bytes:

| Tag | Service | Reliability/policy | Queue capacity |
|---:|---|---|---:|
| 1 | Input | Ordered, lossless queue; high writer priority | 256 messages |
| 2 | Window video | Real-time; stale frame draining/drop allowed | 2 messages |
| 3 | Display video | Real-time; stale frame draining/drop allowed | 2 messages |
| 4 | Files/control RPC | Lossless bounded backpressure; scheduler reserves progress | 8 messages |

Queues are bounded to constrain memory. Input and file producers use backpressure; video producers use nonblocking sends and should discard stale frames. The scheduler periodically yields from input to video and file work. Since every encrypted record uses one TCP connection, packet-loss head-of-line blocking remains possible even when application queues are correctly scheduled. Low-latency media needs measurements and may require transport redesign before acceptance.

## Shared Workspace RPC

File requests/responses are tagged JSON messages carried by service 4. The service has a 1 MiB encrypted record limit; JSON byte-array expansion drives the current app transfer chunk cap to 200 KiB.

Current request operations:

- `List` — list at most 500 entries for a granted relative directory.
- `ReadRange` — read bounded byte ranges; desktop chunks are limited to 200 KiB even though storage supports larger ranges.
- `Metadata` — size/hash/metadata used to detect changing files and verify downloads.
- `CreateDirectory` — create a directory within the granted root.
- `BeginUpload`, `UploadChunk`, `CommitUpload` — sequential temporary upload, length/checksum validation, and commit to a new destination.
- `CommitUploadReplace` — replace a destination only with explicit confirmation and valid digest.
- `AbortUpload` — discard an incomplete upload.
- `Rename`, `Delete` — require explicit confirmation; rename does not overwrite an existing destination.

Paths are relative and validated by a capability-rooted filesystem layer. The host never accepts a peer-supplied absolute path. The desktop download path is selected locally; the receiver writes to a temporary file beside that destination, checks peer metadata and local SHA-256, then commits atomically where supported.

## Input envelope

`InputFrame` carries session ID, monotonically increasing sequence, target device ID, and one typed event: pointer move, pointer button, wheel, or key transition. Pointer moves, buttons and wheel events carry screen-space coordinates captured at event time; wheel direction and extended-key state are preserved. The receiver validates size, destination, sequence, valid button/key codes, and balanced down/up state. `release_all` returns matching key/button releases after disconnect.

This is a protocol/routing contract only. The desktop does not yet send or consume input frames. The local diagnostic does not put events on the wire. No consent policy, surface hit-test registration, local-event suppression, receiver injection, or physical input-sharing proof is implemented.

## Video envelope

`MediaFrame` carries session ID, target device ID, stream ID, sequence, monotonic source timestamp in 100 ns units, dimensions, codec (`H264`, `HEVC`, `AV1`, or `BGRA8`), key-frame flag, and binary payload. A small bounded JSON header is length-prefixed within the service payload; media bytes remain binary. Dimensions and payload are bounded, and receivers reject wrong destinations, replay/reordering, and invalid sizes. Sequence gaps are allowed after stale frame drops.

The desktop now consumes the authenticated window-video lane, checks session/destination/replay state, requests local approval before rendering the first JPEG frame, and exposes the latest approved preview to the UI. JPEG decoding is delegated to the WebView image decoder. This is a receiver-only preview increment: there is still no peer-side capture/encoding producer, interactive proxy, forwarded input, or display-driver consumer. H264/HEVC/AV1/BGRA decoding is not wired. A display-video producer/consumer is still absent.

## Evolution rules

- Bump the protocol version for incompatible handshake, record, or RPC changes and add rejection/compatibility tests.
- Keep size limits and schema tags explicit; do not deserialize untrusted unbounded payloads.
- Preserve independent service availability and backpressure policies. A video failure must not silently disable file or input services.
- Document version negotiation/rollout before claiming interoperability across app versions.
- Add fuzz/property tests for untrusted protocol decoders before production use.
