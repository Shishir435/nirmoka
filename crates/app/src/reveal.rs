//! Handing a path to the operating system's own viewer.
//!
//! Reveal and Quick Look are shell integrations, not backend abilities. No disk
//! tool is involved, nothing is removed, and the answer depends on the desktop
//! rather than on which scanner is installed — so they live here with the window
//! instead of behind `Capabilities`. Putting them in an adapter would make
//! "which backend reveals a file" a question with no meaningful answer.
//!
//! Platform conditionals are allowed here for the same reason they are allowed
//! in an adapter and not in `core` (invariant 3): this crate is already the
//! platform-facing edge.
//!
//! Both take a path the user selected from a scan. It is canonicalised and
//! checked to exist before it becomes an argument, and it is passed as an
//! argument rather than interpolated into a shell string.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::dto::PlatformFeatures;

/// What this platform can do with a selected path.
pub fn features() -> PlatformFeatures {
    features_for(std::env::consts::OS)
}

/// Matched on the OS name at runtime rather than compiled per target, so every
/// platform's answer is testable from every platform.
fn features_for(os: &str) -> PlatformFeatures {
    PlatformFeatures {
        // The label is the platform's own word for the thing. "Reveal in Finder"
        // on Windows would be a macOS habit leaking into another desktop.
        reveal_label: match os {
            "macos" => "Reveal in Finder",
            "windows" => "Show in File Explorer",
            _ => "Show in file manager",
        }
        .to_string(),
        // Quick Look is a macOS feature. Naming a Linux or Windows equivalent
        // would mean picking one that may not be installed.
        quick_look: os == "macos",
    }
}

/// Open the platform's file manager with `path` selected.
pub fn reveal(path: &Path) -> Result<(), String> {
    let target = existing(path)?;

    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg("-R").arg(&target);
        command
    };

    // `explorer` exits non-zero even when it worked, so its status is ignored
    // below rather than reported as a failure the user did not have.
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer");
        command.arg(format!("/select,{}", target.display()));
        command
    };

    // xdg-open takes the directory, not a "select this entry" argument. The
    // file manager opens on the containing folder without highlighting, which is
    // less than macOS does and is what this platform offers.
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(target.parent().unwrap_or(&target));
        command
    };

    let status = command
        .status()
        .map_err(|error| format!("could not open the file manager: {error}"))?;

    if cfg!(target_os = "windows") || status.success() {
        Ok(())
    } else {
        Err(format!(
            "the file manager exited with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Show `path` in Quick Look.
///
/// `qlmanage -p` is the scriptable entry point. It blocks for as long as the
/// panel is open, so the caller runs it off the main thread.
pub fn quick_look(path: &Path) -> Result<(), String> {
    if !features().quick_look {
        return Err("Quick Look is a macOS feature".to_string());
    }
    let target = existing(path)?;

    let status = Command::new("qlmanage")
        .arg("-p")
        .arg(&target)
        // qlmanage is chatty on both streams and says nothing worth showing.
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map_err(|error| format!("could not start Quick Look: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "Quick Look exited with status {}",
            status.code().unwrap_or(-1)
        ))
    }
}

/// Canonicalise, and refuse anything that is not there.
///
/// A scan describes the filesystem as it was. Passing a path that has since
/// been removed would open the file manager on a stale location, or on a
/// different file if the name was reused.
fn existing(path: &Path) -> Result<PathBuf, String> {
    if path.as_os_str().is_empty() {
        return Err("no path was given".to_string());
    }
    std::fs::canonicalize(path)
        .map_err(|error| format!("{} is not available: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_platform_gets_its_own_wording() {
        assert_eq!(features_for("macos").reveal_label, "Reveal in Finder");
        assert_eq!(
            features_for("windows").reveal_label,
            "Show in File Explorer"
        );
        assert_eq!(features_for("linux").reveal_label, "Show in file manager");
    }

    /// Offering Quick Look elsewhere would mean naming an equivalent that may
    /// not be installed. `false` is the honest answer, not a missing feature.
    #[test]
    fn quick_look_is_claimed_on_macos_only() {
        assert!(features_for("macos").quick_look);
        assert!(!features_for("windows").quick_look);
        assert!(!features_for("linux").quick_look);
    }

    #[test]
    fn a_path_that_is_gone_is_refused_before_any_subprocess() {
        let missing = std::env::temp_dir().join(format!(
            "nirmoka-reveal-missing-{}-{}",
            std::process::id(),
            "no-such-entry"
        ));
        let error = existing(&missing).expect_err("a stale path must not be opened");

        assert!(error.contains("is not available"), "{error}");
        assert!(
            existing(Path::new("")).is_err(),
            "an empty path is not a path"
        );
    }

    #[test]
    fn an_existing_path_canonicalises() {
        let resolved = existing(&std::env::temp_dir()).expect("the temp directory exists");

        assert!(resolved.is_absolute());
    }

    /// Quick Look must refuse rather than spawn a binary that is not there.
    #[test]
    #[cfg(not(target_os = "macos"))]
    fn quick_look_off_macos_refuses_without_spawning() {
        let error = quick_look(&std::env::temp_dir()).expect_err("not a macOS feature");

        assert!(error.contains("macOS"), "{error}");
    }
}
