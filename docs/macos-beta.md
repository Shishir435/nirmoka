# macOS beta

The beta currently validates the Rust workspace on macOS only. The Linux and Windows entries remain
commented in `.github/workflows/ci.yml`; uncommenting those entries restores their jobs while the
backend installation and platform-specific assertions remain intact below the matrix.
The separate web build remains on Ubuntu because it is platform-independent and does not exercise a
desktop backend.

## Launch locally

1. Install supported macOS backends: `brew install ncdu mole`.
2. Install workspace dependencies: `pnpm install`.
3. Confirm detection: `pnpm nrmk backends`.
4. Start the desktop shell: `pnpm tauri dev`.
5. In Overview, leave `~` selected for a home scan or enter a narrower directory, then choose Scan.

The packaged Tauri application always resolves the real transport. `pnpm dev` alone is browser UI
development and displays an explicit fixture-mode banner.

## Current safety boundary

Scanning is read-only. Selected-path deletion remains unavailable under ADR 0018. Mole advertises
curated cleanup and dry-run abilities, but its preview is human-readable output; until the adapter
exposes a typed, tested preview and confirmation API, the Clean page reports it unavailable and does
not run a command or invent cleanup values.

Application and developer inventories are views over the completed ncdu tree. They only report
evidence inside the chosen scan root. Application last-used timestamps, related-data mappings,
leftover detection, and developer modified timestamps are shown as unavailable because ncdu's wire
format does not provide them.
