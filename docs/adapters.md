# Adapter Contract

An adapter teaches Nirmoka to drive one disk tool. This document is the contract every
adapter must satisfy.

## Responsibilities

An adapter is responsible for:

1. **Detection** — is this backend installed, and where?
2. **Version gating** — is the installed version one this adapter understands?
3. **Capability reporting** — what can this backend actually do?
4. **Scanning** — run the backend, stream results as ncdu-format JSON. Capability-gated:
   a backend that cannot walk a tree declares `scan: false` and refuses, rather than
   returning the part of one it could manage.
5. **Deletion** — when the backend exposes a non-interactive selected-path command,
   validate the path, then remove it via the backend's own safe path.
6. **Cancellation** — kill the subprocess cleanly when the user stops a scan.

An adapter is **not** responsible for rendering, sorting, selection state, or confirmation
dialogs. Those live in `core/` and the UI, once, for all backends.

## Which adapter runs

Not registration order. `Registry::resolve(ability, preference)` picks, in three passes,
each filtered by whether the adapter is usable _and_ declares the ability being asked for:

1. **The user's choice**, honoured wherever the backend can do the job.
2. **The platform default** — macOS `mole, rip, ncdu, gdu`; Windows
   `gdu, rip, ncdu, mole`; everywhere else `ncdu, rip, gdu, mole`. Capability filtering
   means rip is considered only for undo and never for scanning or new deletion.
3. **Registration order**, so a backend no default names is still reachable.

A preference is a preference, not an override. Choosing Mole on macOS is honoured for
cleanup and app inventory and falls back to ncdu for scanning, because Mole cannot scan — and
the returned `Choice::instead_of` names the backend that was displaced so the fallback can
be stated rather than looking like the setting was ignored.

An id naming no registered backend is skipped rather than treated as an error. See
[ADR 0013](adr/0013-the-backend-is-a-choice.md).

## The trait

Detection, capabilities, scanning, and selected-path deletion share one trait. Every current
backend refuses new selected-path deletion. rip retains exact undo for durable receipts made
before deletion was withdrawn. See
[ADR 0017](adr/0017-rip-deletion-is-not-execution-bound.md). New selected-path deletion is
deferred beyond v0.1 by
[ADR 0018](adr/0018-selected-path-deletion-is-deferred-for-v0-1.md).

```rust
pub trait Adapter: Send + Sync {
    /// Stable machine identifier: "ncdu", "mole", "gdu", "rip".
    fn id(&self) -> &'static str;

    /// Name shown in the backend picker.
    fn display_name(&self) -> &'static str;

    /// Version range this adapter has been tested against.
    fn supported_versions(&self) -> &'static str;

    /// Is the backend installed, and is its version one we understand?
    fn detect(&self) -> Result<Detection, AdapterError>;

    /// What this backend can do. Meaningful only after a successful detect.
    fn capabilities(&self) -> Capabilities;

    /// Walk `root`, streaming entries into `sink` as the backend produces them.
    fn scan(
        &self,
        root: &Path,
        options: &ScanOptions,
        sink: &mut dyn WireSink,
        cancel: &CancelToken,
    ) -> Result<ScanSummary, AdapterError>;

    /// Validate and canonicalise a selected target. Default: Unsupported.
    fn prepare_delete(
        &self,
        scan_root: &Path,
        target: &Path,
        mode: DeleteMode,
    ) -> Result<DeletePlan, AdapterError>;

    /// Revalidate and execute a confirmed plan. Default: Unsupported.
    fn delete(
        &self,
        plan: &DeletePlan,
        cancel: &CancelToken,
    ) -> Result<DeleteReceipt, AdapterError>;

    /// Restore the exact receipt through the same backend. Default: Unsupported.
    fn undo(
        &self,
        receipt: &DeleteReceipt,
        cancel: &CancelToken,
    ) -> Result<(), AdapterError>;
}
```

Synchronous on purpose. A scan is one subprocess and one blocking read; an async runtime
would be a dependency, a colour on every function, and a second scheduler beside the one
Tauri already runs. Callers put `scan` on a worker thread and cancel it with the token.

The parser and `WireSink` live in `crates/adapter` rather than in the ncdu adapter, because
the format is part of this contract and both scanner backends emit it natively. See
[ADR 0008](adr/0008-wire-parser-in-adapter-crate.md).

## Reading a scan

`WireSink` receives entries during the parse. `open_dir` and `close_dir` bracket a
directory's children, so a sink can keep a stack and never needs the tree in advance:

```rust
pub trait WireSink {
    fn header(&mut self, _header: &WireHeader) {}
    fn open_dir(&mut self, item: WireItem);
    fn item(&mut self, item: WireItem);
    fn close_dir(&mut self);
}
```

`TreeSink` is the implementation almost everything wants: it builds a `nirmoka_core::Tree`,
deduplicates hardlinks, and counts what it had to warn about.

```rust
let mut sink = TreeSink::new();
let summary = adapter.scan(root, &ScanOptions::default(), &mut sink, &cancel)?;
let stats = sink.stats();   // read errors, exclusions, deduplicated hardlinks
let tree = sink.finish();   // sizes rolled up
```

