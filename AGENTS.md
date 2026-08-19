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

| Layer            | Technology                                                       |
| ---------------- | ---------------------------------------------------------------- |
| Shell            | Tauri v2 — `crates/app`                                          |
| Core, adapters   | Rust — `crates/*`                                                |
| Frontend         | React 19 + TypeScript + Tailwind v4 + shadcn/ui — `apps/desktop` |
| Wire format      | ncdu JSON export                                                 |
| Package managers | Cargo (`crates/*`), pnpm (`apps/*`, `packages/*`)                |

## The Five Invariants

These are structural. Breaking one collapses the architecture, and CI enforces every one
of them. If a task seems to require breaking one, stop and say so rather than working
around it.

**1. `crates/core` depends on nothing but the standard library, serde, and thiserror.**
No `tauri`. No `nirmoka-adapter*`. No GUI framework. `crates/cli` exists to make a
violation a build failure rather than a review miss.

**2. `packages/transport` is the only place that may import `@tauri-apps/*`.**
Components import from `@nirmoka/transport`. If a component calls `invoke()` directly, the
React code is welded to Tauri and the escape route becomes fiction.

**3. No `#[cfg(target_os)]` in `crates/core`.** Platform specifics live in adapters. Paths
are `PathBuf`, never string concatenation. Home/cache/config directories come from the
`directories` crate, never from `~/Library` or `/Users/` literals.

**4. The wire format is ncdu's JSON export, not Mole's richer output.** Building against
the narrowest backend is what keeps the abstraction honest. New backend abilities are
`Capabilities` flags. Widening the wire format requires a new ADR.

**5. The tree lives in Rust. The frontend receives only the visible window.** A home
directory scan is 500k–2M nodes. Shipping that into a webview and rendering it as DOM is
the mistake that gets misattributed to Tauri; Electron would crawl identically.

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

## Commands

```bash
pnpm install                 # JS workspace
pnpm dev                     # frontend dev server on :5173 (strict port)
pnpm build                   # typecheck + vite build
pnpm typecheck               # every JS package
pnpm format                  # prettier

cargo check --workspace --all-targets
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

cargo run -p nirmoka-cli -- backends
cargo run -p nirmoka-cli -- backends --json
pnpm nrmk backends           # same thing via pnpm
pnpm nrmk --backend mole backends    # prefer one; it falls back where it cannot

pnpm tauri dev               # the desktop shell (starts Vite itself)
pnpm types                   # regenerate packages/transport/src/generated/bindings.ts

pnpm nrmk uninstall --list                    # apps Mole can address, with its identifiers
pnpm nrmk uninstall localsend                 # Mole's own plan; preview only, removes nothing
pnpm nrmk uninstall localsend --transcript    # the backend's output verbatim

pnpm nrmk scan ~/Downloads                    # real backend, largest first
pnpm nrmk scan . --json --depth 2 --limit 5
pnpm nrmk scan --from-export fixtures/ncdu/2.8.2/simple.json   # no backend needed

./scripts/generate-icons.sh                   # rebuild crates/app/icons from assets/nirmoka-mark.svg
./scripts/record-ncdu-fixture.sh              # re-record fixtures after an ncdu upgrade
./scripts/record-gdu-fixture.sh               # re-record fixtures after a gdu upgrade
./scripts/record-mole-fixture.sh              # re-record the evidence behind ADR 0012
```

## Verification

- **Rust changes:** `cargo fmt --all`, then `cargo clippy --workspace --all-targets -- -D
warnings`, then `cargo test --workspace`.
- **Frontend changes:** `pnpm typecheck && pnpm build`.
- **Adapter changes:** `cargo test --workspace`, then `cargo run -p nirmoka-cli --
backends` and `-- scan <dir>` against a real backend. The shared suite in
  `tests/contract` must pass unchanged; needing a special case there means the trait is
  wrong.
- **Boundary types (`crates/app/src/dto.rs`):** `pnpm types`, then commit the regenerated
  `packages/transport/src/generated/bindings.ts`. CI fails on a diff, and so does the
  pre-push hook.
- **Shell changes:** `pnpm tauri dev` and use the window. A command that compiles and a
  window that shows the result are different claims.
- **Anything touching deletion:** tests first, no exceptions.
- Never pipe a test or check run into `head`/`tail` — the pipeline reports the pager's exit
  code, so a red run reads green. Let it print in full, or capture to a file and check the
  status separately.

## Working Rules

- **Follow `docs/roadmap.md` order.** The sequencing is deliberate, especially "ncdu
  adapter before Mole adapter" and "`nrmk scan` working before any UI". Building the Mole
  adapter first would bake macOS assumptions into `core`; building the UI first would
  invite violating invariant 5.
- **Record decisions as ADRs.** Anything that will matter in six months goes in
  `docs/adr/`, numbered sequentially, never deleted. A reversed decision gets a new ADR
  marking the old one superseded.
- **A new dependency needs a stated reason.** Both `Cargo.toml` and `package.json` lists
  are deliberately short. Prefer the standard library.
- **Do not add tooling before it hurts.** No Turborepo until builds are slow. No
  `packages/ui` until a second consumer exists. Speculative structure costs more than it
  saves at this size.
- **Generated types are committed.** `ts-rs` output lands in
  `packages/transport/src/generated/` and is checked in, so the frontend builds without a
  Rust toolchain. CI regenerates and fails on a diff.
- **`site/` is deliberately outside the pnpm workspace.** It is a zero-dependency static
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
crates/app/           Tauri shell — commands, DTOs, ts-rs bindings

tests/contract/       one suite every adapter must pass, fixture-driven
fixtures/             recorded backend output, per backend and version

apps/desktop/         React frontend. The list is virtualized and paged — see ADR 0011
packages/transport/   THE IPC boundary. Only module that knows about Tauri.

packaging/homebrew/   the tap formula, source of truth — see ADR 0024
site/                 static landing page, deployed to Vercel, outside pnpm workspace
docs/                 architecture, adapter contract, roadmap, ADRs
```

## Platform Status

Development is macOS-first because that is the machine available, but **the code stays
platform-neutral**. The mechanism that makes this work: the first backend is ncdu, which
runs everywhere, so cross-platform-shaped code gets written and tested on one machine.

CI ran `cargo check` and `cargo test` on macOS, Linux, and Windows from step 0. For the duration
of the macOS beta the Linux and Windows matrix entries are commented out in `.github/workflows/ci.yml`
rather than deleted, and the release pipeline packages macOS only — see
[ADR 0023](docs/adr/0023-the-first-release-is-macos-only.md). Mole is macOS-only (`install.sh`
refuses other platforms; every `cmd/analyze/*.go` file is `//go:build darwin`), so it is a
capability upgrade on one platform, never the baseline. Code stays platform-neutral regardless:
that is what makes uncommenting those entries a one-line change instead of a project.
