# Local Shared Workspace UI evidence

**Host:** JACK, Windows 11 Home Single Language x64, build 26300.  
**Build:** Tauri 2 desktop release on 2026-10-18.  
**Scope:** user-selected local folder, in-process capability grant, and local browser UI. This is not remote sharing.

## Implemented behavior

- The user chooses a directory through the native OS directory picker. The app does not guess a location or silently grant an entire drive.
- Tauri sends the selected path to Rust, which canonicalizes it and opens an anchored `cap_std::fs::Dir` capability.
- The UI lists entries, permits directory navigation and folder creation, and displays the selected root as **Local only**.
- Tauri's dialog plugin is limited in `capabilities/default.json` to `dialog:allow-open`; filesystem operations are Rust commands against the selected root, not broad frontend filesystem access.
- No file contents are previewed or opened by this UI yet. No remote endpoint, transfer status, resume, edit/write workflow, or remote synchronization is present.

## Commands and results

From `apps/desktop` (Cargo was added to PATH from the user's Rust installation):

```powershell
npm run typecheck
npm run web:build
npm run build
```

Results: TypeScript check passed; Vite production bundle passed; Tauri release build passed and produced `src-tauri/target/release/unified-workspace-desktop.exe`.

Launch smoke check: executable started as PID 16196, Windows reported `Responding = true` and title `Unified Workspace` after three seconds. The folder chooser and directory operations were not manually exercised in the native UI during this build check; those remain to be tested interactively.

Rust agent checks after the service increment: `cargo fmt --all -- --check`, `cargo test --workspace` (24 passed), and `cargo clippy --workspace --all-targets -- -D warnings` passed. The Tauri application compiles its Rust command wiring as part of the release build.

## Limits

The selected location is held in memory and must be chosen again after app restart. This UI does not establish cross-PC access. Authenticated transport, peer trust persistence, local/remote file views, mutations, transfer cancellation/resume, and concurrent QoS are unfinished. This increment does not satisfy the Shared Workspace acceptance criterion.
