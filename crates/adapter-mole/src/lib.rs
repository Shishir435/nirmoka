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
use std::process::Command;

use directories::BaseDirs;
use nirmoka_adapter::process::{find_in_path, RunningProcess};
use nirmoka_adapter::{
    Adapter, AdapterError, CancelToken, Capabilities, CleanupCategory, CleanupCompletion,
    CleanupExecution, CleanupItem, CleanupPreview, CleanupSystemScope, Detection,
    InstalledApplication, ScanOptions, ScanSummary, SystemStatus, WireSink,
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

        let output = match Command::new(&program).arg("--version").output() {
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
            // The second headline exception. `mo uninstall <name>` — with or
            // without `--dry-run` — matches the app and then stops at
            // `Proceed with uninstallation? [y/N]`. There is no `--yes`, no
            // `--force`, and no environment override; the flag set is `--list`,
            // `--dry-run`, `--permanent`, `--whitelist`, `--debug`. Neither the
            // plan nor the removal is reachable without writing to that prompt,
            // and answering a backend's own safety prompt on its behalf is not
            // something an adapter may do. See ADR 0021, and
            // `fixtures/mole/1.48.1/uninstall-command-surface.txt`.
            uninstall_apps: false,
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
    let mut command = Command::new(binary);
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
    let mut command = Command::new(binary);
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

fn json_from_command<T: serde::de::DeserializeOwned>(
    binary: &Path,
    args: &[&str],
    operation: &'static str,
    cancel: &CancelToken,
) -> Result<T, AdapterError> {
    let mut command = Command::new(binary);
    command.args(args);

    let mut process =
        RunningProcess::spawn(&mut command, cancel).map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;
    let parsed = process
        .take_stdout()
        .ok_or_else(|| AdapterError::OperationFailed {
            backend: "mole",
            operation,
            reason: "backend stdout was unavailable".to_string(),
        })
        .and_then(|stdout| {
            serde_json::from_reader(stdout).map_err(|source| AdapterError::MalformedBackendOutput {
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
            !caps.uninstall_apps,
            "every named uninstall stops at an interactive prompt"
        );
    }

    /// The gate that makes ADR 0021 re-testable rather than remembered.
    ///
    /// If a Mole release adds a way past `Proceed with uninstallation?`, this
    /// fails and the capability should be reconsidered — which is the whole
    /// reason the command surface is recorded.
    #[test]
    fn the_recorded_uninstall_surface_offers_no_non_interactive_flag() {
        let options = UNINSTALL_SURFACE
            .split("== mo uninstall --dry-run")
            .next()
            .expect("the recorded help section");

        for flag in [
            "--yes",
            "--force",
            "--assume-yes",
            "--non-interactive",
            "-y ",
        ] {
            assert!(
                !options.contains(flag),
                "Mole now documents {flag}; uninstall may be scriptable, so revisit ADR 0021"
            );
        }
        assert!(
            UNINSTALL_SURFACE.contains("Proceed with uninstallation?"),
            "the recorded probe no longer shows the prompt this decision rests on"
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
        assert_eq!(applications[0].size, 268_435_456);
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
printf '%s' '[{"name":"Example","bundle_id":"com.example.desktop","source":"system","uninstall_name":"Example","path":"/Applications/Example.app","size":42}]'
"#,
        );
        let applications =
            applications_from(&script, &CancelToken::new()).expect("application list");
        let _ = std::fs::remove_file(script);

        assert_eq!(applications[0].uninstall_name, "Example");
        assert_eq!(applications[0].size, 42);
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
