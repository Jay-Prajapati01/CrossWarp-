# CrossWarp indirect display driver work

**Status: feasibility spike only.** The repository builds Microsoft's IddSampleDriver and generates an unsigned catalog-bearing package. This is not CrossWarp's production IDD; it is not signed, installed, or loaded and does not create a usable CrossWarp extended display.

The product requirement is a real Windows logical monitor in Extend topology. The intended architecture uses a supported UMDF IddCx driver for monitor/swapchain duties and keeps app UI, networking, and encoding in the user session. A stream shown in a normal window is not an extended monitor.

The repeatable sample build is `scripts/build-idd-sample.ps1`; exact toolchain and results are in [IDD sample evidence](../../docs/phase0-evidence/IDD-sample-build.md). Do not install the unsigned sample or change Secure Boot/test-signing on a daily-use PC. Production requires a validated signed package and hardware/OS test matrix. See [ADR-0003](../../docs/decisions/ADR-0003-extended-display.md) and [Windows compatibility](../../docs/WINDOWS_COMPATIBILITY.md).
