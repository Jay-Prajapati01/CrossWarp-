# ADR-0002: One authenticated session with bounded service lanes

- **Status:** Accepted as transport architecture
- **Date:** 2026-10-18

## Context

Input, live video and bulk files need to coexist without one feature becoming a mutually exclusive mode. Unbounded queues or a single undifferentiated work queue can create stale interaction and memory growth.

## Decision

Use one authenticated encrypted connection with typed input, window-video, display-video and file lanes. Bound queues; prioritize control/input, drop stale video, and preserve file progress through scheduling/backpressure. Keep service failures independently observable and clean up child workers on disconnect.

## Consequences and status

Lane scheduling does not remove TCP head-of-line blocking or guarantee latency. Current code multiplexes lanes and has loopback tests, but only the file channel is consumed by the desktop; physical concurrent QoS is unproved. See [session protocol](../SESSION_PROTOCOL.md) and [evidence index](../README.md).
