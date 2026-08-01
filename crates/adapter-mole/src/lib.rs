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

use nirmoka_adapter::process::{find_in_path, RunningProcess};
use nirmoka_adapter::{
    Adapter, AdapterError, CancelToken, Capabilities, Detection, ScanOptions, ScanSummary,
    SystemStatus, WireSink,
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
            uninstall_apps: true,
            // `mo status --json`.
            system_status: true,
        }
    }

    fn system_status(&self, cancel: &CancelToken) -> Result<SystemStatus, AdapterError> {
        let binary = match self.detect()? {
            Detection::Found { path, .. } => path,
            Detection::UnsupportedVersion { version, .. } => {
                return Err(AdapterError::UnsupportedVersion {
                    binary: BINARY,
                    version,
                    supported: SUPPORTED,
                })
            }
            Detection::NotInstalled => return Err(AdapterError::NotInstalled { binary: BINARY }),
        };

        status_from(&binary, cancel)
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

fn status_from(binary: &Path, cancel: &CancelToken) -> Result<SystemStatus, AdapterError> {
    let mut command = Command::new(binary);
    command.args(["status", "--json"]);

    let mut process =
        RunningProcess::spawn(&mut command, cancel).map_err(|source| AdapterError::Spawn {
            binary: BINARY,
            source,
        })?;
    let parsed = process
        .take_stdout()
        .ok_or_else(|| AdapterError::OperationFailed {
            backend: "mole",
            operation: "system status",
            reason: "backend stdout was unavailable".to_string(),
        })
        .and_then(|stdout| {
            serde_json::from_reader(stdout).map_err(|source| AdapterError::MalformedBackendOutput {
                binary: BINARY,
                operation: "system status",
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
            operation: "system status",
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

    const STATUS: &str = include_str!("../../../fixtures/mole/1.48.1/status.json");

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
        assert!(caps.dry_run && caps.cleanup_categories && caps.uninstall_apps);
        assert!(caps.system_status);
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
    fn malformed_status_is_rejected() {
        let error = serde_json::from_str::<SystemStatus>(r#"{"health_score":"healthy"}"#)
            .expect_err("wrong types must fail");

        assert!(error.to_string().contains("invalid type"));
    }

    #[test]
    #[cfg(unix)]
    fn cancelling_status_kills_the_subprocess() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::thread;
        use std::time::{Duration, SystemTime};

        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let script = std::env::temp_dir().join(format!("nirmoka-mole-status-{unique}.sh"));
        fs::write(&script, "#!/bin/sh\nsleep 60\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();

        let cancel = CancelToken::new();
        let worker_cancel = cancel.clone();
        let worker_script = script.clone();
        let worker = thread::spawn(move || status_from(&worker_script, &worker_cancel));
        thread::sleep(Duration::from_millis(100));
        cancel.cancel();

        let error = worker.join().unwrap().expect_err("status was cancelled");
        let _ = fs::remove_file(script);
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
