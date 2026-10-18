# Phase 0 evidence: IDD swapchain to bounded CPU frame snapshot

**Date:** 2026-10-18 (Asia/Calcutta)  
**Status:** Local code/build/contract test passed; no driver load or display streaming proven.

## Increment

The derived IddSampleDriver now reports one logical monitor instead of the sample's multiple monitors. In its swapchain worker, it can recognize BGRA8 surfaces up to 3840×2160, copy them into a D3D11 staging texture, map the staging surface, compact padded rows into a bounded newest-frame `DisplayFrameBuffer`, and record dimensions, format, sequence, and capture time. The buffer is disabled by default; the worker skips the staging/map work unless a future authorized user-session bridge enables it. Disabling capture scrubs and hides the cached pixels. No bridge currently enables it. The buffer has mutex-protected snapshots and rejects unsupported format, size, and stride inputs.

The implementation is in [Driver.cpp](../../spikes/phase0/microsoft-idd-sample/IddSampleDriver/Driver.cpp), [Driver.h](../../spikes/phase0/microsoft-idd-sample/IddSampleDriver/Driver.h), and the independently tested [frame_buffer.h](../../native/idd/frame_buffer.h).

## Validation

- `scripts/build-idd-sample.ps1 -Configuration Release`: exit 0; x64 driver and sample app compile/link passed; Inf2Cat generated an unsigned catalog with **0 errors and 0 warnings**.
- `cmake -S native/idd/tests -B artifacts/idd-frame-buffer-tests -A x64`, Release build, and CTest: **1/1 passed**.
- Contract coverage: disabled-by-default behavior; capture enable/disable and stale-frame scrubbing; invalid dimensions/formats/strides rejected; padded BGRA rows compacted correctly; snapshots do not alias internal data; concurrent writer/reader sees complete frames rather than partially updated pixels.

## Remaining boundary

Even when explicitly enabled by a future consented bridge, frames would remain only in the UMDF driver-host process. There is no user-session IOCTL/device-interface consumer, authenticated session integration, encoder, display-video sender, receiver, or network presentation yet. The current CPU map and copy cost has not been measured and may miss responsiveness targets. The driver is unsigned, not installed, and not loaded. No Windows 10 runtime or AT-01…AT-08 result is claimed.

## Revalidation after privacy-gate change

- Rebuilt `native/idd/tests` Release and reran CTest after adding the disabled-by-default and disable/scrub checks: **1/1 passed**.
- Rebuilt with `scripts/build-idd-sample.ps1 -Configuration Release`: exit **0**; x64 sample and driver compile/link passed; Inf2Cat reported **no errors and no warnings**.
- Driver DLL SHA-256 for that build: `58110EC3B18E30158C8AC94198EEC61D2B7666F85414EF8E76E2BAFABF2E373F`.
- Broader regressions on this checkout: agent **65/65**, desktop **7/7**, frontend typecheck/build passed. These are local automated checks and do not establish driver runtime or two-PC acceptance.
