# CrossWarp desktop application

The app is a Tauri 2 + React/Vite desktop interface backed by the Windows Rust command host. It currently implements LAN discovery, mutually approved first pairing, trusted reconnect, local and remote Shared Workspace UI, transfer progress, capability status, and an opt-in local-only Raw Input diagnostic.

This is a development build. Only the file service is connected to the desktop session. Pairing and file workflows are not yet physically accepted on two PCs; the diagnostic does not share input. No extended display or live window portal is delivered.

## Development

From this directory:

```powershell
npm ci
npm run typecheck
npm run build
npm run dev
```

`npm run build` creates a Windows x64 NSIS development package when the Tauri Windows prerequisites are installed. It is unsigned and does not install an IDD. For the complete prerequisite list, reproducible commands, and test scope, see [Development and build](../../docs/DEVELOPMENT_AND_BUILD.md), [User guide](../../docs/USER_GUIDE.md), and [Testing and acceptance](../../docs/TESTING_AND_ACCEPTANCE.md).
