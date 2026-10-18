# CrossWarp installer

**Current state:** the Tauri project can package an unsigned x64 NSIS development installer. It does not install, sign, or load the display driver. No second-PC install or full uninstall/rollback acceptance has passed.

The intended installer must guide ordinary app installation separately from the explicit privileged IDD step, validate package signatures and OS support before driver installation, retain a recoverable display configuration, and clean up firewall/service/driver state on uninstall. It must not disable Secure Boot or signature enforcement.

See [Release and installation](../docs/RELEASE_AND_INSTALLATION.md), [IDD evidence](../docs/phase0-evidence/IDD-sample-build.md), and [Testing and acceptance](../docs/TESTING_AND_ACCEPTANCE.md).
