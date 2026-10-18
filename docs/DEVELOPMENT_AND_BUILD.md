# CrossWarp development and build guide

**Audience:** contributors building from source on Windows. Commands and results here describe the current checkout, not a production certification.

## Repository layout

- `apps/desktop`: Tauri 2, React, TypeScript, Vite UI and Windows command host.
- `crates/agent`: Rust protocol, identity/trust, transport, service lanes and Shared Workspace.
- `native/windows-bridge`: C++ Raw Input/SendInput bridge and Windows.Graphics.Capture spike.
- `native/idd`, `spikes/phase0`: Microsoft IDD sample and feasibility work; not a CrossWarp release driver.
- `installer`: packaging and driver installation design.
- `browser`: planned companion documentation; extension/host not implemented.
- `docs`: product specification, engineering guides, status, decisions and dated evidence.

## Prerequisites

For the currently verified x64 build path, install Git, Rust stable MSVC with `x86_64-pc-windows-msvc`, Node.js/npm, Visual Studio 2022 C++ build tools, Windows SDK, CMake, WebView2/Tauri prerequisites, and (for the IDD sample only) a matching WDK with its VS integration. The exact machine inventory is in [Implementation status](IMPLEMENTATION_STATUS.md). Product feature compatibility is not implied by having these tools.

Open a PowerShell developer shell. If Rust is installed but absent from `PATH`:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

## Build and verify current code

From the repository root:

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
npm --prefix apps/desktop ci
npm --prefix apps/desktop run typecheck
npm --prefix apps/desktop run build
cmake -S native/windows-bridge/input-bridge -B artifacts/input-bridge-build -G "Visual Studio 17 2022" -A x64
cmake --build artifacts/input-bridge-build --config Release --parallel 4
ctest --test-dir artifacts/input-bridge-build -C Release --output-on-failure
```

Run the desktop shell with `npm --prefix apps/desktop run dev`. Produce a release bundle with `npm --prefix apps/desktop run build`. NSIS packaging creates an unsigned development installer; it does not install or activate the IDD. Do not use a package build as evidence of successful installation or feature acceptance.

Build the official IDD sample only by following [`scripts/build-idd-sample.ps1`](../scripts/build-idd-sample.ps1) and its [evidence record](phase0-evidence/IDD-sample-build.md). It intentionally produces an unsigned sample package and does not install it.

## Reproducibility and reporting

Record the commit, command, exit code, tool versions, OS edition/build/architecture, and artifact hash in the relevant evidence page. Keep generated build output under ignored `target/` or `artifacts/` paths. Do not commit secrets, identity stores, user files, or captures containing private content. Update [Implementation status](IMPLEMENTATION_STATUS.md) and affected component documentation whenever behavior changes.

## Known non-goals of this build

The build does not establish universal Windows support, cross-PC input, live display, or live window portals. The compatibility matrix and required additional fixture work are documented in [Windows compatibility](WINDOWS_COMPATIBILITY.md).
