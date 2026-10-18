# CrossWarp

*Jump between PCs without breaking flow.*

![CrossWarp logo](apps/desktop/public/crosswarp-logo.svg)

CrossWarp is a Windows desktop project for one paired workspace that is intended to coordinate shared mouse/keyboard, a permission-scoped Shared Workspace, a true extended display, and interactive live window portals in one concurrent session.

> **Current release state: development only.** The repo contains working pairing/file-service foundations, an opt-in local input-capture diagnostic, native spikes, and tests. The four-feature concurrent physical acceptance test has not passed. The IDD package is unsigned and unloaded; live input routing, extended-display streaming, and interactive window portals are not complete. Do not treat the current installer as a finished product.

## Start here

- **New user or tester:** [User guide](docs/USER_GUIDE.md)
- **Engineer setting up the checkout:** [Development and build guide](docs/DEVELOPMENT_AND_BUILD.md)
- **What is implemented and what is blocked:** [Implementation status](docs/IMPLEMENTATION_STATUS.md)
- **Document index:** [Documentation guide](docs/README.md)
- **Authoritative product requirements:** [Product specification](docs/PRODUCT_SPEC.md)

## Documentation

The docs are organized by user workflow, system design, security, builds, tests, compatibility, and evidence. Start at [docs/README.md](docs/README.md) for the named guides, decision records, and component notes.

## Repository layout

| Path | Purpose |
|---|---|
| `apps/desktop` | Tauri 2 + React/Vite desktop interface and Windows Rust command host |
| `crates/agent` | Rust identity, pairing, encrypted transport, session lanes, ownership, media/input contracts, and Shared Workspace service |
| `native/windows-bridge` | Windows Raw Input/SendInput bridge and per-window capture spike |
| `native/idd` and `spikes/phase0` | Microsoft IDD sample/build notes; not a signed CrossWarp driver release |
| `browser` | Planned optional browser companion; not currently implemented |
| `installer` | Packaging and privileged-driver installation design notes |
| `docs` | Product, architecture, security, test, compatibility, setup, and evidence documentation |

## Quick checks

Run these from the repository root in a Windows x64 developer shell with Rust, Node.js/npm, Visual Studio C++ tools, Windows SDK, CMake, and the Tauri/WebView2 prerequisites installed:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop run build
cmake --build artifacts/input-bridge-build --config Release --parallel 4
ctest --test-dir artifacts/input-bridge-build -C Release --output-on-failure
```

If Cargo is installed but missing from the shell's `PATH`, add `$env:USERPROFILE\.cargo\bin`. Full prerequisites and reproducible steps are in [Development and build](docs/DEVELOPMENT_AND_BUILD.md). Acceptance requirements are in [Testing and acceptance](docs/TESTING_AND_ACCEPTANCE.md).
