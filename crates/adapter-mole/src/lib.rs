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
    Adapter, AdapterError, CancelToken, Capabilities, CleanupCategory, CleanupItem, CleanupPreview,
    CleanupSystemScope, Detection, InstalledApplication, ScanOptions, ScanSummary, SystemStatus,
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
        let binary = supported_binary(self.detect()?)?;
        status_from(&binary, cancel)
    }

    fn installed_applications(
        &self,
        cancel: &CancelToken,
    ) -> Result<Vec<InstalledApplication>, AdapterError> {
        let binary = supported_binary(self.detect()?)?;
        applications_from(&binary, cancel)
    }

    fn cleanup_preview(&self, cancel: &CancelToken) -> Result<CleanupPreview, AdapterError> {
        let binary = supported_binary(self.detect()?)?;
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
            cancel,
        )
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

fn supported_binary(detection: Detection) -> Result<PathBuf, AdapterError> {
    match detection {
        Detection::Found { path, .. } => Ok(path),
        Detection::UnsupportedVersion { version, .. } => Err(AdapterError::UnsupportedVersion {
            binary: BINARY,
            version,
            supported: SUPPORTED,
        }),
        Detection::NotInstalled => Err(AdapterError::NotInstalled { binary: BINARY }),
    }
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
    parse_cleanup_preview(&contents, scope)
}

fn parse_cleanup_preview(
    contents: &str,
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

    for line in lines {
        if line.is_empty() {
            continue;
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
        if let Some(value) = line.strip_prefix("# Potential cleanup: ") {
            potential_cleanup = Some(value.to_string());
            continue;
        }
        if let Some(value) = line.strip_prefix("# Items: ") {
            declared_items = Some(
                value
                    .parse::<u64>()
                    .map_err(|_| malformed(format!("invalid item count: {value}")))?,
            );
            continue;
        }
        if let Some(value) = line.strip_prefix("# Categories: ") {
            declared_categories = Some(
                value
                    .parse::<usize>()
                    .map_err(|_| malformed(format!("invalid category count: {value}")))?,
            );
            continue;
        }
        if line.starts_with('#') {
            continue;
        }

        let category = current_category
            .ok_or_else(|| malformed("cleanup path appears before a category".to_string()))?;
        let (path, detail) = line
            .rsplit_once("  # ")
            .ok_or_else(|| malformed(format!("cleanup row has no size marker: {line}")))?;
        if path.is_empty() {
            return Err(malformed("cleanup row has an empty path".to_string()));
        }
        let (size, item_count) = parse_cleanup_item_detail(detail).map_err(malformed)?;
        categories[category].items.push(CleanupItem {
            path: PathBuf::from(path),
            reported_size: size,
            item_count,
        });
    }

    let total_items = categories
        .iter()
        .flat_map(|category| &category.items)
        .try_fold(0_u64, |total, item| total.checked_add(item.item_count))
        .ok_or_else(|| malformed("cleanup item count overflowed".to_string()))?;
    if !categories.is_empty() && potential_cleanup.is_none() {
        return Err(malformed(
            "nonempty preview has no potential-cleanup summary".to_string(),
        ));
    }
    let declared_items = match declared_items {
        Some(declared) => declared,
        None if categories.is_empty() => total_items,
        None => {
            return Err(malformed(
                "nonempty preview has no item-count summary".to_string(),
            ))
        }
    };
    let declared_categories = match declared_categories {
        Some(declared) => declared,
        None if categories.is_empty() => categories.len(),
        None => {
            return Err(malformed(
                "nonempty preview has no category-count summary".to_string(),
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
        generated_at,
        categories,
        potential_cleanup,
        total_items,
        system_scope,
        warnings,
    })
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
    fn executable_script(contents: &str) -> PathBuf {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use std::time::SystemTime;

        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let script = std::env::temp_dir().join(format!("nirmoka-mole-test-{unique}.sh"));
        fs::write(&script, contents).unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        script
    }

    const STATUS: &str = include_str!("../../../fixtures/mole/1.48.1/status.json");
    const APPLICATIONS: &str = include_str!("../../../fixtures/mole/1.48.1/applications.json");
    const CLEAN_PREVIEW: &str = include_str!("../../../fixtures/mole/1.48.1/clean-list.txt");

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
        let preview = parse_cleanup_preview(CLEAN_PREVIEW, CleanupSystemScope::UserOnly)
            .expect("cleanup preview fixture");

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
        let error = parse_cleanup_preview(&changed, CleanupSystemScope::Included)
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
            let error = parse_cleanup_preview(&changed, CleanupSystemScope::Included)
                .expect_err("every nonempty preview summary declaration must be present");

            assert!(matches!(error, AdapterError::MalformedBackendOutput { .. }));
        }
    }

    #[test]
    fn cleanup_preview_accepts_an_empty_backend_plan() {
        let preview = parse_cleanup_preview(
            "# Mole Cleanup Preview - 2026-08-01 12:30:00\n#\n",
            CleanupSystemScope::Included,
        )
        .expect("empty cleanup preview");

        assert!(preview.categories.is_empty());
        assert_eq!(preview.total_items, 0);
        assert_eq!(preview.potential_cleanup, None);
    }

    #[test]
    fn cleanup_preview_rejects_unrecognized_sizes() {
        let preview = "# Mole Cleanup Preview - now\n=== Cache ===\n/tmp/cache  # lots\n";
        let error = parse_cleanup_preview(preview, CleanupSystemScope::Unknown)
            .expect_err("unrecognized size must fail");

        assert!(error.to_string().contains("invalid cleanup size"));
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

        let preview = cleanup_preview_from(&script, &preview_path, &CancelToken::new())
            .expect("cleanup preview");
        let _ = std::fs::remove_file(script);
        let _ = std::fs::remove_file(preview_path);

        assert_eq!(preview.system_scope, CleanupSystemScope::UserOnly);
        assert_eq!(preview.total_items, 6);
    }

    #[test]
    #[cfg(unix)]
    fn cancelling_cleanup_preview_kills_the_subprocess() {
        use std::thread;
        use std::time::Duration;

        let script = executable_script("#!/bin/sh\nsleep 60\n");
        let preview_path = script.with_extension("preview");
        let cancel = CancelToken::new();
        let worker_cancel = cancel.clone();
        let worker_script = script.clone();
        let worker_preview = preview_path.clone();
        let worker = thread::spawn(move || {
            cleanup_preview_from(&worker_script, &worker_preview, &worker_cancel)
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

        let script = executable_script("#!/bin/sh\nsleep 60\n");

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
