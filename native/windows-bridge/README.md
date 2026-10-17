# Windows bridge

Windows-native components live in focused subprojects. The consent-gated Raw Input / `SendInput` bridge is implemented in [input-bridge](input-bridge/README.md) and linked into the Rust agent on Windows builds. The per-window capture smoke test remains a local single-frame probe; there is still no production graphics streaming, remote window proxy, or end-to-end input routing implementation.
