//! The Mole adapter — macOS cleanup abilities, and deliberately no scanning.
//!
//! # Why this backend does not scan
//!
//! The roadmap planned this adapter as a second scanner: run `mo analyze
//! --json` and translate its output into the wire format. Mole's analyzer does
//! not produce a tree. It lists the *direct children* of one directory with
//! recursive sizes, and there is no depth, recursion, or full-export flag —
//! `analyze-go` accepts exactly one option, `-json`.
//!
//! Reconstructing a tree from it would mean one subprocess per directory. A
//! home directory holds tens of thousands of them, against a backend that walks
//! the whole subtree on every call, so the same bytes would be read once per
//! level of nesting.
//!
//! So this adapter reports `scan: false` and returns `Unsupported`. That is the
//! contract's own rule — degrade, don't lie — applied to the awkward case where
//! the missing ability is the headline one. ncdu remains the scanner. Mole is
//! here for what it is genuinely better at than anything else available:
//! removal that applies its own curated protections.
//!
//! See [ADR 0012](../../../docs/adr/0012-mole-is-not-a-scanner.md), and
//! `fixtures/mole/1.48.1/` for the recorded evidence.
//!
//! # Licensing
//!
//! Mole is GPL-3.0. This adapter reads its *output* and drives its *CLI*. It
//! does not transcribe its protected-path lists, its cleanup targets, or any
//! other data table — doing so would make Nirmoka a derivative work and
//! silently relicense it. See `NOTICE.md`.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use directories::BaseDirs;
use nirmoka_adapter::process::{self, find_in_path, RunningProcess};
use nirmoka_adapter::{
    Adapter, AdapterError, CancelToken, Capabilities, CleanupCategory, CleanupCompletion,
    CleanupExecution, CleanupItem, CleanupPreview, CleanupSystemScope, Detection,
    InstalledApplication, ScanOptions, ScanSummary, SystemStatus, UninstallApp,
    UninstallCompletion, UninstallExecution, UninstallItem, UninstallItemScope, UninstallPreview,
    WireSink,
};

const BINARY: &str = "mo";

/// Tested against 1.48.1. The 1.x series is the only one that has existed.
///
/// The gate matters more here than for a scanner, not less: this backend's
/// abilities are all destructive, and a changed flag on a delete path is the
/// worst possible place to discover a version drift.
const SUPPORTED: &str = ">=1.48, <2.0";

#[derive(Debug, Default, Clone, Copy)]
pub struct MoleAdapter;

impl MoleAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Adapter for MoleAdapter {
    fn id(&self) -> &'static str {
        "mole"
    }

    fn display_name(&self) -> &'static str {
        "Mole"
    }

    fn supported_versions(&self) -> &'static str {
        SUPPORTED
    }

    /// Mole is macOS-only, and the check is a compile-time one.
    ///
    /// Upstream's `install.sh` refuses other platforms and every `cmd/analyze/*.go`
    /// file carries `//go:build darwin`, so a `mo` on Linux is a different
    /// program that happens to share a name. Reporting it as "found" would be a
    /// worse answer than reporting nothing.
    ///
    /// The gate lives here rather than in a `#[cfg]` on the crate so that the
    /// workspace builds identically on every platform — invariant 3 keeps
    /// platform conditionals out of `core`, and putting them in an adapter is
    /// exactly where they are supposed to go.
    fn detect(&self) -> Result<Detection, AdapterError> {
        if !cfg!(target_os = "macos") {
            return Ok(Detection::NotInstalled);
        }

        let resolved = find_in_path(BINARY);
        let program = resolved.clone().unwrap_or_else(|| PathBuf::from(BINARY));

        let output = match process::command(&program).arg("--version").output() {
            Ok(output) => output,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Detection::NotInstalled)
            }
            Err(source) => {
                return Err(AdapterError::Spawn {
                    binary: BINARY,
                    source,
                })
            }
        };

        if !output.status.success() {
            return Err(AdapterError::BackendFailed {
                binary: BINARY,
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let version = parse_version(&stdout).ok_or_else(|| AdapterError::UnreadableVersion {
            binary: BINARY,
            output: stdout.trim().to_string(),
        })?;

        if is_supported(&version) {
            Ok(Detection::Found {
                path: program,
                version,
            })
        } else {
            Ok(Detection::UnsupportedVersion {
                path: program,
                version,
                supported: SUPPORTED.to_string(),
            })
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            // The headline exception. See the module docs: `mo analyze --json`
            // lists one directory's children, not a tree.
            scan: false,
            // Mole can clean curated categories and uninstall applications,
            // but it has no command that removes an arbitrary scanned path.
            delete: false,
            // `mo uninstall` documents Trash routing, but this flag describes
            // selected-path deletion, which Mole does not offer. Recoverability
            // for app uninstall belongs to that operation's eventual API.
            trash: false,
            undo: false,
            // `mo clean --dry-run`, `mo uninstall --dry-run`. The preview is
            // Mole's own output rather than rows Nirmoka can render, which is a
            // presentation problem for step 10 — not a reason to claim the
            // ability is missing.
            dry_run: true,
            cleanup_categories: true,
            // `mo uninstall --list` is a machine-readable one-shot, and it
            // publishes the exact name the uninstall command accepts.
            app_inventory: true,
            // `mo uninstall <name>` reads a line from stdin before it prints
            // anything, which is why ADR 0021 read the surface as closed. It is
            // not: the plan is one line of input away, and under `--dry-run`
            // that line guards nothing — every destructive call past it is
            // individually gated on `MOLE_DRY_RUN`. So an exact preview costs
            // nothing and risks nothing, and the removal it describes needs only
            // the user's own approval relayed back. Mole even authenticates the
            // user itself when a cask or a system app needs it, through its own
            // native dialog. See ADR 0027, which supersedes ADR 0021, and
            // `fixtures/mole/1.48.1/uninstall-plan.txt`.
            uninstall_apps: true,
            // `mo status --json`.
            system_status: true,
        }
    }

    fn system_status(&self, cancel: &CancelToken) -> Result<SystemStatus, AdapterError> {
        let (binary, _) = supported_binary(self.detect()?)?;
        status_from(&binary, cancel)
    }

    fn installed_applications(
        &self,
        cancel: &CancelToken,
    ) -> Result<Vec<InstalledApplication>, AdapterError> {
        let (binary, _) = supported_binary(self.detect()?)?;
        applications_from(&binary, cancel)
    }

    fn cleanup_preview(&self, cancel: &CancelToken) -> Result<CleanupPreview, AdapterError> {
        let (binary, version) = supported_binary(self.detect()?)?;
        let home = BaseDirs::new()
            .ok_or_else(|| AdapterError::OperationFailed {
                backend: "mole",
                operation: "cleanup preview",
                reason: "the user home directory is unavailable".to_string(),
            })?
            .home_dir()
            .to_path_buf();
        cleanup_preview_from(
            &binary,
            &home.join(".config").join("mole").join("clean-list.txt"),
            &version,
            cancel,
        )
    }

    fn execute_cleanup(
        &self,
        reviewed_version: &str,
        cancel: &CancelToken,
    ) -> Result<CleanupExecution, AdapterError> {
        let binary = execution_binary(self.detect()?, reviewed_version)?;
        execute_cleanup_from(&binary, cancel)
    }

    fn uninstall_preview(
        &self,
        names: &[String],
        cancel: &CancelToken,
    ) -> Result<UninstallPreview, AdapterError> {
        let (binary, version) = supported_binary(self.detect()?)?;
        uninstall_preview_from(&binary, names, &version, cancel)
    }

    fn execute_uninstall(
        &self,
        names: &[String],
        reviewed_version: &str,
        cancel: &CancelToken,
    ) -> Result<UninstallExecution, AdapterError> {
        let binary = execution_binary(self.detect()?, reviewed_version)?;
        execute_uninstall_from(&binary, names, cancel)
    }

    /// Always [`AdapterError::Unsupported`].
    ///
    /// Not a stub and not a TODO. Faking a scan here would mean either a tree
    /// one level deep presented as a complete one, or a subprocess per
    /// directory — a wrong answer, or an unusable one.
    fn scan(
        &self,
        _root: &Path,
        _options: &ScanOptions,
        _sink: &mut dyn WireSink,
        _cancel: &CancelToken,
    ) -> Result<ScanSummary, AdapterError> {
        Err(AdapterError::Unsupported {
            backend: "mole",
            operation: "scan",
        })
    }
}

fn supported_binary(detection: Detection) -> Result<(PathBuf, String), AdapterError> {
    match detection {
        Detection::Found { path, version } => Ok((path, version)),
        Detection::UnsupportedVersion { version, .. } => Err(AdapterError::UnsupportedVersion {
            binary: BINARY,
            version,
            supported: SUPPORTED,
        }),
        Detection::NotInstalled => Err(AdapterError::NotInstalled { binary: BINARY }),
    }
}

fn execution_binary(detection: Detection, reviewed_version: &str) -> Result<PathBuf, AdapterError> {
    let (binary, current_version) = supported_binary(detection)?;
    if current_version != reviewed_version {
        return Err(AdapterError::BackendVersionChanged {
            binary: BINARY,
            reviewed: reviewed_version.to_string(),
            current: current_version,
        });
    }
    Ok(binary)
}

fn applications_from(
    binary: &Path,
    cancel: &CancelToken,
) -> Result<Vec<InstalledApplication>, AdapterError> {
    json_from_command(
        binary,
        &["uninstall", "--list"],
        "application inventory",
        cancel,
    )
}

fn status_from(binary: &Path, cancel: &CancelToken) -> Result<SystemStatus, AdapterError> {
    json_from_command(binary, &["status", "--json"], "system status", cancel)
}

fn cleanup_preview_from(
    binary: &Path,
    preview_path: &Path,
    backend_version: &str,
    cancel: &CancelToken,
) -> Result<CleanupPreview, AdapterError> {
    use std::io::Read;

    let operation = "cleanup preview";
    let mut command = process::command(binary);
    command.args(["clean", "--dry-run"]);

    let mut process =
        RunningProcess::spawn(&mut command, cancel).map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;
    let mut stdout = String::new();
    let read_result = process
        .take_stdout()
        .ok_or_else(|| AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "backend stdout was unavailable".to_string(),
        })
        .and_then(|mut reader| {
            reader
                .read_to_string(&mut stdout)
                .map_err(|source| AdapterError::OperationFailed {
                    backend: "mole",
                    operation,
                    reason: source.to_string(),
                })
        });
    let outcome = process.finish().map_err(|source| AdapterError::Spawn {
        binary: BINARY,
        source,
    })?;
    if outcome.cancelled {
        return Err(AdapterError::Cancelled {
            backend: "mole",
            operation,
        });
    }
    if !outcome.status.success() {
        return Err(AdapterError::BackendFailed {
            binary: BINARY,
            status: outcome.status.code().unwrap_or(-1),
            stderr: outcome.stderr,
        });
    }
    read_result.map(|_| ())?;
    if stdout.contains("Cleanup preview file could not be written safely") {
        return Err(AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "backend could not publish its cleanup preview safely".to_string(),
        });
    }

    let scope = if stdout.contains("Admin access available, system preview included") {
        CleanupSystemScope::Included
    } else if stdout.contains("System caches need sudo") {
        CleanupSystemScope::UserOnly
    } else {
        CleanupSystemScope::Unknown
    };
    let metadata = std::fs::symlink_metadata(preview_path).map_err(|source| {
        AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: format!("backend preview is unavailable: {source}"),
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "backend preview is not a regular file".to_string(),
        });
    }
    let contents =
        std::fs::read_to_string(preview_path).map_err(|source| AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: format!("backend preview could not be read: {source}"),
        })?;
    parse_cleanup_preview(&contents, backend_version, scope)
}

