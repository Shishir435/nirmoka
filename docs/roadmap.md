# Roadmap

Ordered by dependency, not by excitement. This file is the tracker — check boxes off as
work lands, and keep the **Current step** line accurate.

**Current step: 11 — Mole-powered macOS beta.**

## Sequencing rules

Three ordering decisions are load-bearing. Changing them costs more than it saves.

**ncdu adapter before Mole adapter.** ncdu is the narrowest backend and runs everywhere.
Building it first keeps the wire format honest and produces platform-neutral code on a
single macOS machine. Mole is macOS-only and far richer; leading with it bakes macOS
assumptions into `core` and leaves the ncdu path unimplementable.

**`nrmk scan` working before any UI exists.** This proves `core` is framework-independent
while violating that is still impossible. Build the UI first and the temptation to reach
for Tauri types inside `core` arrives before the boundary is established.

**Second adapter before the UI is polished.** If adding `adapter-mole` requires changing
`core`, the trait was wrong — and it is far cheaper to learn that at step 9 than at step 15.

---

## Step 0 — Workspace skeleton ✅

- [x] Name verified unclaimed: GitHub, npm, crates.io, Vercel, Cloudflare
- [x] Apache-2.0 license, `NOTICE.md` attribution for Mole / ncdu / gdu
- [x] Architecture, adapter contract, ADRs 0001–0007
- [x] GitHub repo created, landing page deployed to `nirmoka.vercel.app`
- [x] Cargo workspace: `core`, `adapter`, `adapter-ncdu`, `cli`
- [x] pnpm workspace: `apps/desktop`, `packages/transport`
- [x] `AGENTS.md` + `CLAUDE.md` symlink with the five invariants
- [x] CI: cross-platform Rust matrix, web build, grep-enforced invariants
- [x] `pnpm install` clean, `pnpm typecheck` and `pnpm build` green
- [x] Pre-push hook running the same checks CI does, sharing
      `scripts/check-invariants.sh` so the two cannot drift

## Step 1 — Core types and `nrmk` ✅

- [x] `crates/core`: `Node`, `NodeKind`, `Tree` (arena), `format_bytes`, `CoreError`
- [x] `Tree::rollup` bottom-up size aggregation, `Tree::path_of`, `children_by_size`
- [x] `crates/cli`: `nrmk backends`, table and `--json` output
- [x] Rust 1.97.1 installed; `cargo test --workspace` green (17 tests)
- [x] `cargo clippy --all-targets -- -D warnings` clean
- [x] CI green on macOS, Linux, and Windows

## Step 2 — Adapter contract ✅

- [x] `Adapter` trait: `id`, `display_name`, `supported_versions`, `detect`, `capabilities`
- [x] `Capabilities` flags and a `MINIMAL` constant (seven initially; undo added in step 10)
- [x] `Detection` with a distinct `UnsupportedVersion` state
- [x] `Registry`, push-based to avoid a dependency cycle
- [x] `AdapterError` exercised by tests for each variant, including the `NotInstalled`,
      `UnsupportedVersion`, `MalformedOutput`, and `Cancelled` variants added by steps 4–5

## Step 3 — ncdu adapter: detection ✅

- [x] Detect the binary, parse `ncdu --version`, gate to 2.x
- [x] Reject error text as a version (the "stderr as data" failure)
- [x] `Capabilities::MINIMAL` — no dry run, no Trash, and no pretending otherwise
- [x] `nrmk backends` verified against real ncdu 2.8.2 on macOS — reports `ok`
- [x] Version gate verified against a real ncdu 1.19 on Ubuntu CI — reports
      `unsupported` and exits non-zero. Ubuntu 24.04 shipping the 1.x series makes this
      a free regression test rather than a mock.
- [x] Resolve the absolute binary path cross-platform, with `PATHEXT` handling on Windows
      and an executable-bit check on Unix. CI asserts the reported path is absolute.
- [x] Exercise a usable 2.x backend on Linux, from upstream's static build. apt has no 2.x
      package, so without this entry Linux only ever covered the rejection path.

## Step 4 — ncdu wire format ✅

