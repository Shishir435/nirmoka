# Adapter Contract

An adapter teaches Nirmoka to drive one disk tool. This document is the contract every
adapter must satisfy.

> **Draft.** The trait below is a design sketch, not shipped code. It will change as the
> first two adapters are written. Treat it as the intended shape.

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

## Trait sketch

```rust
#[async_trait]
pub trait Adapter: Send + Sync {
    /// Stable identifier: "mole", "ncdu", "gdu".
    fn id(&self) -> &'static str;

    /// Human-readable name for the backend picker.
    fn display_name(&self) -> &'static str;

    /// Is the backend present on this machine, and at what version?
    async fn detect(&self) -> Result<Detection, AdapterError>;

    /// What this backend can do. Called after successful detection.
    fn capabilities(&self) -> Capabilities;

    /// Stream a scan of `root`. Emits ncdu-format nodes as they arrive.
    async fn scan(
        &self,
        root: &Path,
        opts: ScanOptions,
        sink: &mut dyn ScanSink,
        cancel: CancellationToken,
    ) -> Result<ScanSummary, AdapterError>;

    /// Remove `path`. Must validate before touching a subprocess argument.
    /// Honours `mode`; returns Unsupported if the backend lacks that mode.
    async fn delete(
        &self,
        path: &Path,
        mode: DeleteMode,
    ) -> Result<DeleteOutcome, AdapterError>;
}

pub enum DeleteMode {
    /// Report what would be removed. Nothing is touched.
    DryRun,
    /// Recoverable removal. Requires Capabilities::trash.
    Trash,
    /// Irreversible removal.
    Permanent,
}
```

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

A scan of a large home directory produces a lot of nodes. `scan` streams into a `ScanSink`
so the UI can paint the first rows within a few hundred milliseconds. Collecting the whole
tree before returning makes the app feel broken on exactly the disks people most need it
for.

### Cancellation must actually kill the process

When the user stops a scan, the subprocess must terminate, not be orphaned and left
churning the disk. Every adapter needs a test for this.

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
- Scan: `ncdu -o -` (export mode; the interactive TUI is not scriptable)
- Preview: none — declare `dry_run: false`
- Capabilities: scan and delete only
- This is the baseline. If a feature cannot be expressed here, it belongs behind a
  capability flag rather than in the core interface.

### gdu (cross-platform)

- Binary: `gdu`
- Scan: ncdu-compatible export
- Notable as the realistic Windows path

## Adding a backend

1. Read this document and [architecture.md](architecture.md).
2. Create `adapter-<name>/`.
3. Record real output from the backend into `fixtures/<name>/<version>/`.
4. Implement the trait.
5. Register the adapter in the registry.
6. Run the shared contract suite — no adapter-specific test suite. If your backend needs
   a special case in the shared tests, the trait is probably wrong.