/// Run Mole's normal cleanup command without forwarding paths or categories
/// from the preview. Mole 1.48.1 removed category-selection flags and performs
/// a new discovery here, so the reviewed preview and execution result may
/// differ by design.
fn execute_cleanup_from(
    binary: &Path,
    cancel: &CancelToken,
) -> Result<CleanupExecution, AdapterError> {
    use std::io::Read;

    let operation = "cleanup execution";
    let mut command = process::command(binary);
    command.arg("clean");

    let mut process =
        RunningProcess::spawn(&mut command, cancel).map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;
    let mut stdout = Vec::new();
    let read_result = process
        .take_stdout()
        .ok_or_else(|| AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "backend stdout was unavailable".to_string(),
        })
        .and_then(|mut reader| {
            reader
                .read_to_end(&mut stdout)
                .map(|_| ())
                .map_err(|source| AdapterError::OperationFailed {
                    backend: "mole",
                    operation,
                    reason: source.to_string(),
                })
        });
    // Past this point Mole was running, so it may already have removed files.
    // Every way this can end is an outcome to report; only the errors raised
    // before the spawn above mean nothing happened.
    Ok(execution_of(&stdout, process.finish(), read_result))
}

/// Turn everything one finished run produced into a single outcome.
///
/// Deliberately infallible. Each argument can carry a failure, and none of them
/// can show that Mole removed nothing — so a `Result` here would let a real
/// removal be reported as a run that never happened.
fn execution_of(
    stdout: &[u8],
    finished: std::io::Result<nirmoka_adapter::process::Outcome>,
    read_result: Result<(), AdapterError>,
) -> CleanupExecution {
    let mut execution = parse_cleanup_execution(&String::from_utf8_lossy(stdout));

    let outcome = match finished {
        Ok(outcome) => outcome,
        // Waiting on the child failed, so how far Mole got is unknowable.
        Err(source) => {
            execution.completion = CleanupCompletion::Failed;
            execution.warnings.push(format!(
                "Mole ran, and its exit status could not be read: {source}"
            ));
            return execution;
        }
    };
    if outcome.cancelled {
        execution.completion = CleanupCompletion::Cancelled;
        execution
            .warnings
            .push("Cleanup was stopped part way through. Anything Mole had already removed stays removed.".to_string());
        return execution;
    }
    if !outcome.status.success() {
        execution.completion = CleanupCompletion::Failed;
        execution.warnings.push(format!(
            "Mole exited with status {} part way through: {}",
            outcome.status.code().unwrap_or(-1),
            first_line(&outcome.stderr)
        ));
        return execution;
    }
    if let Err(error) = read_result {
        execution.completion = CleanupCompletion::Failed;
        execution.warnings.push(format!(
            "Mole ran, and its output could not be read: {error}"
        ));
        return execution;
    }
    execution
}

/// The first nonempty line of a backend's stderr, for a one-line warning.
fn first_line(stderr: &str) -> String {
    stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("no error output")
        .to_string()
}

fn parse_cleanup_execution(stdout: &str) -> CleanupExecution {
    let output = strip_ansi(stdout);
    let lines: Vec<_> = output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    let system_scope = if lines
        .iter()
        .any(|line| line.contains("System-level cleanup enabled, sudo session active"))
    {
        CleanupSystemScope::Included
    } else if lines
        .iter()
        .any(|line| line.contains("System-level cleanup skipped, requires sudo"))
    {
        CleanupSystemScope::UserOnly
    } else {
        CleanupSystemScope::Unknown
    };

    let mut warnings = Vec::new();
    match system_scope {
        CleanupSystemScope::Included => {}
        CleanupSystemScope::UserOnly => warnings.push(
            "System-level cleanup was skipped because cached administrator authorization was unavailable."
                .to_string(),
        ),
        CleanupSystemScope::Unknown => warnings.push(
            "Mole did not report whether system-level cleanup was authorized.".to_string(),
        ),
    }

    for line in lines {
        let lower = line.to_ascii_lowercase();
        if ["failed", "timed out", "could not", "permission denied"]
            .iter()
            .any(|marker| lower.contains(marker))
            && !warnings.iter().any(|warning| warning == line)
        {
            warnings.push(line.to_string());
        }
    }

    CleanupExecution {
        system_scope,
        completion: if warnings.is_empty() {
            CleanupCompletion::Finished
        } else {
            CleanupCompletion::Partial
        },
        warnings,
    }
}

