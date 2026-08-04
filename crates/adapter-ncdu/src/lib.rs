//! The ncdu adapter — Nirmoka's baseline backend.
//!
//! ncdu is built first and deliberately: it is the *narrowest* backend, so
//! developing against it keeps the interface honest. Mole produces far more
//! information, and designing around Mole's output would leave the ncdu path
//! impossible to implement — a Mole GUI with a backend-shaped hole in it.
//!
//! It is also cross-platform, which means platform-neutral code can be written
//! and tested on a single machine.
//!
//! # Version support
//!
//! Only ncdu 2.x. The JSON export format used as Nirmoka's wire format
//! (`ncdu -o -`) is the version 2 shape; ncdu 1.x emits a different one, and a
//! hypothetical 3.x is unknown until someone tests it.
//!
//! The gate is enforced on *every* operation, not only in the backend picker.
//! ncdu 1.x emits the same JSON format *major* version as 2.x, so its export
//! parses — an unversioned scan would silently produce plausible numbers from
//! an untested backend.

#![forbid(unsafe_code)]

use std::io::BufReader;
use std::path::{Path, PathBuf};

use nirmoka_adapter::process::{self, find_in_path, RunningProcess};
use nirmoka_adapter::wire;
use nirmoka_adapter::{
    validate_scan_root, Adapter, AdapterError, CancelToken, Capabilities, Detection, ScanOptions,
    ScanSummary, WireSink,
};

const BINARY: &str = "ncdu";
const SUPPORTED: &str = ">=2.0, <3.0";

/// 64 KiB. An export of a large home directory is tens of megabytes arriving
/// through a pipe; reading it in default-sized chunks is measurable overhead.
const READ_BUFFER: usize = 64 * 1024;

#[derive(Debug, Default, Clone, Copy)]
pub struct NcduAdapter;

impl NcduAdapter {
    pub fn new() -> Self {
        Self
    }

    /// The binary path and version, or an error explaining which gate failed.
    ///
    /// Every operation goes through this. Detection is not a one-time startup
    /// check: a user can upgrade ncdu while the app is open.
    fn usable_binary(&self) -> Result<(PathBuf, String), AdapterError> {
        match self.detect()? {
            Detection::Found { path, version } => Ok((path, version)),
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
}

impl Adapter for NcduAdapter {
    fn id(&self) -> &'static str {
        "ncdu"
    }

    fn display_name(&self) -> &'static str {
        "ncdu"
    }

    fn supported_versions(&self) -> &'static str {
        SUPPORTED
    }

    fn detect(&self) -> Result<Detection, AdapterError> {
        // Resolved first so the reported path is the binary that will actually
        // run. On a machine with ncdu in both /usr/bin and /opt/homebrew/bin,
        // "ncdu" is not an answer the user can act on.
        let resolved = find_in_path(BINARY);
        let program = resolved.clone().unwrap_or_else(|| PathBuf::from(BINARY));

        let output = match process::command(&program).arg("--version").output() {
            Ok(output) => output,
            // A missing binary is a normal state, not an error. Anything else
            // (permission denied, exec format error) is worth surfacing.
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
        // ncdu can delete only from its interactive ncurses browser. Export
        // mode has no command for removing a selected path, so claiming that
        // ability here would leave the adapter with no safe implementation.
        Capabilities::MINIMAL
    }

    fn scan(
        &self,
        root: &Path,
        options: &ScanOptions,
        sink: &mut dyn WireSink,
        cancel: &CancelToken,
    ) -> Result<ScanSummary, AdapterError> {
        let root = validate_scan_root(root)?;
        let (binary, version) = self.usable_binary()?;

        if cancel.is_cancelled() {
            return Err(cancelled());
        }

        let mut command = process::command(binary);
        command
            // Without this, a user's ~/.config/ncdu/config silently changes
            // what a scan means — including turning on apparent-size mode or
            // adding exclude patterns Nirmoka never asked for.
            .arg("--ignore-config")
            // No progress UI. `-o -` still draws one otherwise, into a terminal
            // that is not there.
            .arg("-0")
            .arg("-o")
            .arg("-");

        if options.one_file_system {
            command.arg("-x");
        }
        if options.exclude_caches {
            command.arg("--exclude-caches");
        }
        for pattern in &options.exclude {
            command.arg("--exclude").arg(pattern);
        }

        command.arg(&root);

        let mut process =
            RunningProcess::spawn(&mut command, cancel).map_err(|source| AdapterError::Spawn {
                binary: BINARY,
                source,
            })?;

        let stdout = process
            .take_stdout()
            .expect("RunningProcess::spawn pipes stdout");

        // Parsing happens while ncdu is still walking the disk. The sink sees
        // entries during the scan, which is what invariant 5 needs on the other
        // side of the boundary.
        let parsed = wire::parse(BufReader::with_capacity(READ_BUFFER, stdout), sink);

        let outcome = process.finish().map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;

        // Order matters. A cancelled scan leaves a truncated export and a
        // killed process; reporting either of those as the failure would blame
        // the backend for something the user did.
        if outcome.cancelled {
            return Err(cancelled());
        }

        if !outcome.status.success() {
            return Err(AdapterError::BackendFailed {
                binary: BINARY,
                status: outcome.status.code().unwrap_or(-1),
                stderr: outcome.stderr,
            });
        }

        let stats = parsed.map_err(|source| AdapterError::MalformedOutput {
            binary: BINARY,
            source,
        })?;

        Ok(ScanSummary {
            root,
            items: stats.items,
            directories: stats.directories,
            backend_version: Some(version),
        })
    }
}