- [x] Parse ncdu JSON export v2 (`ncdu -o -`) into `Tree`. The format version is 1.2;
      the "v2" in ncdu's docs is the release series, not the format.
- [x] Record fixtures under `fixtures/ncdu/2.8.2/` via `scripts/record-ncdu-fixture.sh`
- [x] Handle hardlinks, sparse files, and read errors without silently under-reporting.
      Disk usage is the number and apparent size travels beside it — see ADR 0009.
- [x] Streaming parse — entries reach the sink during the decode, nothing buffers
- [x] Malformed-input tests: truncated JSON, wrong format version, empty file, trailing
      data, an item with no name, a reader that fails mid-stream
- [x] Nesting deeper than serde_json's 128-level limit, capped at 256 so a hostile export
      cannot overflow a 2 MB worker stack

## Step 5 — `nrmk scan` end to end ⭐ ✅

**The boundary is proven when this works.**

- [x] `Adapter::scan` added to the trait, streaming into a sink
- [x] Cancellation that actually kills the subprocess, with a test that checks the pid is
      gone rather than only that the call returned
- [x] `nrmk scan <path> --json`
- [x] `nrmk scan <path>` human table, largest first, with flags for unreadable, excluded,
      and deduplicated-hardlink entries
- [x] `nrmk scan --from-export <file>` for parsing a recorded export with no backend
- [x] Runs on a real home directory without exhausting memory: 2.2M entries, 399 MB peak
      RSS, 50s — the time is ncdu's, not the parser's

## Step 6 — Contract test suite ✅

- [x] `tests/contract/` — one suite every adapter must pass
- [x] Driven from recorded fixtures, no live backend needed
- [x] Wired into CI on all three platforms, including a Windows run with no backend at all

## Step 7 — Tauri shell ✅

- [x] `crates/app` — Tauri v2, six commands, no logic of its own. Detection, sizes, and
      ordering stay where the CLI and the contract suite can reach them.
- [x] Scans run on a worker thread with the `CancelToken` reaching a real stop button.
      Progress arrives as events every 25k entries rather than per entry.
- [x] `tauri.conf.json` pointing at `apps/desktop/dist`, pnpm as `beforeDevCommand`
- [x] `ts-rs` generating `packages/transport/src/generated/bindings.ts`, committed —
      see [ADR 0010](adr/0010-boundary-types-in-the-shell.md)
- [x] CI check: regenerating types produces no diff, and the `web` job builds the frontend
      with no Rust toolchain installed
- [x] Real `tauriTransport()` in `packages/transport`, with `resolveTransport()` falling
      back to the mock outside the shell so `pnpm dev` still renders
- [x] shadcn/ui initialised, button and badge added, full token set in `index.css`
- [x] `pnpm tauri dev` opens a window showing real backend detection
- [x] `rows` caps its own window size, so no caller can ask for the whole tree
- [x] Node ids are paired with the scan that issued them. An index alone identifies a
      slot, not a node: every scan numbers its tree from zero, so an id kept across a
      rescan would name a different directory rather than fail

## Step 8 — Tree view ✅

- [x] Virtualized list (TanStack Virtual) — invariant 5 applies from the first commit
- [x] IPC carries the visible window plus aggregates, never the whole tree — `useDirectory`
      holds a sparse array of `total` slots and asks for a 100-row chunk only when one of
      its rows is scrolled into view
- [x] Navigate in and out of directories. `RowPage.ancestors` carries the way back out, so
      "up" is a click rather than a rescan
- [x] Sort by size and by name, ordered in Rust before the window is cut —
      see [ADR 0011](adr/0011-ordering-and-paging-are-server-side.md)
- [x] Live scan progress that does not lie about completion: a count and the directory
      being walked, and deliberately no percentage. The backend does not know the total,
      so a progress bar would be an animation with a number attached
- [x] Designed empty, loading, error, and permission-denied states. An unreadable directory
      says so rather than rendering as an empty one

## Step 9 — Mole adapter ✅

