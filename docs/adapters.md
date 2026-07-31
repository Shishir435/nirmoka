# Adapter Contract

An adapter teaches Nirmoka to drive one disk tool. This document is the contract every
adapter must satisfy.

## Responsibilities

An adapter is responsible for:

1. **Detection** — is this backend installed, and where?
2. **Version gating** — is the installed version one this adapter understands?
3. **Capability reporting** — what can this backend actually do?
4. **Scanning** — run the backend, stream results as ncdu-format JSON.
5. **Deletion** — validate the path, then remove it via the backend's own safe path.
6. **Cancellation** — kill the subprocess cleanly when the user stops a scan.

An adapter is **not** responsible for rendering, sorting, selection state, or confirmation
dialogs. Those live in `core/` and the UI, once, for all backends.

## The trait

Detection, capabilities, and scanning are implemented. `delete` arrives in roadmap step 10,
with its own validation and its own tests; it is deliberately absent rather than stubbed,
so nothing can depend on a signature that has not been designed yet.

```rust
pub trait Adapter: Send + Sync {
    /// Stable machine identifier: "ncdu", "mole", "gdu".
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
}
```

Synchronous on purpose. A scan is one subprocess and one blocking read; an async runtime
would be a dependency, a colour on every function, and a second scheduler beside the one
Tauri already runs. Callers put `scan` on a worker thread and cancel it with the token.

The parser and `WireSink` live in `crates/adapter` rather than in the ncdu adapter, because
the format is part of this contract and two of three backends emit it natively. See
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

If a backend cannot do something, report `Unsupported`. Do not emulate it. Specifically:
if a backend has no dry-run mode, the adapter declares `dry_run: false` and the UI falls
back to an explicit confirmation dialog. An adapter must never fake a preview by guessing
what the backend would delete.

## Backend notes

### Mole (macOS)

- Binary: `mo`
- Scan: `mo analyze --json`
- Preview: `mo clean --dry-run`, or the `MOLE_DRY_RUN=1` environment variable
- Capabilities: everything — trash, dry run, cleanup categories, app uninstall, status
- Note: macOS only. GPL-3.0, so read its output, never its data tables.

### ncdu (cross-platform)

- Binary: `ncdu`
- Scan: `ncdu --ignore-config -0 -o - <path>` (export mode; the interactive TUI is not
  scriptable). `--ignore-config` matters: without it a user's `~/.config/ncdu/config`
  silently changes what a scan means.
- Preview: none — declare `dry_run: false`
- Capabilities: scan and delete only
- This is the baseline. If a feature cannot be expressed here, it belongs behind a
  capability flag rather than in the core interface.

### gdu (cross-platform)

- Binary: `gdu`
- Scan: ncdu-compatible export
- Notable as the realistic Windows path

## Fixtures and the contract suite

`tests/contract` is one suite that every adapter must pass. It is driven by recorded backend
output under `fixtures/<backend>/<version>/`, so it runs on machines with no backend
installed — including Windows CI, where ncdu does not exist at all.

```bash
./scripts/record-ncdu-fixture.sh          # re-record after a backend upgrade
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