fn cancelled() -> AdapterError {
    AdapterError::Cancelled {
        backend: "ncdu",
        operation: "scan",
    }
}

/// Pull a version out of `ncdu --version` output.
///
/// Observed shape on ncdu 2.8.2 (Homebrew, macOS): `ncdu 2.8.2`.
fn parse_version(output: &str) -> Option<String> {
    let line = output.lines().next()?.trim();
    let token = line.split_whitespace().nth(1)?;

    // Reject anything that is not digit-led, so error text like
    // "ncdu: command failed" cannot be mistaken for a version.
    if token.starts_with(|c: char| c.is_ascii_digit()) {
        Some(token.to_string())
    } else {
        None
    }
}

/// Accept 2.x only.
fn is_supported(version: &str) -> bool {
    matches!(major_of(version), Some(2))
}

fn major_of(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_observed_homebrew_output() {
        assert_eq!(parse_version("ncdu 2.8.2\n").as_deref(), Some("2.8.2"));
    }

    #[test]
    fn parses_a_one_x_version() {
        assert_eq!(parse_version("ncdu 1.19").as_deref(), Some("1.19"));
    }

    #[test]
    fn rejects_error_text_as_a_version() {
        // The failure mode this guards: treating stderr-ish output as data.
        assert!(parse_version("ncdu: command not found").is_none());
        assert!(parse_version("").is_none());
        assert!(parse_version("ncdu").is_none());
    }

    #[test]
    fn accepts_two_x_only() {
        assert!(is_supported("2.8.2"));
        assert!(is_supported("2.0"));
        assert!(!is_supported("1.19"));
        assert!(!is_supported("3.0.0"));
    }

    #[test]
    fn declares_no_dry_run() {
        // If this ever flips to true, there must be a real preview behind it.
        assert!(!NcduAdapter::new().capabilities().dry_run);
        assert!(!NcduAdapter::new().capabilities().trash);
    }

    #[test]
    fn declares_scan_but_not_scriptable_deletion() {
        let caps = NcduAdapter::new().capabilities();
        assert!(caps.scan);
        assert!(!caps.delete);
    }

    #[test]
    fn a_bad_scan_root_is_refused_before_anything_is_spawned() {
        // Runs on every platform, including the ones with no ncdu: validation
        // happens before the backend is consulted.
        let mut sink = nirmoka_adapter::TreeSink::new();
        let error = NcduAdapter::new()
            .scan(
                Path::new("nirmoka-no-such-directory"),
                &ScanOptions::default(),
                &mut sink,
                &CancelToken::new(),
            )
            .unwrap_err();

        assert!(matches!(error, AdapterError::RefusedPath { .. }));
    }
}
