# Windows bridge

Windows-native components live in focused subprojects. The Raw Input/`SendInput` bridge in [input-bridge](input-bridge/README.md) is linked into the Windows Rust agent. An opt-in local diagnostic counts event kinds; it does not forward input. The per-window capture smoke test remains a local single-frame probe. There is still no production graphics streaming, remote window proxy, or end-to-end input routing implementation. See [feature status](../../docs/FEATURE_STATUS.md) and the [documentation index](../../docs/README.md).
