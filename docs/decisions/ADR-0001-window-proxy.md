# ADR-0001: Present cross-PC windows as interactive proxies

- **Status:** Accepted as product architecture
- **Date:** 2026-10-18

## Context

A Windows process, HWND, browser profile and login remain owned by their origin OS. Moving the process to another PC is not a viable presentation model. URL reopening and screen mirroring fail the live-state and per-window requirements.

## Decision

Capture a user-approved origin window, send its frames to a movable destination proxy, and forward permitted pointer/keyboard actions to the origin process. Bind each portal to origin device/window, destination, authenticated session and consent. Closing a proxy must not implicitly close the origin window. Use supported OS capture/injection APIs and report restricted surfaces clearly.

## Consequences and status

The origin machine must remain online and the window capturable. Latency depends on capture, encode, network, decode and render. Secure desktop, UIPI-protected, DRM and capture-denied content cannot be bypassed. Current repository has only a local one-frame WGC spike and media contracts; no portal is implemented. See [feature status](../FEATURE_STATUS.md).
