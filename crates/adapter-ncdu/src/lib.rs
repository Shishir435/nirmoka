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

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::Command;

use nirmoka_adapter::{Adapter, AdapterError, Capabilities, Detection};

const BINARY: &str = "ncdu";
const SUPPORTED: &str = ">=2.0, <3.0";

#[derive(Debug, Default, Clone, Copy)]
pub struct NcduAdapter;

impl NcduAdapter {
    pub fn new() -> Self {
        Self
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
        let output = match Command::new(BINARY).arg("--version").output() {
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

        // TODO(step 3): resolve the absolute path instead of reporting the
        // command name. Needs a cross-platform PATH search; not worth a
        // dependency until the UI displays it.
        let path = PathBuf::from(BINARY);

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
        // ncdu browses and deletes. It has no dry-run mode and no Trash
        // routing, and the adapter must not pretend otherwise — the UI falls
        // back to an explicit confirmation instead of a faked preview.
        Capabilities::MINIMAL
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
    fn declares_scan_and_delete() {
        let caps = NcduAdapter::new().capabilities();
        assert!(caps.scan);
        assert!(caps.delete);
    }
}
