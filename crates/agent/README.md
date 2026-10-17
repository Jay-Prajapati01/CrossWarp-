# Rust agent

The agent currently provides device identity and mutual approval, an authenticated encrypted TCP session, a bounded four-service multiplexer, and capability-rooted Shared Workspace RPC. The multiplexer gives input, window video, extended-display video, and files independent service channels over one session. It prioritizes input and interactive video, applies bounded queues, and reserves periodic file progress. The desktop consumes the file channel; input and media producers/receivers are not integrated yet and must still report inactive.

The service frame format is part of protocol version 2. Both peers must run a build that supports the same protocol version. Old v1 clients fail the authenticated protocol-version check rather than misinterpreting a service tag as a file packet.

Tests cover all four service channels over an in-process authenticated encrypted stream, Shared Workspace RPC over the multiplexed file channel, bounded queues, bulk progress under input load, and cancellation cleanup. Physical LAN pairing, simultaneous input/video/file operation, and Windows 10/11 acceptance remain unverified. Run `cargo test --workspace` from the repository root. Windows tests exercise DPAPI and filesystem behavior.