fn strip_ansi(input: &str) -> String {
    let mut output = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'[') {
            index += 2;
            while index < bytes.len() {
                let byte = bytes[index];
                index += 1;
                if (0x40..=0x7e).contains(&byte) {
                    break;
                }
            }
        } else {
            output.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn parse_cleanup_preview(
    contents: &str,
    backend_version: &str,
    system_scope: CleanupSystemScope,
) -> Result<CleanupPreview, AdapterError> {
    let malformed = |reason: String| AdapterError::MalformedBackendOutput {
        binary: BINARY,
        operation: "cleanup preview",
        reason,
    };
    let mut lines = contents.lines();
    let header = lines
        .next()
        .ok_or_else(|| malformed("preview is empty".to_string()))?;
    let generated_at = header
        .strip_prefix("# Mole Cleanup Preview - ")
        .filter(|value| !value.is_empty())
        .ok_or_else(|| malformed("preview header is missing or changed".to_string()))?
        .to_string();
    let mut categories: Vec<CleanupCategory> = Vec::new();
    let mut current_category: Option<usize> = None;
    let mut declared_items = None;
    let mut declared_categories = None;
    let mut potential_cleanup = None;
    let mut summary_fields = 0_u8;
    let mut pending_row: Option<String> = None;

    for line in lines {
        if let Some(mut row) = pending_row.take() {
            row.push('\n');
            row.push_str(line);
            match parse_cleanup_row(&row).map_err(malformed)? {
                Some(item) => {
                    let category = current_category.ok_or_else(|| {
                        malformed("cleanup path appears before a category".to_string())
                    })?;
                    categories[category].items.push(item);
                }
                None => pending_row = Some(row),
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }
        if let Some(value) = line.strip_prefix("# Potential cleanup: ") {
            if summary_fields != 0 || value.is_empty() {
                return Err(malformed(
                    "cleanup summary is missing, empty, or out of order".to_string(),
                ));
            }
            potential_cleanup = Some(value.to_string());
            summary_fields = 1;
            continue;
        }
        if let Some(value) = line.strip_prefix("# Items: ") {
            if summary_fields != 1 {
                return Err(malformed(
                    "cleanup summary is missing or out of order".to_string(),
                ));
            }
            declared_items = Some(
                value
                    .parse::<u64>()
                    .map_err(|_| malformed(format!("invalid item count: {value}")))?,
            );
            summary_fields = 2;
            continue;
        }
        if let Some(value) = line.strip_prefix("# Categories: ") {
            if summary_fields != 2 {
                return Err(malformed(
                    "cleanup summary is missing or out of order".to_string(),
                ));
            }
            declared_categories = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| malformed(format!("invalid category count: {value}")))?,
            );
            summary_fields = 3;
            continue;
        }
        if summary_fields != 0 {
            return Err(malformed(
                "cleanup summary must be the terminal preview trailer".to_string(),
            ));
        }
        if let Some(name) = line
            .strip_prefix("=== ")
            .and_then(|value| value.strip_suffix(" ==="))
        {
            if name.is_empty() {
                return Err(malformed("cleanup category has no name".to_string()));
            }
            let index = categories
                .iter()
                .position(|category| category.name == name)
                .unwrap_or_else(|| {
                    categories.push(CleanupCategory {
                        name: name.to_string(),
                        items: Vec::new(),
                    });
                    categories.len() - 1
                });
            current_category = Some(index);
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        match parse_cleanup_row(line).map_err(malformed)? {
            Some(item) => {
                let category = current_category.ok_or_else(|| {
                    malformed("cleanup path appears before a category".to_string())
                })?;
                categories[category].items.push(item);
            }
            None => pending_row = Some(line.to_string()),
        }
    }

    if let Some(row) = pending_row {
        return Err(malformed(format!(
            "cleanup row has no terminal size marker: {row}"
        )));
    }

    let total_items = categories
        .iter()
        .flat_map(|category| &category.items)
        .try_fold(0_u64, |total, item| total.checked_add(item.item_count))
        .ok_or_else(|| malformed("cleanup item count overflowed".to_string()))?;
    if summary_fields != 3 {
        return Err(malformed(
            "preview has no complete summary trailer".to_string(),
        ));
    }
    let declared_items = match declared_items {
        Some(declared) => declared,
        None => return Err(malformed("preview has no item-count summary".to_string())),
    };
    let declared_categories = match declared_categories {
        Some(declared) => declared,
        None => {
            return Err(malformed(
                "preview has no category-count summary".to_string(),
            ));
        }
    };
    if declared_items != total_items {
        return Err(malformed(
            "summary item count does not match its rows".to_string(),
        ));
    }
    if declared_categories != categories.len() {
        return Err(malformed(
            "summary category count does not match its sections".to_string(),
        ));
    }
    let warnings = match system_scope {
        CleanupSystemScope::Included => Vec::new(),
        CleanupSystemScope::UserOnly => vec![
            "System-level candidates are not included because administrator access was unavailable."
                .to_string(),
        ],
        CleanupSystemScope::Unknown => {
            vec!["Mole did not report whether system-level candidates were included.".to_string()]
        }
    };

    Ok(CleanupPreview {
        backend_version: backend_version.to_string(),
        generated_at,
        categories,
        potential_cleanup,
        total_items,
        system_scope,
        warnings,
    })
}

/// Mole's preview is line-oriented, but macOS paths may contain newlines. A
/// row is complete only when its final physical line carries Mole's detail
/// marker; callers retain incomplete text and append the next line verbatim.
fn parse_cleanup_row(line: &str) -> Result<Option<CleanupItem>, String> {
    let Some((path, detail)) = line.rsplit_once("  # ") else {
        return Ok(None);
    };
    if path.is_empty() {
        return Err("cleanup row has an empty path".to_string());
    }
    let (size, item_count) = parse_cleanup_item_detail(detail)?;
    Ok(Some(CleanupItem {
        path: PathBuf::from(path),
        reported_size: size,
        item_count,
    }))
}

fn parse_cleanup_item_detail(detail: &str) -> Result<(Option<String>, u64), String> {
    let (size, item_count) = match detail.rsplit_once(", ") {
        Some((size, count)) if count.ends_with(" items") => {
            let count = count
                .trim_end_matches(" items")
                .parse::<u64>()
                .map_err(|_| format!("invalid cleanup item count: {count}"))?;
            if count < 2 {
                return Err("grouped cleanup item count must be at least two".to_string());
            }
            (size, count)
        }
        _ => (detail, 1),
    };
    if size == "size unknown" {
        return Ok((None, item_count));
    }
    let unit_start = size
        .find(|character: char| character.is_ascii_alphabetic())
        .ok_or_else(|| format!("cleanup size has no unit: {size}"))?;
    let (number, unit) = size.split_at(unit_start);
    if !matches!(unit, "B" | "KB" | "MB" | "GB" | "TB" | "PB")
        || number.is_empty()
        || number.matches('.').count() > 1
        || !number
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
    {
        return Err(format!("invalid cleanup size: {size}"));
    }
    Ok((Some(size.to_string()), item_count))
}

/// The answer written to Mole's confirmation prompt, and nothing more.
///
/// `mo uninstall` asks twice. The first is `read -r confirm`, a whole line, and
/// this is that line. The second is a single-key read *after* the plan has
/// printed, and it treats end of input as confirmation — which this delivers by
/// closing the pipe, because there is nothing left to write.
///
/// Both are reached only once the user has approved the exact plan Mole itself
/// produced. That is the whole basis on which this is allowed to exist; see
/// ADR 0027.
const CONFIRMATION: &[u8] = b"y\n";

/// Check every requested identifier against the backend's live inventory.
///
/// This is the adapter boundary doing its job: an identifier becomes a
/// subprocess argument only if the backend itself just published it. That makes
/// the class of "what if a name is really a flag" questions unaskable — Mole
/// cannot list an application whose `uninstall_name` is `--permanent` — rather
/// than answered with a denylist that a future release could outgrow.
///
/// It also fails closed on the case that matters most: an application that was
/// uninstalled, renamed, or updated between review and execution is no longer in
/// the inventory, so the run is refused instead of matching something else.
fn validated_names(
    binary: &Path,
    requested: &[String],
    operation: &'static str,
    cancel: &CancelToken,
) -> Result<Vec<String>, AdapterError> {
    if requested.is_empty() {
        return Err(AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "no application was named".to_string(),
        });
    }

    let inventory = applications_from(binary, cancel)?;
    let mut names = Vec::with_capacity(requested.len());
    for name in requested {
        if !inventory
            .iter()
            .any(|application| &application.uninstall_name == name)
        {
            return Err(AdapterError::OperationFailed {
                backend: "mole",
                operation,
                reason: format!(
                    "{name:?} is not an application Mole currently lists; \
                     reload the inventory and select it again"
                ),
            });
        }
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    Ok(names)
}

/// Run Mole's own dry run and read back the plan it prints.
///
/// `--dry-run` is set during flag parsing, before anything is discovered, and
/// every removal below it is separately gated on it. So the confirmation this
/// writes guards nothing here: the run cannot modify a file whatever it answers.
/// `--permanent` is never passed, in this function or the next.
fn uninstall_preview_from(
    binary: &Path,
    requested: &[String],
    backend_version: &str,
    cancel: &CancelToken,
) -> Result<UninstallPreview, AdapterError> {
    let operation = "uninstall preview";
    let names = validated_names(binary, requested, operation, cancel)?;
    let mut command = process::command(binary);
    command.args(["uninstall", "--dry-run"]).args(&names);

    let (stdout, outcome, read_result) = run_uninstall(&mut command, operation, cancel)?;
    if outcome.cancelled {
        return Err(AdapterError::Cancelled {
            backend: "mole",
            operation,
        });
    }
    if !outcome.status.success() {
        return Err(AdapterError::BackendFailed {
            binary: BINARY,
            status: outcome.status.code().unwrap_or(-1),
            stderr: outcome.stderr,
        });
    }
    read_result?;

    parse_uninstall_preview(
        &String::from_utf8_lossy(&stdout),
        &names,
        backend_version,
        PlanMode::DryRun,
    )
}

/// Remove the named applications, relaying the approval the shell already holds.
///
/// Nothing from the preview is forwarded. Mole rediscovers every path here and
/// applies its own protections while doing so, exactly as it does under
/// `--dry-run` — which is what makes the reviewed plan an accurate description of
/// this run rather than a list this function had to be trusted to reproduce.
fn execute_uninstall_from(
    binary: &Path,
    requested: &[String],
    cancel: &CancelToken,
) -> Result<UninstallExecution, AdapterError> {
    let operation = "uninstall execution";
    // Re-validated here rather than trusted from the preview. The confirmation
    // names identifiers, and this is the last moment before they become
    // subprocess arguments — an app uninstalled, renamed, or updated since the
    // review is no longer listed, and the run is refused instead of matching
    // something else.
    let names = validated_names(binary, requested, operation, cancel)?;
    let mut command = process::command(binary);
    command.arg("uninstall").args(&names);

    let (stdout, outcome, read_result) = run_uninstall(&mut command, operation, cancel)?;
    // Past this point Mole was running, so it may already have moved files.
    // Every way this can end is an outcome to report — same rule as cleanup.
    Ok(uninstall_execution_of(&stdout, outcome, read_result))
}

/// Spawn one `mo uninstall`, answer its prompt, and drain its output.
///
/// Returns `Err` only for failures that mean the backend never ran.
type UninstallRun = (
    Vec<u8>,
    nirmoka_adapter::process::Outcome,
    Result<(), AdapterError>,
);

fn run_uninstall(
    command: &mut std::process::Command,
    operation: &'static str,
    cancel: &CancelToken,
) -> Result<UninstallRun, AdapterError> {
    use std::io::Read;

    let mut process =
        RunningProcess::spawn_with_input(command, CONFIRMATION, cancel).map_err(|source| {
            AdapterError::Spawn {
                binary: BINARY,
                source,
            }
        })?;
    let mut stdout = Vec::new();
    let read_result = process
        .take_stdout()
        .ok_or_else(|| AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "backend stdout was unavailable".to_string(),
        })
        .and_then(|mut reader| {
            reader
                .read_to_end(&mut stdout)
                .map(|_| ())
                .map_err(|source| AdapterError::OperationFailed {
                    backend: "mole",
                    operation,
                    reason: source.to_string(),
                })
        });
    let outcome = process.finish().map_err(|source| AdapterError::Spawn {
        binary: BINARY,
        source,
    })?;
    Ok((stdout, outcome, read_result))
}

/// Turn one finished uninstall run into a single outcome.
///
/// Infallible for the same reason [`execution_of`] is: none of these inputs can
/// show that Mole moved nothing, so a `Result` would let a real removal be
/// reported as a run that never happened.
fn uninstall_execution_of(
    stdout: &[u8],
    outcome: nirmoka_adapter::process::Outcome,
    read_result: Result<(), AdapterError>,
) -> UninstallExecution {
    let transcript = strip_ansi(&String::from_utf8_lossy(stdout));
    let mut execution = parse_uninstall_execution(&transcript);

    if outcome.cancelled {
        execution.completion = UninstallCompletion::Cancelled;
        execution.warnings.push(
            "The uninstall was stopped part way through. Anything Mole had already moved to the \
             Trash stays there."
                .to_string(),
        );
        return execution;
    }
    if !outcome.status.success() {
        execution.completion = UninstallCompletion::Failed;
        execution.warnings.push(format!(
            "Mole exited with status {} part way through: {}",
            outcome.status.code().unwrap_or(-1),
            first_line(&outcome.stderr)
        ));
        return execution;
    }
    if let Err(error) = read_result {
        execution.completion = UninstallCompletion::Failed;
        execution.warnings.push(format!(
            "Mole ran, and its output could not be read: {error}"
        ));
    }
    execution
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanMode {
    DryRun,
}

impl PlanMode {
    /// The title Mole closes with. Required, and checked: it is the one line that
    /// proves the run reached its own summary rather than dying in the middle of
    /// printing a plan that would then look complete because it parsed.
    fn title(self) -> &'static str {
        match self {
            Self::DryRun => "Uninstall dry run complete",
        }
    }
}

/// Read Mole's plan back out of its own output.
///
/// Every field here is for *rendering*. The preview also keeps the transcript,
/// and execution forwards none of this, so the worst a parser bug can do is
/// display a plan badly — not delete the wrong thing. Even so this refuses
/// rather than guesses: a structure that no longer matches means Mole changed,
/// and a narrowed plan shown as a complete one is the failure worth preventing.
///
/// The grammar, from `lib/uninstall/batch.sh` in Mole 1.48.1:
///
/// ```text
/// ◎ Matched 1 app(s):
/// 1. LocalSend  83.2MB  |  Last: 9m ago
///
/// Proceed with uninstallation? [y/N]
/// Files to be removed:
/// ◎ Homebrew apps will be fully cleaned, --zap removes configs and data
///
/// ◎ LocalSend [Brew] , 83.4MB
///   ✓ /Applications/LocalSend.app , 83.2MB
///   ✓ ~/Library/Containers/org.localsend.localsendApp , 225KB
///   ◎ System: /Library/LaunchDaemons/com.example.helper.plist
///   ◎ Review only: /Library/Preferences/com.example.plist
///
/// ➤ Remove 1 app, 83.4MB  Enter confirm, ESC cancel:
/// Uninstall dry run complete
/// Would remove 1 app, would free 83.4MB: LocalSend
/// ☞ Local Network permissions on macOS 15+ can outlive app removal: LocalSend
/// ↳ Mole does not reset …
/// ```
fn parse_uninstall_preview(
    stdout: &str,
    requested: &[String],
    backend_version: &str,
    mode: PlanMode,
) -> Result<UninstallPreview, AdapterError> {
    let transcript = strip_ansi(stdout);
    let malformed = |reason: String| AdapterError::MalformedBackendOutput {
        binary: BINARY,
        operation: "uninstall preview",
        reason,
    };

    let lines: Vec<&str> = transcript.lines().collect();
    let matched = parse_matched_apps(&lines).map_err(malformed)?;
    if matched.is_empty() {
        return Err(malformed(
            "Mole matched no application, and did not say so in a form this understands"
                .to_string(),
        ));
    }

    let plan_start = lines
        .iter()
        .position(|line| line.trim() == "Files to be removed:")
        .ok_or_else(|| malformed("Mole printed no \"Files to be removed:\" section".to_string()))?;
    if !lines.iter().any(|line| line.trim() == mode.title()) {
        return Err(malformed(format!(
            "Mole did not print {:?}, so its plan may be incomplete",
            mode.title()
        )));
    }

    let mut apps: Vec<UninstallApp> = Vec::new();
    let mut warnings = Vec::new();
    let mut notes = Vec::new();
    let mut reported_total = None;

    for line in &lines[plan_start + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('=') {
            continue;
        }

        // Indented lines belong to the app most recently opened. Checked on the
        // raw line, because the indent is the only thing separating a path from
        // an app header — both can begin with the same marker.
        let indented = line.starts_with("  ") || line.starts_with('\t');
        if indented {
            if let Some(item) = parse_plan_item(trimmed) {
                let app = apps.last_mut().ok_or_else(|| {
                    malformed(format!(
                        "Mole listed {:?} before naming an app",
                        item.display_path
                    ))
                })?;
                app.items.push(item);
                continue;
            }
        }

        if let Some(rest) = trimmed.strip_prefix("➤ ") {
            reported_total = parse_removal_note_total(rest);
            continue;
        }
        if let Some(note) = trimmed
            .strip_prefix("☞ ")
            .or_else(|| trimmed.strip_prefix("↳ "))
        {
            notes.push(note.trim().to_string());
            continue;
        }
        if let Some(freed) = parse_summary_freed(trimmed) {
            reported_total = reported_total.or(freed);
            continue;
        }
        if trimmed.starts_with("Failed: ") {
            warnings.push(trimmed.to_string());
            continue;
        }

        if let Some(header) = trimmed.strip_prefix("◎ ") {
            // An app header and Mole's own notices share this marker, so the app
            // list decides which this is. Matching on the notice text instead
            // would make every new notice look like an application.
            if let Some(app) = parse_app_header(header, &matched) {
                apps.push(app);
                continue;
            }
            warnings.push(header.trim().to_string());
            continue;
        }
    }

    if apps.is_empty() {
        return Err(malformed(
            "Mole matched applications and then listed none of them".to_string(),
        ));
    }
    if apps.len() != matched.len() {
        return Err(malformed(format!(
            "Mole matched {} application(s) and detailed {}",
            matched.len(),
            apps.len()
        )));
    }

    Ok(UninstallPreview {
        backend_version: backend_version.to_string(),
        requested: requested.to_vec(),
        apps,
        reported_total,
        warnings,
        notes,
        transcript,
    })
}

