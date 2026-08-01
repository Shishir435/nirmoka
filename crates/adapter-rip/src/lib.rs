//! Recoverable selected-path deletion through `rip` (rm-improved).

#![forbid(unsafe_code)]

use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use directories::ProjectDirs;
use nirmoka_adapter::process::{find_in_path, RunningProcess};
use nirmoka_adapter::{
    validate_delete_target, Adapter, AdapterError, CancelToken, Capabilities, DeleteMode,
    DeletePlan, DeleteReceipt, Detection, ScanOptions, ScanSummary, WireSink,
};

const BINARY: &str = "rip";
const SUPPORTED: &str = ">=0.13, <0.14";

#[derive(Debug)]
pub struct RipAdapter {
    binary: Option<PathBuf>,
    recovery_root: Option<PathBuf>,
    next_operation: AtomicU64,
}

impl RipAdapter {
    pub fn new() -> Self {
        Self {
            binary: None,
            recovery_root: ProjectDirs::from("app", "nirmoka", "Nirmoka")
                .map(|dirs| dirs.data_local_dir().join("recoverable-delete")),
            next_operation: AtomicU64::new(0),
        }
    }

    /// Construct against explicit paths. Used by the destructive contract
    /// tests so they never touch a user's real graveyard or PATH.
    #[doc(hidden)]
    pub fn with_binary_and_recovery_root(binary: PathBuf, recovery_root: PathBuf) -> Self {
        Self {
            binary: Some(binary),
            recovery_root: Some(recovery_root),
            next_operation: AtomicU64::new(0),
        }
    }

    fn resolved_binary(&self) -> Option<PathBuf> {
        self.binary.clone().or_else(|| find_in_path(BINARY))
    }

    fn usable_binary(&self) -> Result<PathBuf, AdapterError> {
        match self.detect()? {
            Detection::Found { path, .. } => Ok(path),
            Detection::UnsupportedVersion { version, .. } => {
                Err(AdapterError::UnsupportedVersion {
                    binary: BINARY,
                    version,
                    supported: SUPPORTED,
                })
            }
            Detection::NotInstalled => Err(AdapterError::NotInstalled { binary: BINARY }),
        }
    }

    fn operation_root(&self) -> Result<PathBuf, AdapterError> {
        let base = self.recovery_root.as_ref().ok_or_else(|| {
            failed(
                "delete",
                "this system has no application data directory for recoverable deletion",
            )
        })?;
        fs::create_dir_all(base).map_err(|error| {
            failed(
                "delete",
                format!("could not create {}: {error}", base.display()),
            )
        })?;

        let epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let sequence = self.next_operation.fetch_add(1, Ordering::Relaxed);
        let root = base.join(format!("{epoch}-{}-{sequence}", std::process::id()));
        fs::create_dir(&root).map_err(|error| {
            failed(
                "delete",
                format!("could not create {}: {error}", root.display()),
            )
        })?;
        Ok(root)
    }
}

