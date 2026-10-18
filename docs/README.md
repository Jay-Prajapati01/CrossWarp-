# CrossWarp documentation guide

**Documentation status:** maintained with the implementation; feature claims are tied to code and evidence.  
**Product state:** in development. The complete four-feature acceptance matrix is not passed.

This page is the map for the project documentation. Read the product specification for the requirements; read the status/evidence pages for what has actually been built and proved. A design description does not prove runtime behavior.

## Start by role

| If you are… | Read… |
|---|---|
| Trying the current development build | [User guide](USER_GUIDE.md), then [Troubleshooting](TROUBLESHOOTING.md) |
| Setting up a development machine | [Development and build guide](DEVELOPMENT_AND_BUILD.md) |
| Reviewing implementation progress | [Implementation status](IMPLEMENTATION_STATUS.md), then [Feature status](FEATURE_STATUS.md) |
| Understanding the software structure | [Architecture](ARCHITECTURE.md), [Session protocol](SESSION_PROTOCOL.md), and [Architecture research](ARCHITECTURE_RESEARCH.md) |
| Reviewing security or privacy | [Security and privacy](SECURITY_AND_PRIVACY.md) |
| Running tests or acceptance sessions | [Testing and acceptance](TESTING_AND_ACCEPTANCE.md) |
| Checking supported Windows releases | [Windows compatibility](WINDOWS_COMPATIBILITY.md) |
| Preparing an installer | [Release and installation](RELEASE_AND_INSTALLATION.md) |

## Documents by topic

### Product and user experience

- [Product specification](PRODUCT_SPEC.md) — authoritative requirements, architecture blueprint, acceptance tests, and completion rule.
- [User guide](USER_GUIDE.md) — current setup, pairing, Shared Workspace, local capture diagnostic, and truthful limitations.
- [Feature status](FEATURE_STATUS.md) — requirement-by-requirement inventory of implementation, proof, and remaining work.
- [Troubleshooting](TROUBLESHOOTING.md) — build, discovery, pairing, file, driver, and second-PC test issues.

### Engineering

- [Engineering changelog](CHANGELOG.md) — implementation milestones and scope notes, with links back to current state/evidence.
- [Architecture](ARCHITECTURE.md) — components, ownership boundaries, data flow, runtime lifecycle, and gaps.
- [Session protocol](SESSION_PROTOCOL.md) — protocol v4 handshake, encrypted records, service lanes, file RPC, input/media contracts.
- [Security and privacy](SECURITY_AND_PRIVACY.md) — trust, encryption, permissions, native input limits, data storage, and threat boundaries.
- [Development and build](DEVELOPMENT_AND_BUILD.md) — prerequisites, commands, project layout, and reproducible Windows builds.
- [Testing and acceptance](TESTING_AND_ACCEPTANCE.md) — automated checks, evidence inventory, AT-01…AT-08, and physical test procedure.
- [Windows compatibility](WINDOWS_COMPATIBILITY.md) — verified API floors and the unpassed OS/version matrix.
- [Release and installation](RELEASE_AND_INSTALLATION.md) — current packaging, driver/signing constraints, and release checklist.
- [Architecture research](ARCHITECTURE_RESEARCH.md) — public platform concepts and design consequences.
- [Feasibility plan](FEASIBILITY_PLAN.md) — Phase 0 prerequisites and gates.

### Evidence logs

- [Phase 0: IDD sample build](phase0-evidence/IDD-sample-build.md)
- [Phase 0: local WGC window-capture smoke](phase0-evidence/wgc-capture-smoke.md)
- [Phase 0: changed-frame burst capture proof](phase0-evidence/wgc-burst-capture.md)
- [Phase 0: capture-to-window-lane preview](phase0-evidence/window-capture-preview.md)
- [Phase 1: authenticated stream](phase1-evidence/authenticated-stream.md)
- [Phase 1: authenticated shared session identity](phase1-evidence/session-identity.md)
- [Phase 1: shared-session identity package build](phase1-evidence/session-identity-package-build.md)
- [Phase 1: trust and peer storage](phase1-evidence/trusted-peer-store.md)
- [Phase 1: trusted reconnect](phase1-evidence/trusted-reconnect.md)
- [Phase 1: Shared Workspace storage and identity tests](phase1-evidence/shared-files-and-identity.md)
- [Phase 1: file RPC](phase1-evidence/remote-workspace-rpc.md)
- [Phase 1: local Shared Workspace UI](phase1-evidence/local-workspace-ui.md)
- [Phase 1: session multiplexer](phase1-evidence/session-multiplexer.md)
- [Phase 1: typed input session](phase1-evidence/input-session.md)
- [Phase 1: Win32 input bridge](phase1-evidence/input-bridge.md)
- [Phase 1: local input capture diagnostic](phase1-evidence/input-capture-diagnostic.md)
- [Phase 1: native input coordinate mapping](phase1-evidence/input-coordinate-mapping.md)
- [Phase 1: protocol-v4 Windows package build](phase1-evidence/protocol-v4-package-build.md)
- [Live LAN observation and local checks (2026-10-18)](phase1-evidence/live-lan-observation-2026-10-18.md)
- [Phase 1: typed media session](phase1-evidence/media-session.md)
- [Historical installer smoke](phase1-evidence/installer-smoke.md)

## Component notes

- [Desktop application](../apps/desktop/README.md)
- [Rust agent](../crates/agent/README.md)
- [Windows bridge overview](../native/windows-bridge/README.md)
- [Raw Input/SendInput bridge](../native/windows-bridge/input-bridge/README.md)
- [WGC capture spike](../native/windows-bridge/capture-smoke/README.md)
- [IDD spike](../native/idd/README.md)
- [Installer design](../installer/README.md)
- [Browser companion](../browser/README.md)

## Design decision records

- [ADR-0001: represent a cross-PC window as an interactive proxy](decisions/ADR-0001-window-proxy.md)
- [ADR-0002: one encrypted session with independent service lanes](decisions/ADR-0002-session-lanes.md)
- [ADR-0003: true extended display uses an IDD backend](decisions/ADR-0003-extended-display.md)
- [ADR-0004: capability-rooted file sharing](decisions/ADR-0004-shared-workspace-security.md)

## Documentation maintenance rules

1. Update [Implementation status](IMPLEMENTATION_STATUS.md) in the same change as any implementation/evidence increment.
2. Put commands, OS build, architecture, toolchain, and actual result in evidence logs. State clearly when a result is a unit test, local smoke, installer test, or physical multi-PC acceptance proof.
3. Keep planned, implemented, tested, and physically accepted claims distinct. A design or compile result is not a runtime or acceptance result.
4. When changing user-visible behavior, update the [User guide](USER_GUIDE.md). When changing protocol/security behavior, update [Session protocol](SESSION_PROTOCOL.md) and [Security and privacy](SECURITY_AND_PRIVACY.md).
5. Add a decision record when a major architecture choice changes. Never delete failed results; supersede them with dated evidence and explain the change.
6. Do not mark a Windows release supported until every concurrent AT-01…AT-08 acceptance test passes for that OS/build pair.