Warnings are part of the result, not a log line. A total that omits twelve unreadable
directories is a lie by omission, and `TreeStats` is how the UI can say so.

## Requirements

### Detection must not be a `which` call alone

Finding the binary is not enough. Run its version flag, parse the result, and compare
against this adapter's tested range. An adapter that has never seen the installed version
must report `Detection::UnsupportedVersion` and let the UI explain, rather than guessing
that the output format is unchanged.

### Validate before you spawn

Path validation happens inside the adapter, before the path becomes a subprocess argument.
Minimum bar:

- The path is absolute and canonicalised.
- Symlinks are resolved and re-checked (the resolved target is what gets deleted).
- The path is inside the scanned root.
- The path is not a system-critical location for this platform.

The backend may have its own protections — Mole's `should_protect_path()` is far stricter
than anything Nirmoka should attempt to reimplement. That is a second layer, not a
replacement for this one. Neither layer is allowed to assume the other ran.

### Never reimplement a backend's safety rules

If a backend maintains curated protected-path or cleanup-target lists, call the backend and
let it apply them. Copying those lists into the adapter creates two problems at once: they
go stale the moment upstream updates them, and for a GPL-licensed backend like Mole,
copying its data tables makes Nirmoka a derivative work. See
[NOTICE.md](../NOTICE.md).

### Streaming, not buffering

A scan of a large home directory produces a lot of nodes — 2.2 million on the machine this
was developed on. `scan` streams into a `WireSink` so the UI can paint the first rows within
a few hundred milliseconds. Collecting the whole tree before returning makes the app feel
broken on exactly the disks people most need it for.

The parser pulls from the backend's stdout and hands over each entry as it decodes, so the
export text is never held in memory alongside the tree it becomes.

### Cancellation must actually kill the process

When the user stops a scan, the subprocess must terminate, not be orphaned and left
churning the disk. Every adapter needs a test for this.

`RunningProcess` in `crates/adapter` does the work: it spawns with piped output, watches the
`CancelToken` on its own thread, and kills the child when the token trips. A cancelled scan
returns `AdapterError::Cancelled`, never a truncated success — the export a killed backend
leaves behind would otherwise parse as a small disk.

### Degrade, don't lie

If a backend cannot do something, report `Unsupported`. Do not emulate it. This applies to
the headline abilities too — Mole cannot scan, and the adapter says so rather than returning
the one level of tree it could produce. A tree one level deep presented as a complete one is
a disk that looks empty. Specifically:
if a backend has no dry-run mode, the adapter declares `dry_run: false` and the UI falls
back to an explicit confirmation dialog. An adapter must never fake a preview by guessing
what the backend would delete.

## Backend notes

### Mole (macOS)

- Binary: `mo`
- Scan: **none.** `mo analyze --json` lists one directory's direct children with recursive
  sizes and stops; the analyzer takes no depth or recursion flag, so a tree would cost one
  subprocess per directory. The adapter declares `scan: false` and returns `Unsupported`.
  See [ADR 0012](adr/0012-mole-is-not-a-scanner.md) and `fixtures/mole/1.48.1/`
- Preview: `mo clean --dry-run` only. `mo uninstall --dry-run` prints its plan _after_
  `Proceed with uninstallation? [y/N]`, so there is no preview to read. Human-readable text, not JSON —
  the ability is real, and step 11 parses only plans Mole itself produced
- Cleanup preview: the adapter reads Mole's safely published `clean-list.txt`, preserving its
  category names, paths, rounded size labels, grouped counts, summary, and system-scope warning.
  The UI offers no category selector because Mole 1.48.1 removed that command surface
- Cleanup execution: `mo clean` receives no paths or categories from the preview. Mole performs
  fresh discovery at execution time, so the preview is evidence for confirmation rather than an
  immutable delete list; reviewed and executed candidates may differ. Execution is non-interactive:
  an already-cached sudo session enables system cleanup, otherwise Mole continues at user scope and
  the result is partial. Known timeout, permission, authentication, and removal warnings remain
  backend-produced result warnings. Cancellation kills Mole, and reports `Cancelled` rather than
  `AdapterError::Cancelled`: files removed before the kill stay removed, so a started run is always
  an outcome. A backend that dies part way through reports `Failed` for the same reason. An `Err`
  from cleanup execution means the run never started — a Mole version different from the one that
  produced the preview is refused there, even when both versions are otherwise supported
- Cleanup result: recorded in the shared operation journal as a `cleaned` event, with the reviewed
  evidence and the backend's reported scope, completion, and warnings — and no per-path result,
  because Mole publishes none. A failed journal write reports the run beside the error rather than
  hiding it: the removal already happened and cannot be undone. See
  [ADR 0020](adr/0020-cleanup-runs-are-journalled-without-a-receipt.md)
- Status: `mo status --json`, normalized into the capability-specific system-status contract;
  the adapter rejects malformed output and unsupported Mole versions
