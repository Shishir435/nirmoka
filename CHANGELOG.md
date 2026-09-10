# Changelog

Notable changes per release. Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versions follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html), and while the
major is `0` a minor bump is where breaking changes live.

Reasoning behind the larger decisions is in [`docs/adr/`](docs/adr/) rather than here — a
changelog says what changed, an ADR says why it will still be that way in six months.

## [Unreleased]

Everything below is on `main` and not yet tagged. The window a user opens is now the
product, rather than a tree browser with a cleanup tab beside it.

### Added

- **Move to Trash.** The verb the read-only beta was missing. Recoverable removal only,
  through the platform's own Trash service, and on macOS through the Finder specifically
  so that Put Back works. A `Trashed` journal event records it.
  ([ADR 0025](docs/adr/0025-move-to-trash-is-a-platform-integration.md))
- **Application uninstall**, relayed through Mole's own confirmation. Nirmoka never
  handles a password; Mole puts up its own authorization dialog. Trash is the default
  route and a test asserts on the recorded argv that `--permanent` is never passed.
  ([ADR 0027](docs/adr/0027-uninstall-is-a-relayed-confirmation.md))
- **Application footprint attribution.** What an application actually costs, assembled
  from its bundle identifier rather than its `.app` bundle size alone.
  ([ADR 0028](docs/adr/0028-an-applications-footprint-is-what-the-filesystem-says.md))
- **The Inspector**: what one application costs, and what removing it would touch.
- **A dashboard** that classifies the disk into kinds and draws a bar that runs to the
  volume rather than to the scan.
- Application icons, read from each bundle's `Info.plist`.
- A brand mark, drawn from what the name means, on Apple's icon grid.

### Changed

- **Three destinations instead of seven tabs**, after the first person to use the beta
  said the tabs were confusing.
- The scan strip is one line, so the page no longer moves while a scan runs.

### Fixed

- A scan stays on one volume, so a mounted disk no longer inflates the total.
- An application is the outermost bundle: nested helper apps are no longer counted twice,
  and none is cut from the list.
- Several ways the dashboard list and bar could overstate a total.
- Completeness is derived from the bytes a consumer actually counted, not assumed.

### Internal

- Lint sets ratcheted: `[workspace.lints]` across all crates, `clippy::unwrap_used` denied
  in shipping code, and oxlint's `perf` category enabled. The AppContext value is now
  memoised — it had a new identity every render, re-rendering every consumer on every scan
  progress event.
- `deny.toml`: the rule that no GPL code enters this project is now enforced by the build
  rather than by memory.

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

[unreleased]: https://github.com/Shishir435/nirmoka/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/Shishir435/nirmoka/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/Shishir435/nirmoka/releases/tag/v0.1.0
