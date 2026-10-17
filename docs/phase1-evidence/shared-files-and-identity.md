# Phase 1 local service evidence

**Host:** JACK, Windows 11 Home Single Language x64, build 26300.  
**Scope:** local filesystem and identity primitives only; no peer connection was used.

## Shared Workspace service

The Rust agent now exposes `SharedWorkspace`, rooted at an open `cap_std::fs::Dir` capability. File and directory APIs take relative paths and never return an ambient absolute path for later I/O. Implemented operations:

- List a directory and omit symlink/reparse entries.
- Read bounded file ranges (8 MiB maximum per call).
- Create a directory, rename an entry, and delete regular files with explicit confirmation.
- Stream sequential upload chunks (1 MiB maximum per chunk) to a temporary file; verify declared length and SHA-256; flush; publish through an atomic same-directory hard link that fails when the destination already exists.
- Remove abandoned temporary upload files when a transaction is dropped.

Tests exercise listing/range reads, traversal and alternate-data-stream rejection, confirmation gates, successful upload publication, checksum failure cleanup, and no-overwrite behavior.

## Device identity

`IdentityStore` uses Windows DPAPI in current-user scope with application-specific optional entropy. It writes the protected blob to `%LOCALAPPDATA%\UnifiedWorkspace\identity.dpapi` using a same-volume temp file and atomic `MoveFileExW` replacement. Temporary plaintext key copies are zeroized. The Windows test saved a generated identity, confirmed the on-disk bytes differ from the plaintext secret, reloaded it, and verified the stable public-key identity.

## Build and test results

Commands, from repository root in a Visual Studio 2022 x64 developer environment:

```powershell
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Results: **24 Rust tests passed, 0 failed**; Clippy passed with warnings denied; formatting passed. The six Shared Workspace tests and Windows DPAPI round-trip test passed on JACK.

Desktop integration checks:

```powershell
npm run typecheck
npm run web:build
npm run build
```

Results: TypeScript check passed, Vite production build passed, and Tauri release build passed after compiling the updated agent dependency. The generated desktop app launched and reported a responding `Unified Workspace` window.

## Limits

This is not yet the network file service or end-to-end Shared Workspace. There is no RPC, authenticated peer authorization, resume manager, remote file browser UI, or multi-PC transfer test. The API must not be advertised as a connected shared folder. The native app dashboard remains disconnected and marks all four capabilities unavailable.