/// Display names from the `◎ Matched N app(s):` block, checked against the count
/// Mole declared.
fn parse_matched_apps(lines: &[&str]) -> Result<Vec<String>, String> {
    let header = lines
        .iter()
        .position(|line| line.trim().starts_with("◎ Matched "))
        .ok_or_else(|| "Mole printed no match header".to_string())?;
    let declared = lines[header]
        .trim()
        .trim_start_matches("◎ Matched ")
        .split_whitespace()
        .next()
        .and_then(|count| count.parse::<usize>().ok())
        .ok_or_else(|| format!("unreadable match count: {:?}", lines[header].trim()))?;

    let mut matched = Vec::new();
    for line in &lines[header + 1..] {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if matched.is_empty() {
                continue;
            }
            break;
        }
        // `printf "%d. %s  %s  |  Last: %s\n"` — the name is everything between
        // the ordinal and the size, and it can contain spaces.
        let Some((_, rest)) = trimmed.split_once(". ") else {
            break;
        };
        let name = rest
            .split_once("  |  Last: ")
            .map(|(head, _)| head)
            .unwrap_or(rest);
        let name = name.rsplit_once("  ").map(|(head, _)| head).unwrap_or(name);
        if name.is_empty() {
            return Err("Mole listed a matched app with no name".to_string());
        }
        matched.push(name.trim().to_string());
    }

    if matched.len() != declared {
        return Err(format!(
            "Mole declared {declared} matched app(s) and listed {}",
            matched.len()
        ));
    }
    Ok(matched)
}

/// `LocalSend [Brew] , 83.4MB` — but only when the name is one Mole matched.
fn parse_app_header(header: &str, matched: &[String]) -> Option<UninstallApp> {
    let (name, reported_size) = match header.rsplit_once(" , ") {
        Some((name, size)) if reported_size(size).is_some() => (name, reported_size(size)),
        _ => (header, None),
    };
    let name = name.trim();
    let (name, homebrew_cask) = match name.strip_suffix("[Brew]") {
        Some(name) => (name.trim_end(), true),
        None => (name, false),
    };
    matched
        .iter()
        .any(|candidate| candidate == name)
        .then(|| UninstallApp {
            name: name.to_string(),
            homebrew_cask,
            reported_size,
            items: Vec::new(),
        })
}

/// One path line: `✓ path , size`, `◎ System: path`, or `◎ Review only: path`.
fn parse_plan_item(trimmed: &str) -> Option<UninstallItem> {
    // Prefix first, scope second: the marker is what identifies the line, and
    // the classification is what it means.
    let (scope, rest) = [
        ("✓ ", UninstallItemScope::Removed),
        ("◎ System: ", UninstallItemScope::System),
        ("◎ Review only: ", UninstallItemScope::ReviewOnly),
    ]
    .into_iter()
    .find_map(|(prefix, scope)| trimmed.strip_prefix(prefix).map(|rest| (scope, rest)))?;

    let (display_path, size) = match rest.rsplit_once(" , ") {
        Some((path, size)) if reported_size(size).is_some() => (path, reported_size(size)),
        _ => (rest, None),
    };
    let display_path = display_path.trim();
    (!display_path.is_empty()).then(|| UninstallItem {
        display_path: display_path.to_string(),
        reported_size: size,
        scope,
    })
}

/// `Remove 1 app, 83.4MB  Enter confirm, ESC cancel:` → `83.4MB`.
fn parse_removal_note_total(rest: &str) -> Option<String> {
    let head = rest.split("  ").next()?;
    reported_size(head.rsplit_once(", ")?.1.trim())
}

/// `Would remove 1 app, would free 83.4MB: LocalSend` → `83.4MB`.
fn parse_summary_freed(trimmed: &str) -> Option<Option<String>> {
    if !trimmed.starts_with("Would remove ") && !trimmed.starts_with("Removed ") {
        return None;
    }
    let freed = trimmed
        .split_once(", would free ")
        .or_else(|| trimmed.split_once(", freed "))
        .map(|(_, tail)| tail.split(':').next().unwrap_or(tail).trim())
        .and_then(reported_size);
    Some(freed)
}

/// A size label Mole published, or `None` when the text is not one.
///
/// Deliberately strict, and deliberately still a string. It exists to tell a size
/// apart from a path fragment that happened to sit after a comma — not to turn
/// `83.4MB` into a number, which would invent three digits Mole never measured.
fn reported_size(text: &str) -> Option<String> {
    let text = text.trim();
    let unit_start = text.find(|character: char| character.is_ascii_alphabetic())?;
    let (number, unit) = text.split_at(unit_start);
    let valid = matches!(unit, "B" | "KB" | "MB" | "GB" | "TB" | "PB")
        && !number.is_empty()
        && number.matches('.').count() <= 1
        && number
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.');
    valid.then(|| text.to_string())
}

/// Read one real run's result out of its output.
///
/// Never fails: the run already happened. An unreadable summary becomes a
/// `Failed` with the transcript attached, because "Mole ran and I cannot tell you
/// what it did" is a true and useful thing to report, and an error that discarded
/// the transcript would not be.
fn parse_uninstall_execution(transcript: &str) -> UninstallExecution {
    let mut removed = Vec::new();
    let mut failed = Vec::new();
    let mut warnings = Vec::new();
    let mut reported_freed = None;
    let mut saw_summary = false;

    for line in transcript.lines() {
        let trimmed = line.trim();
        if let Some(freed) = parse_summary_freed(trimmed) {
            saw_summary = true;
            reported_freed = reported_freed.or(freed);
            if let Some((_, names)) = trimmed.split_once(": ") {
                removed.extend(
                    names
                        .split(", ")
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .map(str::to_string),
                );
            }
            continue;
        }
        if let Some(rest) = trimmed.split_once("Failed: ").map(|(_, rest)| rest) {
            failed.push(rest.trim().to_string());
            continue;
        }
        if let Some(note) = trimmed
            .strip_prefix("☞ ")
            .or_else(|| trimmed.strip_prefix("↳ "))
        {
            warnings.push(note.trim().to_string());
        }
    }

    let completion = if !failed.is_empty() {
        UninstallCompletion::Partial
    } else if saw_summary {
        UninstallCompletion::Finished
    } else {
        warnings.push(
            "Mole finished without printing a summary, so what it removed could not be read back. \
             Its full output is below."
                .to_string(),
        );
        UninstallCompletion::Failed
    };

    UninstallExecution {
        completion,
        removed,
        failed,
        reported_freed,
        warnings,
        transcript: transcript.to_string(),
    }
}

fn json_from_command<T: serde::de::DeserializeOwned>(
    binary: &Path,
    args: &[&str],
    operation: &'static str,
    cancel: &CancelToken,
) -> Result<T, AdapterError> {
    let mut command = process::command(binary);
    command.args(args);

    let mut process =
        RunningProcess::spawn(&mut command, cancel).map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;
    // Read the whole stream before parsing rather than deserializing from the
    // pipe. `from_reader` stops at the end of the value — or at the first parse
    // error — and drops the reader, and Mole writes its inventory one line at a
    // time: the next `printf` lands on a closed pipe, the backend dies of
    // SIGPIPE, and the real fault is replaced by "mo exited with status -1".
    // Draining first means a malformed field is reported as a malformed field.
    let parsed = process
        .take_stdout()
        .ok_or_else(|| AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "backend stdout was unavailable".to_string(),
        })
        .and_then(|mut stdout| {
            use std::io::Read;

            let mut bytes = Vec::new();
            stdout
                .read_to_end(&mut bytes)
                .map_err(|source| AdapterError::OperationFailed {
                    backend: "mole",
                    operation,
                    reason: source.to_string(),
                })?;
            Ok(bytes)
        })
        .and_then(|bytes| {
            serde_json::from_slice(&bytes).map_err(|source| AdapterError::MalformedBackendOutput {
                binary: BINARY,
                operation,
                reason: source.to_string(),
            })
        });

    let outcome = process.finish().map_err(|source| AdapterError::Spawn {
        binary: BINARY,
        source,
    })?;
    if outcome.cancelled {
        return Err(AdapterError::Cancelled {
            backend: "mole",
            operation,
        });
    }
    if !outcome.status.success() {
        return Err(AdapterError::BackendFailed {
            binary: BINARY,
            status: outcome.status.code().unwrap_or(-1),
            stderr: outcome.stderr,
        });
    }

    parsed
}

/// Pull a version out of `mo --version` output.
///
/// Observed shape on Mole 1.48.1 (Homebrew, macOS) — note the leading blank
/// line, which is in the real output and not a formatting artefact:
///
/// ```text
///
/// Mole version 1.48.1
/// macOS: 26.5.2
/// Architecture: arm64
/// ```
///
/// So this reads the first *non-empty* line, not the first line. The version is
/// its third token, where ncdu's is the second. Everything below that line
/// describes the host rather than the backend and changes from machine to
/// machine.
fn parse_version(output: &str) -> Option<String> {
    let line = output
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    let token = line.split_whitespace().nth(2)?;

    // Digit-led, so "Mole: command failed" cannot be mistaken for a version —
    // the same "stderr as data" failure the ncdu adapter guards against.
    token
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| token.to_string())
}

