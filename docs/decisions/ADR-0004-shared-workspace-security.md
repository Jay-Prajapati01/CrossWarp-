# ADR-0004: Shared Workspace uses explicit capability roots

- **Status:** Accepted as security architecture
- **Date:** 2026-10-18

## Context

Sharing a user's entire filesystem would expose unrelated private data and make path validation fragile. File transfers must also be verifiable and bounded while interactive lanes remain responsive.

## Decision

The user explicitly selects a root. Remote operations use validated relative paths beneath a capability directory. Bound listings and chunks; verify upload/download integrity; stage writes before replacement; require explicit confirmation for destructive actions and replacement. Keep the service inside the authenticated encrypted session.

## Consequences and status

Access is limited to the selected root and active service authorization. Current code and loopback tests cover core operations, but physical two-PC use, conflict semantics and full acceptance are not proved. See [Shared Workspace feature status](../FEATURE_STATUS.md) and [protocol guide](../SESSION_PROTOCOL.md).
