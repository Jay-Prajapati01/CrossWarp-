# CrossWarp testing and acceptance guide

**Current result:** automated component tests exist; no AT-01 through AT-08 acceptance test has passed on the two physical fixtures. Four-feature end-to-end acceptance is **0/4**.

## Test layers

1. **Static/build checks:** Rust formatting, Clippy, TypeScript typecheck, Vite build, C++ compile/link.
2. **Unit/contract tests:** protocol validation, trust and replay rules, workspace path/security behavior, ownership contracts, queue bounds and cancellation.
3. **In-process integration tests:** authenticated encrypted transport, service-lane multiplexing and file RPC; these do not prove LAN behavior.
4. **Local hardware smoke tests:** Raw Input diagnostic or a local WGC frame capture; they do not prove cross-PC operation.
5. **Physical acceptance:** install on both PCs and pass AT-01…AT-08 in one concurrent session, then repeat for each claimed Windows pair.

## Current automated commands

See [Development and build](DEVELOPMENT_AND_BUILD.md) for the full reproducible command sequence. Latest recorded agent result is 55 tests passed; the desktop Tauri suite has 3 tests; native input bridge CTest has 1 contract test. These counts are a recorded development snapshot, not the entire acceptance suite. Inspect individual evidence pages for exact scope and outcome.

## Two-PC acceptance procedure

Use the requirements verbatim in [Product specification §11](PRODUCT_SPEC.md). Capture the exact build commit and both OS editions/builds/architectures first. Run AT-01 pairing/reconnect; keep the session connected throughout AT-02 through AT-05; verify real JACK extended topology, B-origin stateful browser proxy, focus/input ownership, and a multi-GB workspace transfer concurrently. Then run AT-06 recovery, AT-07 installer/trust, and AT-08 truthful terminology. Record every criterion pass/fail/blocked with logs and redacted video where appropriate. A subsystem passing alone does not pass the concurrent test.

## Required telemetry and evidence

For every claimed compatibility group, capture machine identifiers, build and artifact hashes, GPU and network adapter/link details, exact steps, per-AT outcomes, logs, average FPS, dropped frames, p50/p95 action latency, transfer integrity, and limitations. Do not collect passwords or private file/window contents. Store dated evidence in `docs/phase0-evidence/` or `docs/phase1-evidence/`; link it from [Implementation status](IMPLEMENTATION_STATUS.md).

## Evidence index

The [documentation index](README.md) links each existing evidence note. IDD sample compilation, local WGC capture, authenticated stream, trusted reconnect, file RPC, local workspace UI, multiplexer, typed input/media and the local capture diagnostic are separate proofs with intentionally limited scope. None is physical simultaneous acceptance.

## Windows compatibility matrix

Use [Windows compatibility](WINDOWS_COMPATIBILITY.md). Test OS builds as endpoint pairs (including mixed versions), architectures and driver/API variants. A test on the Windows 11 host does not certify Windows 10, and two endpoint rows passing independently does not prove the mixed pair. Unsupported releases must be explicitly scoped with technically valid limits; universal compatibility cannot be asserted without proof.