impl Default for RipAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Adapter for RipAdapter {
    fn id(&self) -> &'static str {
        "rip"
    }

    fn display_name(&self) -> &'static str {
        "rip"
    }

    fn supported_versions(&self) -> &'static str {
        SUPPORTED
    }

    fn detect(&self) -> Result<Detection, AdapterError> {
        let Some(program) = self.resolved_binary() else {
            return Ok(Detection::NotInstalled);
        };

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

        let path = program.canonicalize().unwrap_or(program);
        if is_supported(&version) {
            Ok(Detection::Found { path, version })
        } else {
            Ok(Detection::UnsupportedVersion {
                path,
                version,
                supported: SUPPORTED.to_string(),
            })
        }
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            scan: false,
            delete: true,
            trash: true,
            undo: true,
            dry_run: false,
            cleanup_categories: false,
            uninstall_apps: false,
            system_status: false,
        }
    }

    fn scan(
        &self,
        _root: &Path,
        _options: &ScanOptions,
        _sink: &mut dyn WireSink,
        _cancel: &CancelToken,
    ) -> Result<ScanSummary, AdapterError> {
        Err(AdapterError::Unsupported {
            backend: "rip",
            operation: "scan",
        })
    }

    fn prepare_delete(
        &self,
        scan_root: &Path,
        target: &Path,
        mode: DeleteMode,
    ) -> Result<DeletePlan, AdapterError> {
        if mode != DeleteMode::Trash {
            return Err(AdapterError::Unsupported {
                backend: BINARY,
                operation: "permanent selected-path deletion",
            });
        }

        let scan_root = scan_root.canonicalize().map_err(|error| {
            refused(
                scan_root,
                format!("could not canonicalise scan root: {error}"),
            )
        })?;
        let target = validate_delete_target(&scan_root, target)?;
        Ok(DeletePlan::new(BINARY, scan_root, target, mode))
    }

    fn delete(
        &self,
        plan: &DeletePlan,
        cancel: &CancelToken,
    ) -> Result<DeleteReceipt, AdapterError> {
        if plan.backend() != BINARY || plan.mode() != DeleteMode::Trash {
            return Err(AdapterError::Unsupported {
                backend: BINARY,
                operation: "requested deletion plan",
            });
        }

        // Validation happens again at the final boundary. A confirmation may
        // sit open while the filesystem changes underneath it.
        let target = validate_delete_target(plan.scan_root(), plan.target())?;
        if target != plan.target() {
            return Err(refused(plan.target(), "target changed after confirmation"));
        }

        let binary = self.usable_binary()?;
        if cancel.is_cancelled() {
            return Err(cancelled("delete"));
        }

        let recovery_root = self.operation_root()?;
        let recovery_path = recovery_root.join(relative_storage_path(&target));
        let mut command = Command::new(binary);
        command.arg("--graveyard").arg(&recovery_root).arg(&target);

        let outcome = run(&mut command, cancel, "delete")?;
        let moved = !target.exists() && recovery_path.exists();
        // Filesystem state wins over the process status. Cancellation can race
        // with rip's final exit after the move; reporting Cancelled then would
        // make the caller believe nothing happened and lose the undo receipt.
        if moved {
            return Ok(DeleteReceipt::new(
                BINARY,
                target,
                recovery_root,
                recovery_path,
            ));
        }
        cleanup_empty_recovery(&recovery_root);
        if outcome.cancelled {
            return Err(cancelled("delete"));
        }
        if !outcome.success {
            return Err(AdapterError::BackendFailed {
                binary: BINARY,
                status: outcome.status,
                stderr: outcome.stderr,
            });
        }
        Err(failed(
            "delete",
            format!(
                "backend succeeded but {} was not recorded at {}",
                target.display(),
                recovery_path.display()
            ),
        ))
    }

    fn undo(&self, receipt: &DeleteReceipt, cancel: &CancelToken) -> Result<(), AdapterError> {
        if receipt.backend() != BINARY {
            return Err(AdapterError::Unsupported {
                backend: BINARY,
                operation: "receipt from another backend",
            });
        }
        if receipt.target().exists() {
            return Err(refused(
                receipt.target(),
                "cannot restore over a path that now exists",
            ));
        }

        let configured = self
            .recovery_root
            .as_ref()
            .ok_or_else(|| failed("undo deletion", "this system has no recovery directory"))?;
        let configured = configured.canonicalize().map_err(|error| {
            failed(
                "undo deletion",
                format!("could not open {}: {error}", configured.display()),
            )
        })?;
        let canonical_recovery_root = receipt.recovery_root().canonicalize().map_err(|error| {
            failed(
                "undo deletion",
                format!(
                    "could not open recovery receipt {}: {error}",
                    receipt.recovery_root().display()
                ),
            )
        })?;
        let canonical_recovery_path = receipt.recovery_path().canonicalize().map_err(|error| {
            failed(
                "undo deletion",
                format!(
                    "could not open recovery item {}: {error}",
                    receipt.recovery_path().display()
                ),
            )
        })?;
        if !canonical_recovery_root.starts_with(&configured)
            || canonical_recovery_root == configured
            || !canonical_recovery_path.starts_with(&canonical_recovery_root)
            || canonical_recovery_path == canonical_recovery_root
        {
            return Err(refused(
                receipt.recovery_path(),
                "recovery receipt is outside Nirmoka's recovery directory",
            ));
        }

        let binary = self.usable_binary()?;
        if cancel.is_cancelled() {
            return Err(cancelled("undo deletion"));
        }
        let mut command = Command::new(binary);
        command
            .arg("--graveyard")
            // Keep the spelling recorded by rip. On macOS `/tmp` canonicalises
            // to `/private/tmp`; rip compares the argument to its record as a
            // path string, so canonicalising only the command argument makes a
            // valid receipt look unrelated and produces a no-op success.
            .arg(receipt.recovery_root())
            .arg("--unbury")
            .arg(receipt.recovery_path());

        let outcome = run(&mut command, cancel, "undo deletion")?;
        let restored = receipt.target().exists() && !receipt.recovery_path().exists();
        // Same race as deletion: if restore completed, returning cancellation
        // would leave the journal claiming the item was still deleted.
        if restored {
            return Ok(());
        }
        if outcome.cancelled {
            return Err(cancelled("undo deletion"));
        }
        if !outcome.success {
            return Err(AdapterError::BackendFailed {
                binary: BINARY,
                status: outcome.status,
                stderr: outcome.stderr,
            });
        }
        Err(failed(
            "undo deletion",
            "backend succeeded but the original path was not restored",
        ))
    }
}

