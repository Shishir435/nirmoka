//! Scan inputs, scan results, and the path check every scan starts with.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AdapterError;

/// What to ask the backend for.
///
/// Deliberately small. Every option here has to be expressible by the
/// *narrowest* backend, so anything one backend can do and another cannot
/// belongs behind a capability flag instead.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOptions {
    /// Stay on the filesystem the root is on. Without it, scanning `/` on macOS
    /// wanders into every mounted volume and Time Machine snapshot.
    pub one_file_system: bool,

    /// Skip directories tagged with `CACHEDIR.TAG`.
    pub exclude_caches: bool,

    /// Glob patterns the backend should skip. Excluded entries still appear in
    /// the tree, flagged, with unknown size.
    pub exclude: Vec<String>,
}

/// What a completed scan produced.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanSummary {
    /// The canonical path actually scanned, which may differ from the one asked
    /// for if it was relative or went through a symlink.
    pub root: PathBuf,

    /// Entries the backend reported, including directories.
    pub items: u64,

    pub directories: u64,

    /// Backend version, so a saved scan can be traced to what produced it.
    pub backend_version: Option<String>,
}

/// Check a path before it becomes a subprocess argument.
///
/// Scanning is the harmless operation, so this is not the interesting
/// validation — [deletion](crate::Adapter) is. It exists anyway because a
/// relative path handed to a backend resolves against *the backend's* working
/// directory, and because "that directory does not exist" is a better error
/// than whatever a backend prints when it cannot open its argument.
pub fn validate_scan_root(path: &Path) -> Result<PathBuf, AdapterError> {
    let canonical = path
        .canonicalize()
        .map_err(|source| AdapterError::RefusedPath {
            path: path.display().to_string(),
            reason: format!("cannot be resolved: {source}"),
        })?;

    if !canonical.is_dir() {
        return Err(AdapterError::RefusedPath {
            path: path.display().to_string(),
            reason: "not a directory".to_string(),
        });
    }

    Ok(strip_verbatim_prefix(canonical))
}

/// `canonicalize` on Windows returns a `\\?\C:\…` verbatim path, which many
/// command-line tools cannot open. Unix has no equivalent problem.
#[cfg(windows)]
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(stripped) => PathBuf::from(stripped),
        None => path,
    }
}

#[cfg(not(windows))]
fn strip_verbatim_prefix(path: PathBuf) -> PathBuf {
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_real_directory_and_returns_it_canonicalised() {
        let root = validate_scan_root(Path::new(".")).unwrap();
        assert!(root.is_absolute());
        assert!(root.is_dir());
    }

    #[test]
    fn refuses_a_path_that_does_not_exist() {
        let error = validate_scan_root(Path::new("nirmoka-no-such-directory")).unwrap_err();
        assert!(matches!(error, AdapterError::RefusedPath { .. }));
    }

    #[test]
    fn refuses_a_file() {
        // A scan root has to be a directory; handing a backend a file produces
        // a backend-specific error message nobody can act on.
        let error = validate_scan_root(Path::new("Cargo.toml")).unwrap_err();
        match error {
            AdapterError::RefusedPath { reason, .. } => assert_eq!(reason, "not a directory"),
            other => panic!("expected RefusedPath, got {other}"),
        }
    }

    #[test]
    fn default_options_ask_for_nothing_special() {
        let options = ScanOptions::default();
        assert!(!options.one_file_system);
        assert!(!options.exclude_caches);
        assert!(options.exclude.is_empty());
    }
}
