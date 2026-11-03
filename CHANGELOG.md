# Changelog

Notable changes per release. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and while the
major is `0` a minor bump is where breaking changes live.

Reasoning behind the larger decisions is in [`docs/adr/`](docs/adr/) rather than here — a
changelog says what changed, an ADR says why it will still be that way in six months.

## [Unreleased]

## [0.2.0] — 2026-11-03

### Added

- Native SwiftUI macOS app, backed by a private Rust service. macOS 13 or later.
- Storage overview with category filtering and largest-item inspection.
- Application footprints that include attributed Library data, available without
  a disk scan when Mole supplies an installed-app inventory.
- Searchable application inventory and a dedicated removal-review sheet.
- Reviewed cleanup, uninstall, and maintenance through Mole's own safety rules.
- Move to Trash with resolved-path confirmation and an operation journal.
- Scanner and cleanup-tool setup guidance, including version diagnostics.

### Changed

- The native macOS app is the only desktop shell. React, Tauri, the TypeScript
  transport, and the JavaScript build workspace have been removed (ADR 0033).
- Navigation makes Applications, cleanup, Activity, and maintenance reachable
  outside Settings. File inspectors expose consistent actions.
- Folder browsing has native list selection, size/name ordering, back/forward
  history, and explicit bounded page ranges.
- Rescans retain the previous snapshot until a replacement completes.
- CI, Homebrew, and universal release packaging build Swift and Rust directly.

### Fixed

- Empty directories can be opened and distinguished from unreadable locations.
- Final uninstall confirmation appears above the removal-review sheet.
- Closing the native app cancels its scanner instead of orphaning it.
- Incomplete footprint measurements and scan coverage are labeled explicitly.
- Cleanup reviews explain fresh discovery and do not promise recovery or report
  reviewed bytes as actual space freed.

### Known limitations

- Downloads are ad-hoc signed and not notarized. Homebrew source builds remain
  the supported install route; building requires Xcode 16+ with Swift 6.
- The desktop app is macOS-only. Core and adapters remain platform-neutral.
- Directory browsing uses bounded pages; cleanup recovery is not guaranteed.
- Backends are installed separately. The Homebrew formula installs ncdu; Mole
  remains optional for cleanup, uninstall, inventory, and maintenance.

## [0.1.1] — 2026-08-04

### Fixed

- **Backends were reported as missing in the packaged app.** A windowed process inherits
  launchd's `PATH`, not a shell's, so every backend read as absent no matter what was
  installed. Adapters now search the package-manager directories themselves. This one
  survived to a user, which is why 0.1.1 exists.

## [0.1.0] — 2026-08-04

The first tagged release, and superseded within the day. It rehearsed the macOS pipeline
and found two failures only a real tag could reach: the pinned toolchain was missing a
darwin target, and Tauri tried to sign against an _empty_ `APPLE_CERTIFICATE`. The tap
serves 0.1.1; 0.1.0 is kept for the record.

### Added

- macOS beta: scan with ncdu or gdu, browse a virtualized tree, review Mole's cleanup
  plan, and run it with an explicit confirmation.
- `brew install nirmoka/tap/nirmoka` — a source-built formula, because an unsigned `.dmg`
  is refused by Gatekeeper.
  ([ADR 0024](docs/adr/0024-distribution-is-a-source-built-homebrew-formula.md))
- Backends: ncdu (baseline, cross-platform), gdu (Windows scanner), Mole (macOS cleanup
  and uninstall), rip (exact undo for existing receipts).
- An append-only JSON Lines operation journal, reloaded across launches.

### Known limitations

- Releases are **unsigned**. There is no Apple Developer certificate, so Homebrew is the
  supported install path and the attached `.dmg` will be refused on first launch.
- macOS only. Linux and Windows compile and are tested in CI but are not packaged, because
  the cleanup loop that makes this worth installing runs on Mole.
  ([ADR 0023](docs/adr/0023-the-first-release-is-macos-only.md))
- Selected-path deletion is deliberately unavailable.
  ([ADR 0017](docs/adr/0017-rip-deletion-is-not-execution-bound.md),
  [ADR 0018](docs/adr/0018-selected-path-deletion-is-deferred-for-v0-1.md))

[unreleased]: https://github.com/Shishir435/nirmoka/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/Shishir435/nirmoka/compare/v0.1.1...v0.2.0
[0.1.1]: https://github.com/Shishir435/nirmoka/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Shishir435/nirmoka/releases/tag/v0.1.0
