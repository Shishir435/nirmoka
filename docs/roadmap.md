# Roadmap

Ordered by dependency, not by excitement. This file is the tracker — check boxes off as
work lands, and keep the **Current step** line accurate.

**Current step: 13 — three destinations.** Step 11 shipped as 0.1.1: `brew install
nirmoka/tap/nirmoka` installs a working macOS beta, and it is read-only. Step 12 added the verb the
product was missing — see [ADR 0025](adr/0025-move-to-trash-is-a-platform-integration.md) — and the
first person to use it said the seven tabs were confusing, which step 13 answers. **0.2.0 ships both.**
Releases are unsigned until there is an Apple Developer account, which is why the install path is a
Homebrew formula rather than a download —
[ADR 0024](adr/0024-distribution-is-a-source-built-homebrew-formula.md).

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
- [x] Handle authorization, cancellation, partial failure, and backend version changes
- [x] Append exact results to the durable operation journal. One journal, one id space, a
      `cleaned` event carrying the reviewed evidence and the backend's reported scope, completion,
      and warnings — and no per-path receipt, because Mole publishes none. A failed append reports
      the run beside the error rather than hiding a removal that already happened; the inverse of
      the deletion rule, and why — see
      [ADR 0020](adr/0020-cleanup-runs-are-journalled-without-a-receipt.md)

### Phase 4 — application uninstall ✅

Closed as **not possible against Mole 1.48.1**, on recorded evidence rather than on a guess. Every
named `mo uninstall` — `--dry-run` included — matches the app and then blocks on
`Proceed with uninstallation? [y/N]`, and the release exposes no non-interactive flag. The plan is
behind the prompt, so previewing and executing are the same unreachable call. See
[ADR 0021](adr/0021-application-uninstall-is-not-an-adapter-api.md).

- [x] Use Mole's uninstall name as the backend identifier; display names are not commands. It
      crosses the boundary unchanged and the window shows it, which is what makes the Terminal
      fallback usable rather than a guess at what `mo uninstall` accepts
- [x] ~~Preview application bundle, leftovers, sensitive/review-only items, and recovery mode~~ —
      **not possible.** `mo uninstall --dry-run <app>` prints its plan only after the confirmation
      prompt is answered; with stdin closed it exits 1 having printed nothing but the match.
      Recorded in `fixtures/mole/1.48.1/uninstall-command-surface.txt`
- [x] ~~Execute through Mole with explicit confirmation and partial-result reporting~~ —
      **not possible.** The only way past Mole's prompt is writing to its stdin, which would mean
      answering another tool's safety gate on the user's behalf. `uninstall_apps` is false and the
      `Adapter` trait gains no uninstall method
- [x] Default to Mole's recoverable Trash route; permanent removal is not a beta default. Satisfied
      by construction: Mole trashes by default, `--permanent` is opt-in, and Nirmoka never invokes
      uninstall at all
- [x] Capability split so the honest answer is expressible: `app_inventory` for listing, which Mole
      can do, separate from `uninstall_apps` for removing, which it cannot. One flag for both would
      have hidden a working inventory or offered a removal that dies at a prompt
- [x] A test over the recorded command surface fails if a Mole release documents a way past the
      prompt, so the decision is re-checked on upgrade rather than remembered

### Phase 5 — consumer navigation and accessibility ✅

- [x] Add selected-row keyboard navigation: arrows, Page Up/Down, Home/End. Which key means what
      is a pure function in `row-keyboard.ts`, so the awkward cases — nothing selected yet, an
      empty directory, a page jump past the end — are tested rather than tried
- [x] Add Enter/Right to open, Left/Backspace to go up, and back/forward history. History is an
      array and a cursor in `space-navigation.ts`; opening a directory discards the forward branch,
      and a rescan starts a fresh history because node ids are per-scan arena indices
- [x] Add Quick Look, Reveal in Finder, visible focus, and VoiceOver labels. The list is a
      `listbox` driven by `aria-activedescendant` rather than by focusing rows, which is what
      survives the virtualizer unmounting a focused row as it scrolls away. Reveal and Quick Look
      are shell integrations reported by `platform_features`, not `Capabilities` flags, and they
      take a scan-and-node pair so Rust resolves the path — see
      [ADR 0022](adr/0022-shell-integrations-are-not-adapter-abilities.md)
- [x] Add frontend smoke tests for missing backends, scan/cancel/rescan, large directories,
      cleanup review, confirmation, failure, and uninstall. No DOM harness was added: the decisions
      worth testing were extracted into pure modules — `scan-machine`, `cleanup-flow`,
      `backend-gating`, `chunk-window`, `row-keyboard`, `space-navigation` — which the existing
      `node --test` runner drives directly. 41 tests, and the pages now read their gating from the
      same functions the tests assert on

### Phase 6 — release

