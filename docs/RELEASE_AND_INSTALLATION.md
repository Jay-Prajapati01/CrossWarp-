# CrossWarp release and installation guide

**Release state:** development preview only. The current installer is unsigned and does not install a driver. No full-feature Windows release is supported or accepted yet.

## Current package

The Tauri application can produce an x64 NSIS bundle at `apps/desktop/src-tauri/target/release/bundle/nsis/CrossWarp_0.1.0_x64-setup.exe`. Build it using [Development and build](DEVELOPMENT_AND_BUILD.md). Inspect the artifact hash and provenance in [Implementation status](IMPLEMENTATION_STATUS.md). Packaging success is not installation verification; the package has not passed second-PC installation and acceptance.

## Driver boundary

The current IDD work builds Microsoft's sample and generates an unsigned catalog-bearing package. It is not a CrossWarp production IDD, is not installed or loaded, and does not provide an extended display. Never disable Secure Boot or signature enforcement as a shortcut. A development proof requires an isolated, recoverable test machine and supported test-signing configuration. A production release needs a valid Microsoft signing/submission route and applicable validation for the declared OS/architecture matrix. See [IDD evidence](phase0-evidence/IDD-sample-build.md) and [ADR-0003](decisions/ADR-0003-extended-display.md).

## Development installation cautions

Install only builds you understand on a test account/machine. Pair only trusted peers; grant only an intended Shared Workspace folder. Current app permissions do not grant arbitrary filesystem access. The current preview does not share input, create an extended display, or present live remote windows. Uninstall and driver-cleanup behavior is not release-qualified.

## Release gate checklist

- [ ] Reproducible source build and dependency/license inventory.
- [ ] Signed application package, update/rollback, and uninstall verification.
- [ ] Production IDD signing and install path validated on every declared OS group.
- [ ] Firewall, pairing revocation, DPAPI lifecycle, privacy and threat review complete.
- [ ] All four features work concurrently and pass AT-01…AT-08 on physical endpoints.
- [ ] Compatibility matrix includes supported mixed-version pairs and architectures.
- [ ] Performance, recovery, crash cleanup, file integrity, and security test evidence published.
- [ ] User guide, troubleshooting, release notes, and known limitations match the build.

Do not label a build production-ready until every applicable gate has evidence and no required feature is silently omitted.
