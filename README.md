# CrossWarp

*Jump between PCs without breaking flow.*

![CrossWarp logo](apps/desktop/public/crosswarp-logo.svg)

Windows-first desktop application for a single authenticated session combining shared input, Shared Workspace files, an IddCx-backed extended display, and live interactive remote-window portals.

The requirements and acceptance tests are in [docs/PRODUCT_SPEC.md](docs/PRODUCT_SPEC.md). See [docs/FEASIBILITY_PLAN.md](docs/FEASIBILITY_PLAN.md) for Phase 0 prerequisites and the gate, and [docs/IMPLEMENTATION_STATUS.md](docs/IMPLEMENTATION_STATUS.md) for verified progress and blockers.

## Current state

The application has working code foundations for pairing and Shared Workspace operations, but Phase 0 hardware proofs have not passed. This checkout is on the JACK Windows 11 fixture; the Windows 10 peer is visible on the LAN but current credentials cannot enumerate its shares or run remote commands. The official IDD sample compiles and generates an unsigned catalog-bearing package through the registered WDK integration. No driver has been signed, installed, or loaded, and no display, portal, integrated input, or acceptance behavior is claimed as working. See [implementation status](docs/IMPLEMENTATION_STATUS.md) for exact results and blockers.