- [x] `adapter-gdu` for the Windows path — gdu 5.32.x, native ncdu-format export,
      fixture-driven contract coverage, cancellation, and a real Windows CI scan. Unsupported
      scan options fail closed rather than changing semantics — see
      [ADR 0015](adr/0015-gdu-is-the-windows-scanner.md)
- [x] Release pipeline for a signed macOS build. `.github/workflows/release.yml` builds a universal
      Apple silicon and Intel bundle on a `v*` tag, refuses a tag that disagrees with the bundled
      version, signs and notarizes when the credentials are present, verifies the result with
      `codesign` and `spctl`, and opens a **draft** release only after those checks pass. An unsigned
      tag fails unless someone set `ALLOW_UNSIGNED_RELEASE=true`, so an unsigned artifact cannot reach
      a release by accident — see `docs/releasing.md`
- [x] A supported install that works without a certificate. `packaging/homebrew/nirmoka.rb` is a
      source-built formula for the org tap: `brew install nirmoka/tap/nirmoka`. Homebrew removed
      `--no-quarantine`, so a cask of an unsigned `.dmg` is refused by Gatekeeper, while a formula
      compiles locally and is never quarantined — see
      [ADR 0024](adr/0024-distribution-is-a-source-built-homebrew-formula.md)
- [x] `nirmoka/homebrew-tap` created, holding `Formula/nirmoka.rb`. A tap must be its own
      repository, and only the account holder can make one
- [ ] Signing credentials in repository secrets. Needs a paid Apple Developer account and a
      Developer ID Application certificate, which only the account holder can produce. Until then
      releases are unsigned and Homebrew is the install path
- [x] ~~Linux AppImage or Flatpak~~ — **dropped.** The shipped product is the Mole cleanup loop, and
      Mole is macOS-only; off macOS, Nirmoka is a browser for tools that already have good terminal
      interfaces. A second cleanup backend is what reopens this, not demand for a package. See
      [ADR 0023](adr/0023-the-first-release-is-macos-only.md)
- [x] ~~CI builds installers for all three platforms~~ — one platform is packaged. CI still compiles
      and tests the workspace, and the Linux and Windows matrix entries stay commented in place for
      the end of the beta rather than deleted
- [x] Pin `rust-toolchain.toml` to Rust 1.97.1
- [x] First tagged release. v0.1.0 rehearsed the pipeline and found two failures only a tag could
      reach — the pinned toolchain missing a darwin target, and Tauri signing on an _empty_
      `APPLE_CERTIFICATE` — so it was superseded by **v0.1.1**, which is what the tap serves. A
      third bug survived to a user: a windowed process inherits launchd's `PATH`, so every backend
      read as missing until adapters searched the package-manager directories themselves

## Step 12 — Move to Trash

The beta is installable and read-only. A user finds the 21 GiB directory and leaves for Finder,
which is the loop this step closes. Recoverable removal only — permanent selected-path deletion
stays deferred under ADR 0017's gate, and nothing here weakens it. Reasoning, and the residual race
it does _not_ close, in [ADR 0025](adr/0025-move-to-trash-is-a-platform-integration.md).

Nothing here ships without tests.

### Phase 1 — the decision

- [x] [ADR 0025](adr/0025-move-to-trash-is-a-platform-integration.md): Trash is a platform
      integration under the ADR 0022 precedent, not an adapter capability. Every adapter keeps
      `delete: false` and `trash: false`
- [x] ADR 0018 marked partly superseded — recoverable removal only
- [x] Roadmap caught up with what 0.1.1 actually shipped

### Phase 2 — the operation

- [x] `crates/app/src/trash.rs`, beside `reveal.rs`. Validate through the shared
      `validate_delete_target`, validate again immediately before the move, then move — a
      confirmation dialog can sit open for as long as the user leaves it open
- [x] The `trash` crate (5.2.6, MIT) rather than a subprocess: the platform's own Trash service,
      no optional install, and Windows and freedesktop for free. Default features off; they add
      `chrono` only to list Trash contents, which macOS does not support. On macOS the move goes
      through the Finder rather than `trashItemAtURL:`, because Put Back is the property
      [ADR 0025](adr/0025-move-to-trash-is-a-platform-integration.md) rests on and only the Finder
      route records it — so a refused Apple event names the setting to grant
- [x] `PlatformFeatures` gains the platform's own wording and whether the operation is offered
- [x] Tests for an empty path, a path that is gone, the scan root, a symlink escaping the root, a
      system-critical location, and a real round trip into the Trash. Eleven tests; the round trip
      really moves a file, because a mock would prove only that the mock was called

### Phase 3 — the boundary

- [x] A `Trashed` journal event with no recovery path. The crate cannot enumerate or restore the
      macOS Trash, and guessing a name inside `~/.Trash` would be a receipt that does not resolve.
      It shares the deletion id space and survives reload
