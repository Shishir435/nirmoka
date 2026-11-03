# Nirmoka Agent Guide

Shared source of truth for any AI agent working on this repo (Claude Code, Codex, etc.).
`CLAUDE.md` is a symlink to this file. Put personal overrides in `AGENTS.local.md` /
`CLAUDE.local.md`; both are gitignored.

## Project

Nirmoka is a cross-platform desktop GUI for disk analysis and cleanup. It writes **no disk
scanner of its own** — it drives existing CLI tools (ncdu, gdu, Mole, rip) as subprocesses
behind an adapter layer, and focuses entirely on being a good interface to them.

Read `docs/architecture.md` before your first change. `docs/roadmap.md` has the current
step and what is in scope.

## Stack

- Desktop: SwiftUI / AppKit, `apps/macos` (macOS 13+, Swift 6).
- Service: Rust, `crates/app`, private JSON lines over stdin/stdout.
- Core and adapters: Rust, `crates/*`; ncdu JSON export remains the scan wire format.
- Build: Cargo and Swift Package Manager. `site/` remains independent.
- React/Tauri and TypeScript transport are retired; see ADR 0033.

## The Five Invariants

These are structural. Breaking one collapses the architecture, and CI enforces every one
of them. If a task seems to require breaking one, stop and say so rather than working
around it.

**1. `crates/core` depends on nothing but the standard library, serde, and thiserror.**
No `tauri`. No `nirmoka-adapter*`. No GUI framework. `crates/cli` exists to make a
violation a build failure rather than a review miss.

**2. Swift views use AppModel; NativeServiceClient is the only process transport.**
Rust owns validation, backend selection, scan trees, and one-time confirmation tokens.
Do not reintroduce Tauri or a second desktop shell.

**3. No `#[cfg(target_os)]` in `crates/core`.** Platform specifics live in adapters. Paths
are `PathBuf`, never string concatenation. Home/cache/config directories come from the
`directories` crate, never from `~/Library` or `/Users/` literals.

**4. The wire format is ncdu's JSON export, not Mole's richer output.** Building against
the narrowest backend is what keeps the abstraction honest. New backend abilities are
`Capabilities` flags. Widening the wire format requires a new ADR.

**5. The tree lives in Rust. The native frontend receives only bounded row windows.** A home
directory scan is 500k–2M nodes. Keep paging and sorting in Rust; do not ship the whole tree into Swift.

## Safety Rules

Deletion is the entire risk surface. Everything else is recoverable.

- **Adapters own path validation.** A path is validated at the adapter boundary before it
  becomes a subprocess argument. The UI never assembles a delete command.
- **Never reimplement a backend's safety rules.** Mole's `should_protect_path()` and its
  curated cleanup lists are stricter than anything this project should attempt. Call the
  backend and let it apply them.
- **Never copy Mole's data tables into this repo.** Mole is GPL-3.0. Transcribing its
  protected-path arrays or cleanup target lists — even "just as data" — makes Nirmoka a
  derivative work and silently relicenses the project. See `NOTICE.md`.
- **Degrade, don't lie.** If a backend cannot do something, report `Unsupported`. An
  adapter must never fake a dry-run preview by guessing what the backend would delete.
- **Version-gate every backend.** Output formats drift silently, and a changed field on a
  delete path is the worst place to discover it. An untested version is
  `UnsupportedVersion`, not an optimistic `Found`.
- **Cancellation must kill the subprocess**, not orphan it. Every adapter needs a test.

## Commands and verification

```sh
apps/macos/run-local.sh
apps/macos/build-app.sh
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy --workspace -- -D clippy::unwrap_used
cargo test --workspace
cargo build -p nirmoka-app --bin nirmoka-native-service
NIRMOKA_NATIVE_SERVICE="$PWD/target/debug/nirmoka-native-service" swift test --package-path apps/macos
./scripts/check-invariants.sh
cargo deny check
cargo run -p nirmoka-cli -- backends
cargo run -p nirmoka-cli -- scan crates/core
```

Rust changes require fmt, clippy, no-unwrap, and workspace tests. Native changes
require Swift tests, a bundle build, and use of the actual window. New dependencies
need a stated reason and cargo-deny. Adapter changes require the shared contract
suite and a real backend scan. Anything touching deletion requires tests first.
Never pipe test/check output into head/tail; capture output and check its status.

## Working Rules

- **Follow `docs/roadmap.md` order.** The sequencing is deliberate, especially "ncdu
  adapter before Mole adapter" and "`nrmk scan` working before any UI". Building the Mole
  adapter first would bake macOS assumptions into `core`; building the UI first would
  invite violating invariant 5.
- **Record decisions as ADRs.** Anything that will matter in six months goes in
  `docs/adr/`, numbered sequentially, never deleted. A reversed decision gets a new ADR
  marking the old one superseded.
- **Lint sets are deliberately narrow.** Keep workspace Rust lints measured and
  actionable. Suppressions carry the reason on the line above them.
- **A new dependency needs a stated reason.** Both `Cargo.toml` and `package.json` lists
  are deliberately short. Prefer the standard library.
- **Do not add tooling before it hurts.** No Turborepo until builds are slow. No
  `packages/ui` until a second consumer exists. Speculative structure costs more than it
  saves at this size.
- **Boundary DTOs need contract tests.** Rust serializes JSON; Swift decodes it.
  Run native tests against the real Rust helper after changing boundary types.
- **`site/` is deliberately independent.** It is a zero-dependency static
  landing page deployed separately to Vercel. Do not pull it into the app build graph.
- Do not add AI attribution trailers to commits.

## Repository Map

```
crates/core/          domain model, tree, sizes, policy — framework-independent
crates/adapter/       Adapter trait, Capabilities, Detection, Registry
crates/adapter-ncdu/  baseline backend (cross-platform)
crates/adapter-gdu/   cross-platform scanner; primary Windows backend
crates/adapter-mole/  macOS-only cleanup backend. Does not scan — see ADR 0012
crates/adapter-rip/   exact undo for existing receipts; new deletion refused
crates/cli/           bin `nrmk` — headless harness, not shipped
crates/app/           Rust application service — commands, JSON DTOs, state

tests/contract/       one suite every adapter must pass, fixture-driven
fixtures/             recorded backend output, per backend and version

apps/macos/           The only desktop app — SwiftUI, AppModel, NativeServiceClient

packaging/homebrew/   the tap formula, source of truth — see ADR 0024
site/                 static landing page, deployed to Vercel, independent static hosting
docs/                 architecture, adapter contract, roadmap, ADRs
```

## Platform Status

The only desktop interface is native macOS SwiftUI (ADR 0033). Core and adapters
remain platform-neutral: no platform conditionals or GUI dependencies in core.
The Rust CI matrix retains commented Linux/Windows entries for future backend
coverage. The desktop release builds macOS only, for Apple silicon and Intel.
