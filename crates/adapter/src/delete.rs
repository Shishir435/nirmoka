//! Validation shared by adapters before a path reaches a destructive command.
//!
//! This is intentionally only the common floor. A backend's own protection
//! rules still run afterwards, and adapters must not copy those rules here.

use std::path::{Path, PathBuf};

use crate::AdapterError;

/// Resolve and validate one caller-selected deletion target.
///
/// A successful result is absolute, canonical, strictly below `scan_root`, and
/// outside the small set of operating-system roots Nirmoka protects itself.
/// The resolved path is the only path an adapter may pass to its backend.
///
/// This check does not replace backend protections. In particular, a backend
/// with curated safety rules must still apply them rather than assuming this
/// generic boundary knows everything it does.
pub fn validate_delete_target(scan_root: &Path, target: &Path) -> Result<PathBuf, AdapterError> {
    require_absolute(scan_root, "scan root")?;
    require_absolute(target, "target")?;

    let root = canonicalize(scan_root)?;
    if !root.is_dir() {
        return refused(scan_root, "scan root is not a directory");
    }

    let resolved = canonicalize(target)?;
    if resolved == root {
        return refused(target, "cannot delete the scan root");
    }
    if !resolved.starts_with(&root) {
        return refused(target, "resolved target is outside the scan root");
    }
    if is_system_critical(&resolved) {
        return refused(target, "resolved target is a system-critical location");
    }

    Ok(strip_verbatim_prefix(resolved))
}

fn require_absolute(path: &Path, role: &str) -> Result<(), AdapterError> {
    if path.is_absolute() {
        Ok(())
    } else {
        refused(path, format!("{role} is not absolute"))
    }
}

fn canonicalize(path: &Path) -> Result<PathBuf, AdapterError> {
    path.canonicalize()
        .map_err(|source| AdapterError::RefusedPath {
            path: path.display().to_string(),
            reason: format!("cannot be resolved: {source}"),
        })
}

fn refused<T>(path: &Path, reason: impl Into<String>) -> Result<T, AdapterError> {
    Err(AdapterError::RefusedPath {
        path: path.display().to_string(),
        reason: reason.into(),
    })
}

#[cfg(unix)]
fn is_system_critical(path: &Path) -> bool {
    // This is a deliberately short, independently maintained set of OS roots,
    // not a copy of any backend's curated cleanup or protection tables.
    const ROOTS: &[&str] = &[
        "/bin",
        "/boot",
        "/dev",
        "/etc",
        "/lib",
        "/lib64",
        "/proc",
        "/sbin",
        "/sys",
        "/usr",
        "/System",
        "/Library",
        "/private/etc",
    ];

    ROOTS
        .iter()
        .any(|root| starts_with_system_root(path, Path::new(root)))
}

#[cfg(windows)]
fn is_system_critical(path: &Path) -> bool {
    [
        std::env::var_os("SystemRoot"),
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
        std::env::var_os("ProgramData"),
    ]
    .into_iter()
    .flatten()
    .map(PathBuf::from)
    .filter_map(|root| root.canonicalize().ok())
    .any(|root| starts_with_system_root(path, &root))
}

#[cfg(any(target_os = "macos", windows))]
fn starts_with_system_root(path: &Path, root: &Path) -> bool {
    let mut path_components = path.components();
    root.components().all(|expected| {
        path_components.next().is_some_and(|actual| {
            actual
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(&expected.as_os_str().to_string_lossy())
        })
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
fn starts_with_system_root(path: &Path, root: &Path) -> bool {
    path.starts_with(root)
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
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct Fixture {
        root: PathBuf,
    }

    static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock is after epoch")
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "nirmoka-delete-validation-{}-{nonce}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).expect("create fixture root");
            Self { root }
        }

        fn child(&self, name: &str) -> PathBuf {
            self.root.join(name)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn reason(error: AdapterError) -> String {
        match error {
            AdapterError::RefusedPath { reason, .. } => reason,
            other => panic!("expected RefusedPath, got {other}"),
        }
    }

    #[test]
    fn accepts_an_existing_descendant_and_returns_it_canonicalised() {
        let fixture = Fixture::new();
        let child = fixture.child("child");
        fs::create_dir(&child).unwrap();

        let validated = validate_delete_target(&fixture.root, &child).unwrap();

        assert!(validated.is_absolute());
        assert_eq!(validated, child.canonicalize().unwrap());
    }

    #[test]
    fn refuses_relative_inputs_instead_of_resolving_them_against_process_state() {
        let fixture = Fixture::new();
        assert_eq!(
            reason(validate_delete_target(Path::new("."), &fixture.root).unwrap_err()),
            "scan root is not absolute"
        );
        assert_eq!(
            reason(validate_delete_target(&fixture.root, Path::new("child")).unwrap_err()),
            "target is not absolute"
        );
    }

    #[test]
    fn refuses_the_scan_root_itself() {
        let fixture = Fixture::new();
        assert_eq!(
            reason(validate_delete_target(&fixture.root, &fixture.root).unwrap_err()),
            "cannot delete the scan root"
        );
    }

    #[test]
    fn refuses_a_sibling_outside_the_scan_root() {
        let fixture = Fixture::new();
        let sibling = fixture.root.with_extension("sibling");
        fs::create_dir(&sibling).unwrap();

        let error = validate_delete_target(&fixture.root, &sibling).unwrap_err();
        fs::remove_dir(&sibling).unwrap();

        assert_eq!(reason(error), "resolved target is outside the scan root");
    }

    #[cfg(unix)]
    #[test]
    fn resolves_a_symlink_before_checking_containment() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let outside = fixture.root.with_extension("outside");
        fs::create_dir(&outside).unwrap();
        let link = fixture.child("escape");
        symlink(&outside, &link).unwrap();

        let error = validate_delete_target(&fixture.root, &link).unwrap_err();
        fs::remove_dir_all(&outside).unwrap();

        assert_eq!(reason(error), "resolved target is outside the scan root");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_system_locations_even_when_the_scan_root_contains_them() {
        let error = validate_delete_target(Path::new("/"), Path::new("/etc")).unwrap_err();
        assert_eq!(
            reason(error),
            "resolved target is a system-critical location"
        );
    }

    #[test]
    fn refuses_missing_and_dangling_targets() {
        let fixture = Fixture::new();
        let error = validate_delete_target(&fixture.root, &fixture.child("missing")).unwrap_err();
        assert!(reason(error).starts_with("cannot be resolved:"));
    }
}
