# Microsoft IDD sample baseline

Source: Microsoft Windows Driver Samples, `video/IndirectDisplay`.
Upstream commit: `2dc3fd3a0cc84a2933f2194e7ec0871584979071`.
License: Microsoft Public License (MS-PL), included in `LICENSE.txt`.

This is a feasibility baseline only. Its README explicitly says it enumerates one monitor and its sample swapchain loop does nothing with the frames. It is not our product driver and does not prove streaming, installation, performance, or safe production behavior. Changes to this copy must retain the license/attribution; first build and document a minimal monitor enumeration proof, then adapt frame processing only after reviewing its driver callbacks and Session 0 boundaries.