- Application inventory: `mo uninstall --list` in non-interactive JSON mode. The backend's
  `uninstall_name` crosses the boundary unchanged so nothing downstream guesses from a label
- Application uninstall: **none.** Every named `mo uninstall`, `--dry-run` included, stops at
  `Proceed with uninstallation? [y/N]` and blocks on stdin; the flag set is `--list`, `--dry-run`,
  `--permanent`, `--whitelist`, `--debug`, with no non-interactive escape. Answering another tool's
  confirmation prompt is not something an adapter may do, so `uninstall_apps` is false and the trait
  gains no uninstall method. The window shows the inventory and the exact name, and the user runs the
  command. See [ADR 0021](adr/0021-application-uninstall-is-not-an-adapter-api.md) and
  `fixtures/mole/1.48.1/uninstall-command-surface.txt`
- Capabilities: dry run, cleanup categories, app inventory, status. Not arbitrary-path
  deletion: neither `mo clean` nor `mo uninstall` accepts a scanned path as such. Not app
  uninstall, which is a prompt rather than a command surface
- Note: macOS only. GPL-3.0, so read its output, never its data tables.

Mole is the reason no single capability is a floor. A backend has to be able to do
_something_, and the contract suite asserts exactly that — not that it can do any
particular thing.

### ncdu (cross-platform)

- Binary: `ncdu`
- Scan: `ncdu --ignore-config -0 -o - <path>` (export mode; the interactive TUI is not
  scriptable). `--ignore-config` matters: without it a user's `~/.config/ncdu/config`
  silently changes what a scan means.
- Preview: none — declare `dry_run: false`
- Capabilities: scan only. Its deletion feature is a keybinding in the interactive browser,
  not a scriptable command an adapter can safely target
- This is the baseline. If a feature cannot be expressed here, it belongs behind a
  capability flag rather than in the core interface.

### gdu (cross-platform)

- Binary: `gdu`
- Supported versions: `>=5.32, <5.33`, recorded from 5.32.0
- Scan: `gdu --config-file <null-device> --no-progress -o - <path>`, producing ncdu
  format 1.2 directly
- Primary Windows scanner and an available alternative elsewhere
- `one_file_system` maps to `--no-cross`
- Cache-tag and glob exclusions are refused: gdu 5.32 has no CACHEDIR.TAG option and its
  ignore patterns are regular expressions, not ncdu globs. Silently translating them would
  change the requested scan
- Selected-path deletion is not exposed: like ncdu, gdu deletes only inside its interactive
  terminal browser — see [ADR 0014](adr/0014-interactive-deletion-is-not-an-adapter-api.md)
- Details and consequences: [ADR 0015](adr/0015-gdu-is-the-windows-scanner.md)

### rip (macOS and Linux)

- Binary: `rip` from the `rm-improved` package
- Supported versions: `>=0.13, <0.14`, tested against the real 0.13.1 release
- Scan: none
- Selected-path deletion: none. rip 0.13.1 canonicalises a path and later moves that pathname,
  so an ancestor replacement can redirect execution after containment validation
- Undo: exact non-interactive `rip --graveyard <operation> --unbury <receipt>` for existing
  durable receipts
- Dry run and permanent deletion: none
- GPL-3.0 and separately installed. Nirmoka invokes the binary and does not bundle or copy it
- Details and consequences: [ADR 0017](adr/0017-rip-deletion-is-not-execution-bound.md)

## Fixtures and the contract suite

`tests/contract` is one suite that every adapter must pass. It is driven by recorded backend
output under `fixtures/<backend>/<version>/`, so it runs on machines with no backend
installed — including Windows CI, where ncdu does not exist at all.

Only backends that declare `scan: true` contribute wire-format fixtures; the suite filters by
that flag rather than by directory name. `fixtures/mole/` is recorded output that Nirmoka
never parses — it is the evidence behind ADR 0012, asserted by
`crates/adapter-mole/tests/analyzer_shape.rs` so an upgrade re-tests the finding.

```bash
./scripts/record-ncdu-fixture.sh          # re-record after a backend upgrade
./scripts/record-gdu-fixture.sh           # re-record after a backend upgrade
cargo test -p nirmoka-contract-tests
```

Recorded output is never hand-written and never edited afterwards, with one exception: the
scan root is rewritten to `/fixtures/<name>` so the recording machine's paths stay out of
the repository. The point of a fixture is to capture the difference between what a backend
documents and what it emits; a made-up one captures nothing.

Fixtures are stored per backend version. A format drift should appear as a new directory
beside the old one, not as a diff that quietly replaces the evidence.

## Adding a backend

1. Read this document and [architecture.md](architecture.md).
2. Create `adapter-<name>/`.
3. Record real output from the backend into `fixtures/<name>/<version>/`.
4. Implement the trait.
5. Register the adapter in the registry — `crates/cli`, the Tauri app, and
   `tests/contract/src/lib.rs` must all build the same one.
6. Run the shared contract suite — no adapter-specific test suite. If your backend needs
   a special case in the shared tests, the trait is probably wrong.
