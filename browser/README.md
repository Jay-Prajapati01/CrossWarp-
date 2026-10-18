# Browser companion

**Status: planned; not implemented.** This directory records product constraints for a future optional browser extension and Native Messaging host.

The companion may enumerate or label eligible tabs and provide a handoff affordance, but it must launch a live window-portal flow. It must never claim that URL sending, tab reopening, Chrome account sync, or a browser-only surface is the required live origin-window portal. Whole-window portal behavior must also work for non-Chromium windows without the extension.

Before implementation, document supported browsers and versions, extension permissions, native host registration/uninstall, signing, privacy indicators, and compatibility fallback. See [Product specification](../docs/PRODUCT_SPEC.md), [Feature status](../docs/FEATURE_STATUS.md), and [ADR-0001](../docs/decisions/ADR-0001-window-proxy.md).
