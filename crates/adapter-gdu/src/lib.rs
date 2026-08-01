//! The gdu adapter — the cross-platform scanner and primary Windows backend.
//!
//! gdu 5.32 emits ncdu JSON export format 1.2 directly, so this adapter shares
//! the parser and tree sink in `nirmoka-adapter`. It does not translate or
//! buffer the export.
//!
//! # Version support
//!
//! Only the 5.32 release line is accepted. The recorded fixture and live
//! command evidence come from 5.32.0; another minor release joins the supported
//! range only after its output has been recorded and tested.

#![forbid(unsafe_code)]

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::process::Command;

use nirmoka_adapter::process::{find_in_path, RunningProcess};
use nirmoka_adapter::wire;
use nirmoka_adapter::{
    validate_scan_root, Adapter, AdapterError, CancelToken, Capabilities, Detection, ScanOptions,
    ScanSummary, WireSink,
};

const BINARY: &str = "gdu";
const SUPPORTED: &str = ">=5.32, <5.33";
const READ_BUFFER: usize = 64 * 1024;

#[derive(Debug, Default, Clone, Copy)]
pub struct GduAdapter;

impl GduAdapter {
    pub fn new() -> Self {
        Self
    }

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

impl Adapter for GduAdapter {
    fn id(&self) -> &'static str {
        "gdu"
    }

    fn display_name(&self) -> &'static str {
        "gdu"
    }

    fn supported_versions(&self) -> &'static str {
        SUPPORTED
    }

    fn detect(&self) -> Result<Detection, AdapterError> {
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
        // `d` deletes inside gdu's terminal browser. There is no command that
        // removes a caller-selected path, so selected-path deletion remains
        // false for the same reason as ncdu. See ADR 0014.
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

        // gdu 5.32 has no CACHEDIR.TAG option and its ignore patterns are Go
        // regular expressions, not the glob syntax promised by ScanOptions.
        // Refuse instead of silently changing what the caller asked to scan.
        if options.exclude_caches || !options.exclude.is_empty() {
            return Err(AdapterError::Unsupported {
                backend: BINARY,
                operation: "scan exclusions",
            });
        }

        let (binary, version) = self.usable_binary()?;
        if cancel.is_cancelled() {
            return Err(cancelled());
        }

        let mut command = Command::new(binary);
        command
            // Do not let ~/.gdu.yaml change the meaning of a scan. The null
            // device is a valid empty config on each supported platform.
            .arg("--config-file")
            .arg(empty_config())
            .arg("--no-progress")
            .arg("-o")
            .arg("-");

        if options.one_file_system {
            command.arg("--no-cross");
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
        let parsed = wire::parse(BufReader::with_capacity(READ_BUFFER, stdout), sink);

        let outcome = process.finish().map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;

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

#[cfg(windows)]
fn empty_config() -> &'static str {
    "NUL"
}

#[cfg(not(windows))]
fn empty_config() -> &'static str {
    "/dev/null"
}

fn cancelled() -> AdapterError {
    AdapterError::Cancelled {
        backend: BINARY,
        operation: "scan",
    }
}

/// Observed 5.32.0 output starts with `Version:\t v5.32.0`.
fn parse_version(output: &str) -> Option<String> {
    let line = output.lines().next()?.trim();
    let token = line.split_whitespace().nth(1)?;
    let version = token.strip_prefix('v').unwrap_or(token);

    version
        .starts_with(|c: char| c.is_ascii_digit())
        .then(|| version.to_string())
}

fn is_supported(version: &str) -> bool {
    matches!(major_minor_of(version), Some((5, 32)))
}

fn major_minor_of(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_recorded_version_output() {
        let output = "Version:\t v5.32.0\nBuilt time:\t Sat Nov 22 12:00:38 PM CET 2025\n";
        assert_eq!(parse_version(output).as_deref(), Some("5.32.0"));
    }

    #[test]
    fn parses_versions_with_or_without_the_v_prefix() {
        assert_eq!(parse_version("Version: v5.32.1").as_deref(), Some("5.32.1"));
        assert_eq!(parse_version("Version: 5.32.1").as_deref(), Some("5.32.1"));
    }

    #[test]
    fn rejects_error_text_as_a_version() {
        assert!(parse_version("gdu: command failed").is_none());
        assert!(parse_version("").is_none());
        assert!(parse_version("Version:").is_none());
    }

    #[test]
    fn gates_to_the_recorded_minor_release() {
        assert!(is_supported("5.32.0"));
        assert!(is_supported("5.32.9"));
        assert!(!is_supported("5.31.0"));
        assert!(!is_supported("5.33.0"));
        assert!(!is_supported("6.0.0"));
    }

    #[test]
    fn declares_scan_without_scriptable_deletion() {
        let caps = GduAdapter::new().capabilities();
        assert!(caps.scan);
        assert!(!caps.delete);
        assert!(!caps.trash);
        assert!(!caps.dry_run);
    }

    #[test]
    fn validates_the_root_before_looking_for_gdu() {
        let mut sink = nirmoka_adapter::TreeSink::new();
        let error = GduAdapter::new()
            .scan(
                Path::new("nirmoka-gdu-no-such-directory"),
                &ScanOptions::default(),
                &mut sink,
                &CancelToken::new(),
            )
            .unwrap_err();

        assert!(matches!(error, AdapterError::RefusedPath { .. }));
    }

    #[test]
    fn refuses_scan_options_it_cannot_translate_honestly() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let mut sink = nirmoka_adapter::TreeSink::new();

        for options in [
            ScanOptions {
                exclude_caches: true,
                ..ScanOptions::default()
            },
            ScanOptions {
                exclude: vec!["target".to_string()],
                ..ScanOptions::default()
            },
        ] {
            let error = GduAdapter::new()
                .scan(root, &options, &mut sink, &CancelToken::new())
                .unwrap_err();
            assert!(matches!(error, AdapterError::Unsupported { .. }));
        }
    }
}
