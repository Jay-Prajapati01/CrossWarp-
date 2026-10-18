# CrossWarp release and installation guide

**Release state:** development preview only. The current installer is unsigned and does not install a driver. No full-feature Windows release is supported or accepted yet.

## Current package

The current protocol-v6 source produced an x64 NSIS bundle at `J:\cw\release\bundle\nsis\CrossWarp_0.1.0_x64-setup.exe`. Its exact hash and validation limits are in [current build validation](phase1-evidence/current-build-validation-2026-10-18.md). Standard `npm run build` uses the default Cargo target directory; set `CARGO_TARGET_DIR` to a separate path when the running app locks that output executable. Packaging success is not installation verification; this package has not passed installation or second-PC acceptance.

The previous shared-session-identity installer at `artifacts/session-identity-build/release/bundle/nsis/CrossWarp_0.1.0_x64-setup.exe` is historical evidence; see [package build evidence](phase1-evidence/session-identity-package-build.md). It is not the current package. The current v6 peers must be upgraded together before cross-PC session checks.

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