struct CommandOutcome {
    success: bool,
    status: i32,
    stderr: String,
    cancelled: bool,
}

fn run(
    command: &mut Command,
    cancel: &CancelToken,
    operation: &'static str,
) -> Result<CommandOutcome, AdapterError> {
    let mut process =
        RunningProcess::spawn(command, cancel).map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;
    let mut stdout = String::new();
    if let Some(mut pipe) = process.take_stdout() {
        pipe.read_to_string(&mut stdout).map_err(|error| {
            failed(operation, format!("could not read backend output: {error}"))
        })?;
    }
    let outcome = process.finish().map_err(|source| AdapterError::Spawn {
        binary: BINARY,
        source,
    })?;
    Ok(CommandOutcome {
        success: outcome.status.success(),
        status: outcome.status.code().unwrap_or(-1),
        stderr: if outcome.stderr.is_empty() {
            stdout.trim().to_string()
        } else {
            outcome.stderr
        },
        cancelled: outcome.cancelled,
    })
}

fn relative_storage_path(path: &Path) -> PathBuf {
    let mut relative = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => {
                let drive = prefix
                    .as_os_str()
                    .to_string_lossy()
                    .replace([':', '\\', '/'], "_");
                relative.push(drive);
            }
            Component::Normal(segment) => relative.push(segment),
            Component::RootDir | Component::CurDir | Component::ParentDir => {}
        }
    }
    relative
}

fn cleanup_empty_recovery(path: &Path) {
    // Only the unique directory created for this attempt is targeted. Ignore a
    // failure: cleanup must never replace the backend error the user needs.
    let _ = fs::remove_dir(path);
}

fn parse_version(output: &str) -> Option<String> {
    let line = output.lines().next()?.trim();
    let token = line.split_whitespace().last()?;
    let version = token.strip_prefix('v').unwrap_or(token);
    version
        .split('.')
        .all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
        .then(|| version.to_string())
}

fn is_supported(version: &str) -> bool {
    let mut parts = version.split('.');
    matches!((parts.next(), parts.next()), (Some("0"), Some("13")))
}

fn refused(path: &Path, reason: impl Into<String>) -> AdapterError {
    AdapterError::RefusedPath {
        path: path.display().to_string(),
        reason: reason.into(),
    }
}

fn failed(operation: &'static str, reason: impl Into<String>) -> AdapterError {
    AdapterError::OperationFailed {
        backend: BINARY,
        operation,
        reason: reason.into(),
    }
}

fn cancelled(operation: &'static str) -> AdapterError {
    AdapterError::Cancelled {
        backend: BINARY,
        operation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_observed_version_shape() {
        assert_eq!(parse_version("rm-improved 0.13.1\n"), Some("0.13.1".into()));
        assert_eq!(parse_version("rip v0.13.1\n"), Some("0.13.1".into()));
        assert_eq!(parse_version("error: 0.13.1 is broken\n"), None);
    }

    #[test]
    fn gates_to_the_recorded_minor_line() {
        assert!(is_supported("0.13.0"));
        assert!(is_supported("0.13.99"));
        assert!(!is_supported("0.12.0"));
        assert!(!is_supported("0.14.0"));
    }
}