- [x] `crates/adapter-mole`, macOS-gated at the adapter level
- [x] ~~Translate `mo analyze --json` down into the ncdu wire format~~ — **not possible.**
      `mo analyze --json` lists one directory's children, not a tree, and the analyzer
      accepts no depth or recursion flag. Rebuilding a tree from it would take one
      subprocess per directory. The adapter declares `scan: false` and returns
      `Unsupported`; ncdu stays the scanner. Recorded evidence in `fixtures/mole/1.48.1/`,
      reasoning in [ADR 0012](adr/0012-mole-is-not-a-scanner.md)
- [x] Extra abilities as `Capabilities` flags, not format extensions
- [x] Capability flags reaching the UI, hiding unsupported controls. `Capabilities` is now
      per backend in `RegistryEntry`, `dto::Backend`, the CLI's `SCANS` column, and the
      backend list — one set of flags for "the active backend" described neither once the
      two stopped overlapping
- [x] Passes the step 6 contract suite unchanged. The suite gained a capability split
      (`for_each_scanner`, and a promise that a non-scanner refuses) rather than a
      backend-specific case
- [x] **If this required changing `core`, fix the trait now** — it did not. `core` and the
      `Adapter` trait are both untouched by this step: `Capabilities` was already the
      mechanism for "this backend cannot do that", and it turned out to cover the headline
      ability too

## Step 9.5 — Backend selection ✅

Unplanned, and forced by step 9. Once two backends stopped overlapping, "the backend" had no
single referent and registration order — a constant compiled into `main` — was deciding
something that belongs to the user and to the platform.

- [x] `Registry::resolve(ability, preference)`: the user's choice, then the platform default,
      then registration order, each filtered by whether the adapter can do the thing
- [x] Platform defaults — macOS `mole, rip, ncdu, gdu`; Windows
      `gdu, rip, ncdu, mole`; else `ncdu, rip, gdu, mole`. Matched on
      `std::env::consts::OS`, so every platform's default is
      testable from every platform rather than only from the job that runs there
- [x] A preference is not an override: choosing Mole on macOS is honoured for cleanup and
      falls back to ncdu for scanning, and `Choice::instead_of` names the displaced backend
      so the fallback is stated rather than looking like the setting was ignored
- [x] The reason a preference went unmet is never guessed. "Cannot scan" and "is not
      installed" have different fixes; the window reads the reason from detection it already
      has, and the CLI states the fact and points at `nrmk backends`
- [x] Stored in `settings.json` via `directories`. A corrupt or missing file degrades to the
      default rather than refusing to start
- [x] Picker in the window, shown only with two or more usable backends. `nrmk --backend <id>`
      for the harness, which deliberately does not read the GUI's settings file
- [x] CI asserts against the binaries: preferring a backend that cannot scan must still
      report `SCANS WITH ncdu`, name the unmet preference, and complete a real scan — on
      macOS where Mole is installed and cannot scan, and on Linux where it is absent
- [x] [ADR 0013](adr/0013-the-backend-is-a-choice.md)

## Step 10 — Deletion ✅

Nothing here ships without tests.

- [x] Verify the backend command surface before designing the destructive API. ncdu 2.8.2
      deletes only inside its interactive browser; Mole 1.48.1 cleans curated categories
      and uninstalls named apps, but neither exposes arbitrary selected-path deletion — see
      [ADR 0014](adr/0014-interactive-deletion-is-not-an-adapter-api.md)
- [x] Shared deletion validator, with tests for relative paths, scan-root deletion,
      containment, symlink escape, missing targets, and system-critical locations. It is
      shared by preparation and final execution rather than copied into the rip adapter
- [x] Path validation was wired at the `adapter-rip` boundary and repeated immediately before
      spawning; ADR 0017 records why this was still insufficient and deletion was withdrawn
- [x] Dry-run preview where the backend supports it. No selected-path backend currently
      does, so `dryRun` remains false rather than showing a guessed preview
- [x] Explicit confirmation boundary for new deletion. No current backend can execute an
      execution-bound selected path, so the capability stays unavailable and no confirmation
      UI is shown
- [x] Recoverable deletion policy closed fail-safe for v0.1. rip 0.13.x was withdrawn by
      ADR 0017 rather than exposing a pathname race; ADR 0018 defers new selected-path deletion
      until a backend can bind execution to the validated object
