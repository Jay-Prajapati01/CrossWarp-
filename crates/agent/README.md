# Rust agent

Foundational paired-session coordinator, ownership, QoS, trust, capability-rooted Shared Workspace file operations, and Windows DPAPI identity persistence. The file API is local and is not yet connected to a peer transport. Network transport/discovery and live input/display/window services remain unimplemented. Run `cargo test --workspace` from the repository root; on Windows this includes the DPAPI identity round-trip test and filesystem service tests.
