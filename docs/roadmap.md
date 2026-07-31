# Roadmap

Ordered by dependency, not by excitement. This file is the tracker — check boxes off as
work lands, and keep the **Current step** line accurate.

**Current step: 8 — the tree view.**

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
- [x] `Capabilities` with seven flags and a `MINIMAL` constant
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
- [x] Node ids arriving from the webview are validated against the current tree rather
      than trusted as indices

## Step 8 — Tree view

- [ ] Virtualized list (TanStack Virtual) — invariant 5 applies from the first commit
- [ ] IPC carries the visible window plus aggregates, never the whole tree — the `rows`
      command and its cap already exist; step 8 is the UI that pages through them
- [ ] Navigate in and out of directories
- [ ] Sort by size and by name
- [ ] Live scan progress that does not lie about completion
- [ ] Designed empty, loading, error, and permission-denied states

## Step 9 — Mole adapter

- [ ] `crates/adapter-mole`, macOS-gated at the adapter level
- [ ] Translate `mo analyze --json` down into the ncdu wire format
- [ ] Extra abilities as `Capabilities` flags, not format extensions
- [ ] Capability flags reaching the UI, hiding unsupported controls
- [ ] Passes the step 6 contract suite unchanged
- [ ] **If this required changing `core`, fix the trait now**

## Step 10 — Deletion

Nothing here ships without tests.

- [ ] Path validation at the adapter boundary: absolute, canonicalised, symlinks resolved,
      inside the scan root, not system-critical
- [ ] Dry-run preview where the backend supports it
- [ ] Explicit confirmation where it does not
- [ ] Trash where available, permanent clearly marked where not
- [ ] Undo affordance for trashed items
- [ ] A readable operation log

## Step 11 — Ship

- [ ] `adapter-gdu` for the Windows path
- [ ] Signed macOS build
- [ ] Linux AppImage or Flatpak
- [ ] CI builds installers for all three platforms
- [ ] Pin `rust-toolchain.toml` to an exact version
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