/// Accept 1.48 and later within 1.x.
///
/// A lower bound as well as an upper one, because this is the only version the
/// output shapes above were recorded from. An older 1.x may well work; until
/// somebody records its output, saying so would be a guess.
fn is_supported(version: &str) -> bool {
    match parts_of(version) {
        Some((1, minor)) => minor >= 48,
        _ => false,
    }
}

fn parts_of(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next().unwrap_or("0").parse().ok()?;
    Some((major, minor))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    static TEST_SCRIPT_SEQUENCE: std::sync::atomic::AtomicU64 =
        std::sync::atomic::AtomicU64::new(0);

    #[cfg(unix)]
    fn executable_script(contents: &str) -> PathBuf {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        let sequence = TEST_SCRIPT_SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let script = std::env::temp_dir().join(format!(
            "nirmoka-mole-test-{}-{sequence}.sh",
            std::process::id()
        ));
        fs::write(&script, contents).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        script
    }

    /// A backend that blocks until it is killed.
    ///
    /// `exec` is load-bearing: without it the shell's `sleep` survives the
    /// killed shell, holds the stdout pipe open, and the cancellation tests wait
    /// the full minute for a process they already stopped.
    #[cfg(unix)]
    fn blocking_script() -> PathBuf {
        executable_script("#!/bin/sh\nexec sleep 60\n")
    }

    const STATUS: &str = include_str!("../../../fixtures/mole/1.48.1/status.json");
    const APPLICATIONS: &str = include_str!("../../../fixtures/mole/1.48.1/applications.json");
    const CLEAN_PREVIEW: &str = include_str!("../../../fixtures/mole/1.48.1/clean-list.txt");
    const UNINSTALL_SURFACE: &str =
        include_str!("../../../fixtures/mole/1.48.1/uninstall-command-surface.txt");
    const UNINSTALL_PLAN: &str = include_str!("../../../fixtures/mole/1.48.1/uninstall-plan.txt");

    /// Byte-for-byte what `mo --version` printed on Mole 1.48.1, leading blank
    /// line included. Trimming it here to make the parser's job easier is how
    /// a test ends up asserting against the code instead of the backend.
    const OBSERVED: &str =
        "\nMole version 1.48.1\nmacOS: 26.5.2\nArchitecture: arm64\nKernel: 25.5.0\n";

    #[test]
    fn parses_the_observed_homebrew_output() {
        assert_eq!(parse_version(OBSERVED).as_deref(), Some("1.48.1"));
    }

    /// The bug this caught: `lines().next()` reads the blank line Mole opens
    /// with, and detection reports `UnreadableVersion` against a perfectly good
    /// install.
    #[test]
    fn a_leading_blank_line_is_not_the_version_line() {
        assert!(
            OBSERVED.starts_with('\n'),
            "the fixture lost its blank line"
        );
        assert_eq!(
            parse_version("\n\n  Mole version 1.48.1\n").as_deref(),
            Some("1.48.1")
        );
    }

    #[test]
    fn rejects_error_text_as_a_version() {
        assert!(parse_version("mo: command not found").is_none());
        assert!(parse_version("Mole version").is_none());
        assert!(parse_version("").is_none());
    }

    #[test]
    fn gates_to_the_versions_actually_recorded() {
        assert!(is_supported("1.48.1"));
        assert!(is_supported("1.60.0"));

        assert!(!is_supported("1.47.9"), "older than anything recorded");
        assert!(!is_supported("2.0.0"), "a new major is a new format");
        assert!(!is_supported("nonsense"));
    }

    /// The whole point of this adapter's shape. A caller that ignores
    /// `capabilities().scan` gets an error naming the backend, not a tree one
    /// level deep that looks like a small disk.
    #[test]
    fn scanning_is_refused_rather_than_faked() {
        struct Discard;
        impl WireSink for Discard {
            fn open_dir(&mut self, _item: nirmoka_adapter::wire::WireItem) {}
            fn item(&mut self, _item: nirmoka_adapter::wire::WireItem) {}
            fn close_dir(&mut self) {}
        }

        let error = MoleAdapter::new()
            .scan(
                Path::new("/tmp"),
                &ScanOptions::default(),
                &mut Discard,
                &CancelToken::new(),
            )
            .expect_err("mole cannot scan");

        assert!(
            matches!(
                error,
                AdapterError::Unsupported {
                    backend: "mole",
                    operation: "scan"
                }
            ),
            "got: {error}"
        );
    }

    #[test]
    fn capabilities_say_what_the_cli_offers_and_nothing_more() {
        let caps = MoleAdapter::new().capabilities();

        assert!(!caps.scan, "mo analyze lists one directory, not a tree");
        assert!(
            !caps.trash,
            "recoverability is not documented, so not claimed"
        );
        assert!(caps.dry_run && caps.cleanup_categories);
        assert!(caps.system_status);
        assert!(
            caps.app_inventory,
            "mo uninstall --list is machine-readable"
        );
        assert!(
            caps.uninstall_apps,
            "the plan is one line of input away, and the removal needs only the user's approval"
        );
    }

    /// The gate that makes ADR 0027 re-testable rather than remembered.
    ///
    /// Two things have to stay true. The prompt is still there — so the adapter
    /// still has to answer it, and a release that dropped it would mean this
    /// whole flow can be simplified. And Mole still routes to the Trash by
    /// default, with `--permanent` the opt-in the adapter never passes: if that
    /// default inverted, every uninstall would silently become unrecoverable.
    #[test]
    fn the_recorded_surface_still_prompts_and_still_defaults_to_the_trash() {
        assert!(
            UNINSTALL_SURFACE.contains("Proceed with uninstallation?"),
            "Mole no longer prompts; the confirmation this adapter writes may now be unnecessary"
        );
        assert!(
            UNINSTALL_SURFACE.contains("uninstalled files go to the macOS Trash"),
            "Mole no longer documents Trash routing as the default; an uninstall may now be \
             unrecoverable, so revisit ADR 0027 before shipping"
        );
        assert!(
            UNINSTALL_SURFACE.contains("--permanent"),
            "the flag this adapter must never pass is no longer documented"
        );
    }

    /// What actually reaches Mole's command line, recorded by a backend that
    /// writes its own argv down.
    ///
    /// Two claims in one, both about the arguments rather than about the parse.
    /// `--permanent` is the single flag that would turn the recoverable operation
    /// the user approved into an unrecoverable one. And no path from the reviewed
    /// plan is forwarded: Mole rediscovers, which is what makes the preview an
    /// accurate description of this run instead of a list to be reproduced.
    #[test]
    fn execution_passes_the_identifier_and_no_flag_that_changes_the_operation() {
        let script = executable_script(
            r#"#!/bin/sh
if [ "$1" = "uninstall" ] && [ "$2" = "--list" ]; then
  cat "$0.inventory"
  exit 0
fi
printf '%s\n' "$@" > "$0.argv"
read -r answer
printf '%s' "$answer" > "$0.stdin"
printf 'Uninstall complete\nRemoved 1 app, freed 83.4MB: Example Cask\n'
"#,
        );
        std::fs::write(format!("{}.inventory", script.display()), APPLICATIONS)
            .expect("inventory fixture");
        let argv_path = PathBuf::from(format!("{}.argv", script.display()));
        let stdin_path = PathBuf::from(format!("{}.stdin", script.display()));

        let execution =
            execute_uninstall_from(&script, &["example-cask".to_string()], &CancelToken::new())
                .expect("uninstall execution");

        let argv: Vec<String> = std::fs::read_to_string(&argv_path)
            .expect("backend ran")
            .lines()
            .map(str::to_string)
            .collect();
        assert_eq!(argv, vec!["uninstall", "example-cask"]);
        assert!(
            !argv.iter().any(|argument| argument == "--permanent"),
            "the user approved a recoverable uninstall"
        );
        assert!(
            !argv.iter().any(|argument| argument.starts_with('/')),
            "no reviewed path may be forwarded as an argument: {argv:?}"
        );
        // The confirmation is one line and nothing else. A backend reading a
        // second answer finds end of input.
        assert_eq!(
            std::fs::read_to_string(&stdin_path).expect("the prompt was answered"),
            "y"
        );
        assert_eq!(execution.completion, UninstallCompletion::Finished);
        assert_eq!(execution.removed, vec!["Example Cask".to_string()]);

        let _ = std::fs::remove_file(format!("{}.inventory", script.display()));
        let _ = std::fs::remove_file(&argv_path);
        let _ = std::fs::remove_file(&stdin_path);
        let _ = std::fs::remove_file(script);
    }

    /// The preview asks for `--dry-run`, and asks for nothing else that would
    /// change what Mole does.
    #[test]
    fn the_preview_runs_only_moles_own_dry_run() {
        let script = executable_script(
            r#"#!/bin/sh
if [ "$1" = "uninstall" ] && [ "$2" = "--list" ]; then
  cat "$0.inventory"
  exit 0
fi
printf '%s\n' "$@" > "$0.argv"
read -r _answer
cat "$0.plan"
"#,
        );
        std::fs::write(format!("{}.inventory", script.display()), APPLICATIONS)
            .expect("inventory fixture");
        std::fs::write(format!("{}.plan", script.display()), UNINSTALL_PLAN).expect("plan fixture");
        let argv_path = PathBuf::from(format!("{}.argv", script.display()));

        let preview = uninstall_preview_from(
            &script,
            &["example-cask".to_string()],
            "1.48.1",
            &CancelToken::new(),
        )
        .expect("uninstall preview");

        let argv: Vec<String> = std::fs::read_to_string(&argv_path)
            .expect("backend ran")
            .lines()
            .map(str::to_string)
            .collect();
        assert_eq!(argv, vec!["uninstall", "--dry-run", "example-cask"]);
        assert_eq!(preview.apps.len(), 1);
        assert_eq!(preview.backend_version, "1.48.1");

        let _ = std::fs::remove_file(format!("{}.inventory", script.display()));
        let _ = std::fs::remove_file(format!("{}.plan", script.display()));
        let _ = std::fs::remove_file(&argv_path);
        let _ = std::fs::remove_file(script);
    }

    /// Cancelling must kill the subprocess rather than orphan it — the contract's
    /// rule, and one an uninstall needs more than a scan does.
    #[test]
    fn cancelling_an_uninstall_kills_the_subprocess_and_reports_the_run() {
        let script = executable_script(
            r#"#!/bin/sh
if [ "$1" = "uninstall" ] && [ "$2" = "--list" ]; then
  cat "$0.inventory"
  exit 0
fi
: > "$0.started"
exec sleep 60
"#,
        );
        std::fs::write(format!("{}.inventory", script.display()), APPLICATIONS)
            .expect("inventory fixture");
        let started = PathBuf::from(format!("{}.started", script.display()));

        let cancel = CancelToken::new();
        let worker_cancel = cancel.clone();
        let worker_script = script.clone();
        let worker = std::thread::spawn(move || {
            execute_uninstall_from(
                &worker_script,
                &["example-cask".to_string()],
                &worker_cancel,
            )
        });

        // Waited for rather than slept past. The inventory check runs first, and
        // a fixed delay long enough to clear it on this machine is a delay that
        // cancels the wrong subprocess on a slower one.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        while !started.exists() {
            assert!(
                std::time::Instant::now() < deadline,
                "the uninstall never started"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        cancel.cancel();

        let execution = worker
            .join()
            .unwrap()
            .expect("a stopped run still happened");
        assert_eq!(execution.completion, UninstallCompletion::Cancelled);
        assert!(execution
            .warnings
            .iter()
            .any(|warning| warning.contains("stays there")));

        let _ = std::fs::remove_file(format!("{}.inventory", script.display()));
        let _ = std::fs::remove_file(&started);
        let _ = std::fs::remove_file(script);
    }

    #[test]
    fn the_recorded_plan_is_a_dry_run_that_reached_its_own_summary() {
        assert!(
            UNINSTALL_PLAN.contains("DRY RUN MODE, No app files or settings will be modified"),
            "the recorded plan is not from a dry run"
        );
        assert!(
            UNINSTALL_PLAN.contains("Uninstall dry run complete"),
            "the recorded plan never reached Mole's summary, so it may be truncated"
        );
    }

    /// The parse of the real recording, field by field.
    ///
    /// A hand-written input would test the parser against itself. This one came
    /// out of Mole with the same `y` the adapter writes.
    #[test]
    fn parses_the_recorded_dry_run_plan() {
        let preview = parse_uninstall_preview(
            UNINSTALL_PLAN,
            &["example-cask".to_string()],
            "1.48.1",
            PlanMode::DryRun,
        )
        .expect("the recorded plan");

        assert_eq!(preview.requested, vec!["example-cask".to_string()]);
        assert_eq!(preview.reported_total.as_deref(), Some("83.4MB"));
        assert_eq!(preview.apps.len(), 1);

        let app = &preview.apps[0];
        assert_eq!(app.name, "Example");
        assert!(app.homebrew_cask, "the [Brew] tag was recorded");
        assert_eq!(app.reported_size.as_deref(), Some("83.4MB"));
        assert_eq!(app.items.len(), 7);
        assert_eq!(app.items[0].display_path, "/Applications/Example.app");
        assert_eq!(app.items[0].reported_size.as_deref(), Some("83.2MB"));
        assert_eq!(app.items[0].scope, UninstallItemScope::Removed);
        // A path with no size stays a path: the size suffix is optional, and
        // splitting on the comma regardless would truncate one.
        assert_eq!(
            app.items[2].display_path,
            "~/Library/Application Scripts/com.example.desktop"
        );
        assert_eq!(app.items[2].reported_size, None);
        assert_eq!(preview.total_items(), 7);

        // Mole's `--zap` notice is not an application, and it is not silently
        // dropped either.
        assert!(
            preview
                .warnings
                .iter()
                .any(|warning| warning.contains("--zap removes configs and data")),
            "{:?}",
            preview.warnings
        );
        assert!(
            preview
                .notes
                .iter()
                .any(|note| note.starts_with("Local Network permissions")),
            "{:?}",
            preview.notes
        );
        assert!(
            preview.transcript.contains("Proceed with uninstallation?"),
            "the transcript is what the user approves, so it keeps the prompt"
        );
    }

    /// The classifications that decide whether a row means "will be removed" or
    /// "you have to deal with this yourself". Rendering the second as the first
    /// would promise a removal that never happens.
    #[test]
    fn system_and_review_only_rows_keep_their_own_scope() {
        let plan = "\
◎ Matched 1 app(s):
1. Example  1MB  |  Last: <when>

Files to be removed:

◎ Example , 1MB
  ✓ /Applications/Example.app , 1MB
  ◎ System: /Library/LaunchDaemons/com.example.helper.plist
  ◎ Review only: /Library/Preferences/com.example.plist
Uninstall dry run complete
";
        let preview = parse_uninstall_preview(plan, &[], "1.48.1", PlanMode::DryRun)
            .expect("a plan with every row kind");
        let scopes: Vec<_> = preview.apps[0]
            .items
            .iter()
            .map(|item| item.scope)
            .collect();
        assert_eq!(
            scopes,
            vec![
                UninstallItemScope::Removed,
                UninstallItemScope::System,
                UninstallItemScope::ReviewOnly
            ]
        );
        assert!(preview.has_review_only_items());
        assert!(!preview.apps[0].homebrew_cask);
    }

    /// Every way Mole's plan can stop making sense has to be an error rather than
    /// a shorter plan. A narrowed list shown as a complete one is the failure this
    /// whole parser exists to prevent.
    #[test]
    fn a_changed_plan_structure_is_refused_rather_than_narrowed() {
        let complete = "\
◎ Matched 1 app(s):
1. Example  1MB  |  Last: <when>

Files to be removed:

◎ Example , 1MB
  ✓ /Applications/Example.app , 1MB
Uninstall dry run complete
";
        assert!(parse_uninstall_preview(complete, &[], "1.48.1", PlanMode::DryRun).is_ok());

        for (reason, plan) in [
            (
                "no match header",
                complete.replace("◎ Matched 1 app(s):", ""),
            ),
            (
                "the declared count disagrees with the list",
                complete.replace("Matched 1 app(s)", "Matched 2 app(s)"),
            ),
            (
                "no file section",
                complete.replace("Files to be removed:", ""),
            ),
            (
                "no terminal summary",
                complete.replace("Uninstall dry run complete", ""),
            ),
            (
                "a matched app was never detailed",
                complete.replace("◎ Example , 1MB", ""),
            ),
        ] {
            let error =
                parse_uninstall_preview(&plan, &[], "1.48.1", PlanMode::DryRun).expect_err(reason);
            assert!(
                matches!(error, AdapterError::MalformedBackendOutput { .. }),
                "{reason}: {error}"
            );
        }
    }

    /// An app whose display name Mole did not match cannot open a section. The
    /// marker is shared with Mole's own notices, so the match list is what tells
    /// them apart — and a notice that looked like an app would attach the next
    /// app's paths to it.
    #[test]
    fn a_notice_sharing_the_app_marker_is_not_read_as_an_app() {
        let plan = "\
◎ Matched 1 app(s):
1. Example  1MB  |  Last: <when>

Files to be removed:
◎ Some notice Mole has not written yet , 4KB

◎ Example , 1MB
  ✓ /Applications/Example.app , 1MB
Uninstall dry run complete
";
        let preview =
            parse_uninstall_preview(plan, &[], "1.48.1", PlanMode::DryRun).expect("one app");
        assert_eq!(preview.apps.len(), 1);
        assert_eq!(preview.apps[0].name, "Example");
        assert_eq!(preview.apps[0].items.len(), 1);
        assert!(preview
            .warnings
            .iter()
            .any(|warning| warning.starts_with("Some notice")));
    }

    /// An application name with spaces is the common case, not the exotic one:
    /// "Google Chrome", "Visual Studio Code", "LM Studio".
    #[test]
    fn a_matched_name_may_contain_spaces() {
        let plan = "\
◎ Matched 2 app(s):
1. Google Chrome  1.47GB  |  Last: <when>
2. LM Studio  7.1GB  |  Last: <when>

Files to be removed:

◎ Google Chrome , 1.47GB
  ✓ /Applications/Google Chrome.app , 1.47GB

◎ LM Studio , 7.1GB
  ✓ /Applications/LM Studio.app , 7.1GB
Uninstall dry run complete
";
        let preview =
            parse_uninstall_preview(plan, &[], "1.48.1", PlanMode::DryRun).expect("two apps");
        let names: Vec<_> = preview.apps.iter().map(|app| app.name.as_str()).collect();
        assert_eq!(names, vec!["Google Chrome", "LM Studio"]);
        assert_eq!(
            preview.apps[1].items[0].display_path,
            "/Applications/LM Studio.app"
        );
    }

    #[test]
    fn a_size_is_only_read_where_mole_published_one() {
        assert_eq!(reported_size("83.4MB").as_deref(), Some("83.4MB"));
        assert_eq!(reported_size("4KB").as_deref(), Some("4KB"));
        assert_eq!(reported_size("2.07GB").as_deref(), Some("2.07GB"));
        // Not sizes. Each of these would otherwise be read off the end of a path
        // that happened to contain a comma.
        assert_eq!(reported_size("Application Scripts"), None);
        assert_eq!(reported_size("1.2.3MB"), None);
        assert_eq!(reported_size("MB"), None);
        assert_eq!(reported_size("12"), None);
        assert_eq!(reported_size("12 Mb"), None);
    }

    #[test]
    fn a_real_run_reports_what_it_removed() {
        let execution = parse_uninstall_execution(
            "\
Uninstall complete
Removed 1 app, freed 83.4MB: Example
☞ Local Network permissions on macOS 15+ can outlive app removal: Example
",
        );
        assert_eq!(execution.completion, UninstallCompletion::Finished);
        assert_eq!(execution.removed, vec!["Example".to_string()]);
        assert_eq!(execution.reported_freed.as_deref(), Some("83.4MB"));
        assert!(execution.failed.is_empty());
        assert_eq!(execution.warnings.len(), 1);
    }

    #[test]
    fn a_partly_failed_run_is_not_reported_as_finished() {
        let execution = parse_uninstall_execution(
            "\
Uninstall complete
Removed 1 app, freed 12MB: Example
✗ Failed: Other is still running
",
        );
        assert_eq!(execution.completion, UninstallCompletion::Partial);
        assert_eq!(execution.removed, vec!["Example".to_string()]);
        assert_eq!(execution.failed, vec!["Other is still running".to_string()]);
    }

    /// The case that must never read as success. Mole ran, so files may be gone,
    /// and the summary that would say which is missing.
    #[test]
    fn a_run_with_no_summary_is_failed_and_keeps_its_transcript() {
        let execution = parse_uninstall_execution("→ Scanning applications\n");
        assert_eq!(execution.completion, UninstallCompletion::Failed);
        assert!(execution.removed.is_empty());
        assert_eq!(execution.transcript, "→ Scanning applications\n");
        assert!(execution.warnings[0].contains("without printing a summary"));
    }

    /// Cancellation is an outcome, not an error: the subprocess was killed part
    /// way through, and whatever it had already moved stays moved.
    #[test]
    fn a_cancelled_run_reports_the_removal_it_may_have_started() {
        let execution = uninstall_execution_of(
            b"Removed 1 app, freed 1MB: Example\n",
            nirmoka_adapter::process::Outcome {
                status: std::process::Command::new("false")
                    .status()
                    .expect("a failing status"),
                stderr: String::new(),
                cancelled: true,
            },
            Ok(()),
        );
        assert_eq!(execution.completion, UninstallCompletion::Cancelled);
        assert!(execution.warnings[0].contains("stays there"));
        // The removal Mole did report survives the cancellation.
        assert_eq!(execution.removed, vec!["Example".to_string()]);
    }

    /// An identifier only reaches the command line if the backend just published
    /// it, so a name that is really a flag is not a case that can arise.
    #[test]
    fn only_an_identifier_mole_published_can_become_an_argument() {
        let listing = executable_script(&format!(
            "#!/bin/sh\nif [ \"$1\" = uninstall ]; then cat <<'JSON'\n{APPLICATIONS}\nJSON\nfi\n"
        ));
        let cancel = CancelToken::new();

        assert_eq!(
            validated_names(
                &listing,
                &["example-cask".to_string()],
                "uninstall preview",
                &cancel
            )
            .expect("a published identifier"),
            vec!["example-cask".to_string()]
        );

        for rejected in ["--permanent", "Example Cask", "", "example-cask "] {
            let error = validated_names(
                &listing,
                &[rejected.to_string()],
                "uninstall preview",
                &cancel,
            )
            .expect_err(rejected);
            assert!(
                matches!(error, AdapterError::OperationFailed { .. }),
                "{rejected}: {error}"
            );
        }

        let error = validated_names(&listing, &[], "uninstall preview", &cancel)
            .expect_err("no application named");
        assert!(matches!(error, AdapterError::OperationFailed { .. }));
    }

    /// Same duplicate twice is one argument. Mole matches by name, and passing it
    /// twice would list the app twice and then disagree with its own count.
    #[test]
    fn a_repeated_identifier_is_passed_once() {
        let listing = executable_script(&format!(
            "#!/bin/sh\nif [ \"$1\" = uninstall ]; then cat <<'JSON'\n{APPLICATIONS}\nJSON\nfi\n"
        ));
        assert_eq!(
            validated_names(
                &listing,
                &["example-cask".to_string(), "example-cask".to_string()],
                "uninstall preview",
                &CancelToken::new(),
            )
            .expect("one identifier"),
            vec!["example-cask".to_string()]
        );
    }

    /// The capability split has to leave inventory working. Refusing uninstall by
    /// refusing everything app-shaped would be the easy wrong answer.
    #[test]
    fn application_inventory_does_not_inherit_the_uninstall_refusal() {
        let error = MoleAdapter::new()
            .installed_applications(&CancelToken::new())
            .err();

        // Inventory stays available wherever Mole is installed; the refusal
        // being asserted is the capability split, checked above. This only
        // pins that inventory is not refused *because* uninstall is.
        if let Some(error) = error {
            assert!(
                !matches!(error, AdapterError::Unsupported { .. }),
                "inventory must not inherit the uninstall refusal: {error}"
            );
        }
    }

    #[test]
    fn parses_the_sanitized_status_fixture() {
        let status: SystemStatus = serde_json::from_str(STATUS).expect("status fixture");

        assert_eq!(status.health_score, 91);
        assert_eq!(status.cpu.logical_cpu, 8);
        assert_eq!(status.memory.total, 17_179_869_184);
        assert_eq!(status.disks[0].mount, "/");
        assert_eq!(status.batteries[0].cycle_count, 120);
        assert_eq!(status.thermal.cpu_temp, Some(48.0));
    }

    #[test]
    #[cfg(unix)]
    fn malformed_status_is_rejected_as_backend_output() {
        let script = executable_script("#!/bin/sh\nprintf '%s' '{\"health_score\":\"healthy\"}'\n");
        let error = status_from(&script, &CancelToken::new()).expect_err("wrong types must fail");
        let _ = std::fs::remove_file(script);

        assert!(matches!(
            error,
            AdapterError::MalformedBackendOutput {
                operation: "system status",
                ..
            }
        ));
    }

    #[test]
    fn parses_the_sanitized_application_fixture() {
        let applications: Vec<InstalledApplication> =
            serde_json::from_str(APPLICATIONS).expect("application fixture");

        assert_eq!(applications.len(), 2);
        assert_eq!(applications[0].bundle_id, "com.example.desktop");
        assert_eq!(applications[0].uninstall_name, "Example");
        assert_eq!(applications[0].path, Path::new("/Applications/Example.app"));
        // A rounded string, because that is what the backend publishes. A
        // fixture claiming a byte count is how this reached a release broken.
        assert_eq!(applications[0].reported_size, "767.3MB");
        assert_eq!(applications[1].uninstall_name, "example-cask");
        assert_eq!(applications[1].source, "Homebrew");
    }

    /// The failure that shipped: Mole writes its inventory one entry at a time,
    /// so a parser that stops at the first bad field and drops the pipe kills
    /// the backend with SIGPIPE — and the caller is told `mo exited with status
    /// -1: printf: write error: Broken pipe`, which names neither the field nor
    /// the file. Draining first means the schema mismatch reports itself.
    #[test]
    #[cfg(unix)]
    fn a_schema_mismatch_is_reported_instead_of_a_broken_pipe() {
        let script = executable_script(
            r#"#!/bin/sh
printf '%s
' '['
printf '%s
' '  {"name": "Example", "bundle_id": "com.example", "source": "App", "uninstall_name": "example", "path": "/Applications/Example.app", "size": 42},'
# Mole emits one line per application. Sleeping here is what a real inventory
# does between entries, and it is what turns an early close into SIGPIPE.
sleep 0.3
printf '%s
' '  {"name": "Later", "bundle_id": "com.example.later", "source": "App", "uninstall_name": "later", "path": "/Applications/Later.app", "size": "1.0MB"}'
printf '%s
' ']'
"#,
        );

        let error = applications_from(&script, &CancelToken::new())
            .expect_err("a numeric size is not what this backend publishes");
        let _ = std::fs::remove_file(script);

        assert!(
            matches!(
                error,
                AdapterError::MalformedBackendOutput {
                    operation: "application inventory",
                    ..
                }
            ),
            "{error}"
        );
        assert!(
            !error.to_string().contains("Broken pipe"),
            "the backend's death must not replace the reason: {error}"
        );
    }

    /// Run against the Mole on this machine. Ignored by default because CI has
    /// none — and because the hand-written fixture this replaces is exactly the
    /// kind of thing that passes every test and fails every user.
    ///
    /// `cargo test -p nirmoka-adapter-mole -- --ignored live_`
    #[test]
    #[ignore = "requires a real Mole install"]
    fn live_inventory_parses_against_the_installed_backend() {
        let adapter = MoleAdapter::new();
        let applications = adapter
            .installed_applications(&CancelToken::new())
            .expect("the installed Mole must produce a parseable inventory");

        assert!(!applications.is_empty(), "this machine has applications");
        for application in &applications {
            assert!(!application.uninstall_name.is_empty());
            assert!(!application.reported_size.is_empty());
        }
    }

    /// Parse a real plan out of the real backend, and prove the dry run was one.
    ///
    /// The recorded fixture makes the parser testable in CI; this makes it
    /// testable against whatever Mole is actually installed, which is the version
    /// that will run on a user's machine. It removes nothing — `--dry-run` is set
    /// before any discovery — and it asserts that, by checking the bundle it
    /// claims it would remove is still there afterwards.
    ///
    /// `cargo test -p nirmoka-adapter-mole -- --ignored live_`
    #[test]
    #[ignore = "requires a real Mole install"]
    fn live_uninstall_preview_parses_and_removes_nothing() {
        let adapter = MoleAdapter::new();
        let cancel = CancelToken::new();
        let applications = adapter
            .installed_applications(&cancel)
            .expect("an inventory to pick a target from");
        // Prefer one whose display name and identifier disagree — a Homebrew cask.
        // That probes the path where the identifier matters, which is the one a
        // display name would silently get wrong.
        let target = applications
            .iter()
            .find(|application| application.uninstall_name != application.name)
            .or_else(|| applications.first())
            .expect("this machine has applications");

        let preview = adapter
            .uninstall_preview(std::slice::from_ref(&target.uninstall_name), &cancel)
            .expect("the installed Mole must produce a parseable plan");

        assert_eq!(preview.requested, vec![target.uninstall_name.clone()]);
        assert_eq!(preview.apps.len(), 1, "one identifier, one app");
        assert!(
            preview.total_items() > 0,
            "a plan that lists nothing is not a plan"
        );
        assert!(
            preview.transcript.contains("DRY RUN MODE"),
            "the preview must be a dry run"
        );

        // The claim that matters. Every path it says it would remove is still
        // present, the application bundle included.
        assert!(
            target.path.exists(),
            "the preview moved {}",
            target.path.display()
        );
        for item in preview.apps.iter().flat_map(|app| &app.items) {
            assert!(
                !item.display_path.is_empty(),
                "a plan row with no path is a parse failure"
            );
        }
    }

    #[test]
    fn malformed_application_inventory_is_rejected() {
        let error = serde_json::from_str::<Vec<InstalledApplication>>(r#"[{"name":"Example"}]"#)
            .expect_err("missing command identity must fail");

        assert!(error.to_string().contains("bundle_id"));
    }

    #[test]
    #[cfg(unix)]
    fn application_inventory_uses_the_noninteractive_list_command() {
        let script = executable_script(
            r#"#!/bin/sh
[ "$1" = "uninstall" ] && [ "$2" = "--list" ] || exit 12
printf '%s' '[{"name":"Example","bundle_id":"com.example.desktop","source":"App","uninstall_name":"Example","path":"/Applications/Example.app","size":"42B"}]'
"#,
        );
        let applications =
            applications_from(&script, &CancelToken::new()).expect("application list");
        let _ = std::fs::remove_file(script);

        assert_eq!(applications[0].uninstall_name, "Example");
        assert_eq!(applications[0].reported_size, "42B");
    }

    #[test]
    fn parses_the_sanitized_cleanup_preview_fixture() {
        let preview = parse_cleanup_preview(CLEAN_PREVIEW, "1.48.1", CleanupSystemScope::UserOnly)
            .expect("cleanup preview fixture");

        assert_eq!(preview.backend_version, "1.48.1");
        assert_eq!(preview.generated_at, "2026-08-01 12:30:00");
        assert_eq!(preview.categories.len(), 2);
        assert_eq!(preview.categories[0].items[0].item_count, 4);
        assert_eq!(preview.categories[1].items[0].reported_size, None);
        assert_eq!(
            preview.potential_cleanup.as_deref(),
            Some("At least 192.00MB")
        );
        assert_eq!(preview.total_items, 6);
        assert_eq!(preview.warnings.len(), 1);
    }

    #[test]
    fn cleanup_preview_rejects_summary_drift() {
        let changed = CLEAN_PREVIEW.replace("# Items: 6", "# Items: 600");
        let error = parse_cleanup_preview(&changed, "1.48.1", CleanupSystemScope::Included)
            .expect_err("inconsistent preview must fail");

        assert!(matches!(error, AdapterError::MalformedBackendOutput { .. }));
    }

    #[test]
    fn cleanup_preview_rejects_a_missing_or_partial_summary() {
        for declaration in [
            "# Potential cleanup: At least 192.00MB\n",
            "# Items: 6\n",
            "# Categories: 2\n",
        ] {
            let changed = CLEAN_PREVIEW.replace(declaration, "");
            let error = parse_cleanup_preview(&changed, "1.48.1", CleanupSystemScope::Included)
                .expect_err("every nonempty preview summary declaration must be present");

            assert!(matches!(error, AdapterError::MalformedBackendOutput { .. }));
        }
    }

    #[test]
    fn cleanup_preview_requires_an_ordered_terminal_summary() {
        let trailer = "# Potential cleanup: At least 192.00MB\n# Items: 6\n# Categories: 2\n";
        let early = CLEAN_PREVIEW.replacen(
            "=== Browser caches ===",
            &format!("{trailer}\n=== Browser caches ==="),
            1,
        );
        let first_cleanup_row = CLEAN_PREVIEW
            .lines()
            .find(|line| line.contains("128.00MB, 4 items"))
            .expect("fixture has a grouped cleanup row");
        let interleaved = CLEAN_PREVIEW.replacen(
            &format!("{first_cleanup_row}\n"),
            &format!("{first_cleanup_row}\n{trailer}"),
            1,
        );
        let reordered = CLEAN_PREVIEW.replace(
            "# Potential cleanup: At least 192.00MB\n# Items: 6\n",
            "# Items: 6\n# Potential cleanup: At least 192.00MB\n",
        );
        let trailing = format!("{CLEAN_PREVIEW}/tmp/trailing  # 1KB\n");

        for changed in [early, interleaved, reordered, trailing] {
            let error = parse_cleanup_preview(&changed, "1.48.1", CleanupSystemScope::Included)
                .expect_err("the summary must be an ordered terminal trailer");

            assert!(matches!(error, AdapterError::MalformedBackendOutput { .. }));
        }
    }

    #[test]
    fn cleanup_preview_accepts_an_empty_backend_plan() {
        let preview = parse_cleanup_preview(
            "# Mole Cleanup Preview - 2026-08-01 12:30:00\n#\n# Potential cleanup: 0B\n# Items: 0\n# Categories: 0\n",
            "1.48.1",
            CleanupSystemScope::Included,
        )
        .expect("empty cleanup preview");

        assert!(preview.categories.is_empty());
        assert_eq!(preview.total_items, 0);
        assert_eq!(preview.potential_cleanup.as_deref(), Some("0B"));
    }

    #[test]
    fn cleanup_preview_rejects_category_free_output_without_summary() {
        let preview = "# Mole Cleanup Preview - 2026-08-01 12:30:00\n#\n# How to protect files:\n";
        let error = parse_cleanup_preview(preview, "1.48.1", CleanupSystemScope::Included)
            .expect_err("category-free output needs completeness evidence");

        assert!(matches!(error, AdapterError::MalformedBackendOutput { .. }));
    }

    #[test]
    fn cleanup_preview_rejects_unrecognized_sizes() {
        let preview = "# Mole Cleanup Preview - now\n=== Cache ===\n/tmp/cache  # lots\n";
        let error = parse_cleanup_preview(preview, "1.48.1", CleanupSystemScope::Unknown)
            .expect_err("unrecognized size must fail");

        assert!(error.to_string().contains("invalid cleanup size"));
    }

    #[test]
    fn cleanup_preview_preserves_newlines_inside_backend_paths() {
        let preview = "# Mole Cleanup Preview - now\n=== Logs ===\n/fixtures/mole/logs\n,  # 0B\n# Potential cleanup: 0B\n# Items: 1\n# Categories: 1\n";

        let parsed = parse_cleanup_preview(preview, "1.48.1", CleanupSystemScope::UserOnly)
            .expect("newline-bearing path");

        assert_eq!(
            parsed.categories[0].items[0].path,
            PathBuf::from("/fixtures/mole/logs\n,")
        );
    }

    #[test]
    #[cfg(unix)]
    fn cleanup_preview_runs_only_moles_dry_run_and_reads_its_published_file() {
        let script = executable_script(
            r#"#!/bin/sh
[ "$1" = "clean" ] && [ "$2" = "--dry-run" ] && [ "$#" = "2" ] || exit 12
printf '%s' 'System caches need sudo'
"#,
        );
        let preview_path = script.with_extension("preview");
        std::fs::write(&preview_path, CLEAN_PREVIEW).unwrap();

        let preview = cleanup_preview_from(&script, &preview_path, "1.48.1", &CancelToken::new())
            .expect("cleanup preview");
        let _ = std::fs::remove_file(script);
        let _ = std::fs::remove_file(preview_path);

        assert_eq!(preview.system_scope, CleanupSystemScope::UserOnly);
        assert_eq!(preview.total_items, 6);
    }

    #[test]
    #[cfg(unix)]
    fn cleanup_execution_asks_mole_to_rediscover_without_preview_paths() {
        let script = executable_script(
            r#"#!/bin/sh
[ "$1" = "clean" ] && [ "$#" = "1" ] || exit 12
printf '%s' 'fresh backend discovery' > "$0.executed"
printf '%s\n' 'System-level cleanup enabled, sudo session active'
"#,
        );
        let marker = PathBuf::from(format!("{}.executed", script.display()));

        let execution =
            execute_cleanup_from(&script, &CancelToken::new()).expect("cleanup execution");

        assert_eq!(execution.completion, CleanupCompletion::Finished);
        assert_eq!(execution.system_scope, CleanupSystemScope::Included);
        assert_eq!(
            std::fs::read_to_string(&marker).expect("backend ran"),
            "fresh backend discovery"
        );
        let _ = std::fs::remove_file(script);
        let _ = std::fs::remove_file(marker);
    }

    #[test]
    fn cleanup_execution_reports_missing_authorization_and_partial_failures() {
        let execution = parse_cleanup_execution(
            "\u{1b}[90mSystem-level cleanup skipped, requires sudo\u{1b}[0m\n\
             Browser cleanup timed out after 5 minutes\n\
             Orphaned container stubs: 2 could not be removed\n",
        );

        assert_eq!(execution.system_scope, CleanupSystemScope::UserOnly);
        assert_eq!(execution.completion, CleanupCompletion::Partial);
        assert_eq!(execution.warnings.len(), 3);
        assert!(execution.warnings[0].contains("administrator authorization"));
        assert!(execution.warnings[1].contains("timed out"));
        assert!(execution.warnings[2].contains("could not be removed"));
        assert!(execution
            .warnings
            .iter()
            .all(|warning| !warning.contains('\u{1b}')));
    }

    #[test]
    fn cleanup_execution_rejects_a_backend_changed_since_review() {
        let error = execution_binary(
            Detection::Found {
                path: PathBuf::from("/fixtures/mo"),
                version: "1.49.0".to_string(),
            },
            "1.48.1",
        )
        .expect_err("changed backend needs a new preview");

        assert!(matches!(error, AdapterError::BackendVersionChanged { .. }));
    }

    /// A cancelled cleanup is a run that happened, not an error. Mole may have
    /// removed files before it was killed, so the caller gets a result to
    /// journal rather than an `AdapterError::Cancelled` that implies nothing did.
    #[test]
    #[cfg(unix)]
    fn cancelling_cleanup_execution_kills_the_subprocess_and_reports_the_run() {
        use std::thread;
        use std::time::Duration;

        let script = blocking_script();
        let cancel = CancelToken::new();
        let worker_cancel = cancel.clone();
        let worker_script = script.clone();
        let worker = thread::spawn(move || execute_cleanup_from(&worker_script, &worker_cancel));
        thread::sleep(Duration::from_millis(100));
        cancel.cancel();

        let execution = worker
            .join()
            .unwrap()
            .expect("a stopped run still happened");
        let _ = std::fs::remove_file(script);
        assert_eq!(execution.completion, CleanupCompletion::Cancelled);
        assert!(execution
            .warnings
            .iter()
            .any(|warning| warning.contains("stays removed")));
    }

    /// Nothing that can go wrong after the spawn may be raised as an error.
    ///
    /// A failed wait and unreadable output both leave the removals unknowable,
    /// and an `Err` would report them as a run that never happened.
    #[test]
    fn post_spawn_failures_are_outcomes_rather_than_errors() {
        let stdout = b"System-level cleanup enabled, sudo session active\n";

        let unwaitable = execution_of(stdout, Err(std::io::Error::other("waitpid failed")), Ok(()));
        assert_eq!(unwaitable.completion, CleanupCompletion::Failed);
        assert_eq!(unwaitable.system_scope, CleanupSystemScope::Included);
        assert!(unwaitable
            .warnings
            .iter()
            .any(|warning| warning.contains("exit status could not be read")));

        let unreadable = execution_of(
            b"",
            Ok(nirmoka_adapter::process::Outcome {
                status: successful_status(),
                stderr: String::new(),
                cancelled: false,
            }),
            Err(AdapterError::OperationFailed {
                backend: "mole",
                operation: "cleanup execution",
                reason: "backend stdout was unavailable".to_string(),
            }),
        );
        assert_eq!(unreadable.completion, CleanupCompletion::Failed);
        assert!(unreadable
            .warnings
            .iter()
            .any(|warning| warning.contains("output could not be read")));
    }

    #[cfg(unix)]
    fn successful_status() -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    #[cfg(windows)]
    fn successful_status() -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(0)
    }

    /// Same reasoning for a backend that dies part way through.
    #[test]
    #[cfg(unix)]
    fn a_failed_cleanup_run_is_reported_rather_than_raised() {
        let script = executable_script(
            r#"#!/bin/sh
printf '%s\n' 'System-level cleanup enabled, sudo session active'
printf '%s\n' 'disk full' >&2
exit 3
"#,
        );

        let execution =
            execute_cleanup_from(&script, &CancelToken::new()).expect("a failed run happened");
        let _ = std::fs::remove_file(script);

        assert_eq!(execution.completion, CleanupCompletion::Failed);
        assert_eq!(execution.system_scope, CleanupSystemScope::Included);
        assert!(execution
            .warnings
            .iter()
            .any(|warning| warning.contains("status 3") && warning.contains("disk full")));
    }

    #[test]
    #[cfg(unix)]
    fn cancelling_cleanup_preview_kills_the_subprocess() {
        use std::thread;
        use std::time::Duration;

        let script = blocking_script();
        let preview_path = script.with_extension("preview");
        let cancel = CancelToken::new();
        let worker_cancel = cancel.clone();
        let worker_script = script.clone();
        let worker_preview = preview_path.clone();
        let worker = thread::spawn(move || {
            cleanup_preview_from(&worker_script, &worker_preview, "1.48.1", &worker_cancel)
        });
        thread::sleep(Duration::from_millis(100));
        cancel.cancel();

        let error = worker.join().unwrap().expect_err("preview was cancelled");
        let _ = std::fs::remove_file(script);
        assert!(matches!(error, AdapterError::Cancelled { .. }), "{error}");
    }

    #[test]
    #[cfg(unix)]
    fn cancelling_status_kills_the_subprocess() {
        use std::thread;
        use std::time::Duration;

        let script = blocking_script();

        let cancel = CancelToken::new();
        let worker_cancel = cancel.clone();
        let worker_script = script.clone();
        let worker = thread::spawn(move || status_from(&worker_script, &worker_cancel));
        thread::sleep(Duration::from_millis(100));
        cancel.cancel();

        let error = worker.join().unwrap().expect_err("status was cancelled");
        let _ = std::fs::remove_file(script);
        assert!(matches!(error, AdapterError::Cancelled { .. }), "{error}");
    }

    /// Mole is macOS-only upstream, so a `mo` found anywhere else is a
    /// different program with the same name.
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn nothing_is_detected_off_macos() {
        assert!(matches!(
            MoleAdapter::new().detect(),
            Ok(Detection::NotInstalled)
        ));
    }
}