- [x] Exact non-interactive `--unbury` remains available for existing rip receipts
- [x] A readable append-only JSON Lines operation log, reloaded across launches; a failed
      durable append cannot create an in-memory success

## Step 11 — Mole-powered macOS beta

The macOS beta must close the loop for a non-technical user: find a storage problem,
review an exact backend-produced plan, approve it, run it through Mole, and understand the
result. A read-only explorer remains useful infrastructure, but it is not the consumer beta.
See [ADR 0019](adr/0019-mole-consumer-operations-before-beta.md).

### Phase 0 — command evidence and contracts

- [x] Verify Mole 1.48.1 command surface from the installed release
- [x] Confirm `mo status --json` is a one-shot machine-readable status source
- [x] Confirm `mo uninstall --list` emits JSON when stdout is not a terminal
- [x] Confirm targeted `mo uninstall --dry-run <name>` exists but its plan is human-readable
- [x] Confirm `mo clean --dry-run` publishes an exact grouped preview file, not JSON
- [x] Confirm clean category-selection flags were removed and execution performs a fresh discovery
- [x] Record sanitized, fixture-driven schemas without committing machine-specific paths or metrics

### Phase 1 — read-only Mole ports

- [x] Add capability-specific adapter contracts; do not widen selected-path deletion
- [x] Parse and expose `mo status --json`, version-gated and cancellable
- [x] Parse and expose `mo uninstall --list` application inventory
- [x] Add malformed-output, cancellation, unsupported-version, and contract tests

### Phase 2 — exact cleanup review

- [x] Parse Mole's generated cleanup preview into typed categories, paths, sizes, and warnings
- [x] Keep every path backend-produced; never copy Mole cleanup or protection tables
- [x] Show full/partial preview state when system-level candidates require authorization
- [x] Design selection honestly around Mole's supported command surface

### Phase 3 — cleanup execution

- [x] Add a one-time confirmation boundary for the reviewed operation
- [x] State and test that Mole re-discovers candidates at execution time
- [ ] Handle authorization, cancellation, partial failure, and backend version changes
- [ ] Append exact results to the durable operation journal

### Phase 4 — application uninstall

- [ ] Use Mole's uninstall name as the backend identifier; display names are not commands
- [ ] Preview application bundle, leftovers, sensitive/review-only items, and recovery mode
- [ ] Execute through Mole with explicit confirmation and partial-result reporting
- [ ] Default to Mole's recoverable Trash route; permanent removal is not a beta default

### Phase 5 — consumer navigation and accessibility

- [ ] Add selected-row keyboard navigation: arrows, Page Up/Down, Home/End
- [ ] Add Enter/Right to open, Left/Backspace to go up, and back/forward history
- [ ] Add Quick Look, Reveal in Finder, visible focus, and VoiceOver labels
- [ ] Add frontend smoke tests for missing backends, scan/cancel/rescan, large directories,
      cleanup review, confirmation, failure, and uninstall

### Phase 6 — release

- [x] `adapter-gdu` for the Windows path — gdu 5.32.x, native ncdu-format export,
      fixture-driven contract coverage, cancellation, and a real Windows CI scan. Unsupported
      scan options fail closed rather than changing semantics — see
      [ADR 0015](adr/0015-gdu-is-the-windows-scanner.md)
- [ ] Signed macOS build
- [ ] Linux AppImage or Flatpak
- [ ] CI builds installers for all three platforms
- [x] Pin `rust-toolchain.toml` to Rust 1.97.1
- [ ] First tagged release

---

## Explicitly out of scope

Written down so they can be declined quickly later.

- **A built-in disk scanner.** The premise is that scanning is solved.
- **Bundled backend binaries.** Detect and guide; do not redistribute. Avoids inheriting
  GPL distribution obligations and is more honest about what executes.
- **`nrmk` as a shipped product.** It is a dev and CI harness. A CLI wrapping a CLI muddies
  the product story. See ADR 0007.
- **Reimplementing any backend's curated cleanup lists.** Legally risky under GPL-3.0 and
  immediately stale.
- **Background monitoring, menu bar agents, scheduled cleanups.** This is a tool you open.
- **Mobile.** Tauri can target it. There is no disk to browse there.