- [x] A failed journal append reports the move beside the error rather than failing it — ADR 0020's
      rule, because the Trash is its own record
- [x] Prepare and confirm commands through the existing one-time token; no raw path crosses back in.
      The confirmation carries the resolved path, not the clicked one, and the pending slot is an
      enum rather than a second map, so a prepared cleanup and a prepared move cannot both be live
- [x] `TrashPreparation` is deliberately not `DeletePreparation`: that type carries a backend, the
      backend it displaced, and whether a dry run happened — three questions with no answer when no
      backend is involved
- [x] `pnpm types`, with the regenerated bindings committed

### Phase 4 — Space

- [x] Trash the selected row, with the platform's wording and a confirmation naming the resolved
      path and size — the path Rust checked, not the name that was clicked
- [x] ⌘⌫, handled ahead of the "modified keys belong to the platform" guard because it is exactly
      a platform gesture. It opens the same confirmation the button does
- [x] Say that Put Back in Finder is the undo, rather than offering an Undo button that guesses
- [x] ~~The removed row leaves the view~~ — **it stays, struck through and labelled.** Removing it
      would renumber the list under the virtualizer while Rust's `total` still counts it, and the
      size beside it was measured before the move. A marked row says both things; a missing row
      says neither. A line under the list states that totals predate the removals
- [x] Gating and confirmation state as pure modules, driven by the existing `node --test` runner —
      `trash-flow.ts`, 9 tests

### Phase 5 — Applications

- [x] Trash an application bundle from the scan-derived list
- [x] **Only the scan-derived list.** Mole's inventory reports a path; the scan reports a node, and
      a node is the only thing Rust can resolve from its own tree. Accepting a path back from the
      window would undo the property that makes every other destructive call checkable, for one
      screen. When Mole's list is showing, the page says to scan `/Applications` instead of hiding
      an action with no explanation
- [x] State plainly that leftovers stay: this moves the bundle and nothing else. A full uninstall
      needs Mole, which cannot be driven past its own prompt —
      [ADR 0021](adr/0021-application-uninstall-is-not-an-adapter-api.md)
- [x] One confirmation dialog, shared with the browser. Two copies of a destructive confirmation
      drift, and the copy that drifts is the one nobody was looking at

### Phase 6 — verified

- [x] `cargo fmt`, `clippy -D warnings`, `cargo test --workspace`, `pnpm typecheck && pnpm build`,
      `pnpm lint`, and `scripts/check-invariants.sh`. 315 Rust tests and 61 frontend tests pass,
      and regenerating the bindings produces no diff
- [x] `pnpm tauri dev` and trash something from the window. A command that compiles and a window
      that moves a file are different claims. Confirmed from the real window on macOS 26
- [ ] Tagging moved to step 13, so 0.2.0 ships the verb and a window worth using it in

## Step 13 — Three destinations

The first person to use the beta said the tabs were confusing, and they were right: seven nav items
for three jobs, five of them reading the same scan. Nothing here changes what a backend does — no
command added, none widened, no `Capabilities` flag moved. See
[ADR 0026](adr/0026-the-window-has-three-destinations.md).

- [x] **Storage, Clean, Activity.** Help and Settings become header controls; the sidebar carries
      three items and one honest badge
- [x] One scan bar, in the shell header. Rust holds exactly one scan, so two sets of controls
      described a second one that does not exist
- [x] Overview, Space Explorer, Developer, Applications, and System Status become sections under
      `pages/sections/`, and Storage switches between Folders, Developer, and Applications as views
      of the same tree. Applications is the one view with another source, so it still works with no
      scan: Mole reports what is installed either way
- [x] System status loads when its section is opened. It was a tab that ran `mo status` on arrival,
      to report battery health on a disk cleanup tool
- [x] Retired hashes redirect rather than falling through to a default — `#/overview`, `#/space`,
      `#/status` to Storage, `#/developer` and `#/applications` to their view. `route.ts`, 7 tests
- [x] **Activity shows all three journals.** It read only `operation_log()`, so a file the user had
      just moved to the Trash appeared in no history at all while cleanup history sat on the Clean
      page. `activity-feed.ts` merges them newest first and breaks ties on the shared id, which is
      exact rather than arbitrary. 8 tests
- [x] Recovery is stated per kind and none of them is a fake button: Put Back for the Trash, nothing
      for a cleanup run because Mole publishes no receipt, Undo only for a recorded recoverable
      deletion
- [x] The "Read Only Mode" badge is gone. It was already half wrong in 0.1.1, because Clean executes
      real cleanups, and fully wrong once Trash landed. What is unavailable is _permanent_
      selected-path deletion, which is ADR 0017's gate and not a mode
- [ ] `pnpm tauri dev` and use all three destinations. Scan, trash a row, check it appears in
      Activity, and open an old `#/overview` link
- [ ] 0.2.0, tagged, with the tap updated

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
