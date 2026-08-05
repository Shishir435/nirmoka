//! Moving a path the user selected to the platform's own Trash.
//!
//! This is a shell integration for the same reason Reveal in Finder is one: no
//! disk tool is involved, and the answer depends on the desktop rather than on
//! which scanner is installed. Every adapter still reports `delete: false` and
//! `trash: false`, and the `Adapter` trait gains nothing — see
//! [ADR 0025](../../../docs/adr/0025-move-to-trash-is-a-platform-integration.md).
//!
//! **What this does not do is close the race ADR 0017 identified.** The platform
//! API takes a path and resolves it itself, after Nirmoka's check, so another
//! process can still swap an ancestor in between. Validating again immediately
//! before the move closes the stale-confirmation and symlink-retarget cases,
//! which are the ones a check can close. What makes the remaining case
//! survivable is that the item lands in the Trash: macOS records where it came
//! from, and Put Back restores it exactly. The wrong item being recoverable is
//! the whole basis for offering this, which is why permanent removal is not
//! offered beside it.

use std::path::{Path, PathBuf};

use nirmoka_adapter::validate_delete_target;

/// The platform's own word for the operation.
///
/// "Move to Trash" on Windows would be a macOS habit leaking into another
/// desktop, the same mistake `reveal_label` avoids.
pub fn label_for(os: &str) -> &'static str {
    match os {
        "windows" => "Move to Recycle Bin",
        _ => "Move to Trash",
    }
}

/// Validate a target without moving it.
///
/// The resolved path is what the confirmation dialog names, and what the
/// pending operation holds behind its one-time token. A raw path from the
/// window never reaches [`move_to_trash`].
pub fn plan(scan_root: &Path, target: &Path) -> Result<PathBuf, String> {
    validate(scan_root, target)
}

/// Move a validated target to the Trash, and report where it was.
///
/// The target is validated again here rather than trusted from the plan: a
/// confirmation dialog can sit open for as long as the user leaves it open, and
/// the filesystem does not wait.
pub fn move_to_trash(scan_root: &Path, target: &Path) -> Result<PathBuf, String> {
    let resolved = validate(scan_root, target)?;

    context()
        .delete(&resolved)
        .map_err(|error| describe(&resolved, &error))?;

    Ok(resolved)
}

/// macOS has two routes to the Trash and they are not interchangeable.
///
/// `NSFileManager`'s `trashItemAtURL:` needs no permission and does not
/// reliably record the entry that makes **Put Back** work — a long-standing
/// system bug, and the reason the crate documents the two separately. Asking
/// the Finder does record it, and needs Automation permission.
///
/// Put Back is the argument ADR 0025 rests on, so Nirmoka asks the Finder and
/// reports the permission it needs when it is refused. Choosing the quieter
/// route would keep the button working and quietly remove the property that
/// justified adding the button.
#[cfg(target_os = "macos")]
fn context() -> trash::TrashContext {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};

    let mut context = trash::TrashContext::default();
    context.set_delete_method(DeleteMethod::Finder);
    context
}

#[cfg(not(target_os = "macos"))]
fn context() -> trash::TrashContext {
    trash::TrashContext::default()
}

/// Turn a platform failure into something a person can act on.
fn describe(resolved: &Path, error: &trash::Error) -> String {
    let target = resolved.display();
    match error {
        // A volume with no trash directory, a permission never granted, or the
        // Finder refusing an Apple event. All are the platform declining, which
        // is a different problem from a bad selection.
        trash::Error::Unknown { description } | trash::Error::Os { description, .. } => {
            if denied_automation(description) {
                format!(
                    "macOS did not let Nirmoka ask the Finder to move {target} to the Trash. \
                     Allow it under System Settings › Privacy & Security › Automation, \
                     then try again."
                )
            } else {
                format!(
                    "the operating system did not move {target} to the {}: {description}",
                    trash_noun()
                )
            }
        }
        trash::Error::CouldNotAccess { .. } => {
            format!("{target} could not be opened — it may have been moved or removed")
        }
        other => format!("could not move {target} to the {}: {other}", trash_noun()),
    }
}

/// `-1743` is the Apple event authorization refusal, and its message is the
/// only part of this that a user recognises.
fn denied_automation(description: &str) -> bool {
    description.contains("-1743") || description.contains("Not authorized to send Apple events")
}

fn trash_noun() -> &'static str {
    if cfg!(windows) {
        "Recycle Bin"
    } else {
        "Trash"
    }
}

/// Absolute, canonical, strictly below the scan root, and not one of the
/// protected operating-system locations.
///
/// The rules are the shared validator's, unchanged and not reimplemented here.
/// A destructive operation with its own private idea of containment is how two
/// answers to the same question start to disagree.
fn validate(scan_root: &Path, target: &Path) -> Result<PathBuf, String> {
    if target.as_os_str().is_empty() {
        return Err("no path was given".to_string());
    }
    validate_delete_target(scan_root, target).map_err(|error| error.to_string())
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
                "nirmoka-trash-{}-{nonce}-{}",
                std::process::id(),
                NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).expect("create fixture root");
            Self { root }
        }

        fn file(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            fs::write(&path, b"disposable").expect("write fixture file");
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    /// The message for a refused Apple event has to name the setting, because
    /// "operation not permitted" sends nobody anywhere.
    #[test]
    fn a_refused_apple_event_names_the_setting_that_fixes_it() {
        let refused = describe(
            Path::new("/scan/big"),
            &trash::Error::Unknown {
                description: "Not authorized to send Apple events to Finder. (-1743)".to_string(),
            },
        );

        assert!(refused.contains("Automation"), "{refused}");
        assert!(refused.contains("/scan/big"), "{refused}");
    }

    #[test]
    fn an_unrecognised_platform_failure_is_reported_verbatim() {
        let reported = describe(
            Path::new("/scan/big"),
            &trash::Error::Unknown {
                description: "the volume has no trash directory".to_string(),
            },
        );

        assert!(reported.contains("no trash directory"), "{reported}");
        assert!(!reported.contains("Automation"), "{reported}");
    }

    #[test]
    fn every_platform_gets_its_own_word() {
        assert_eq!(label_for("macos"), "Move to Trash");
        assert_eq!(label_for("windows"), "Move to Recycle Bin");
        assert_eq!(label_for("linux"), "Move to Trash");
    }

    #[test]
    fn an_empty_path_is_refused_before_the_validator_sees_it() {
        let fixture = Fixture::new();

        let error = move_to_trash(&fixture.root, Path::new("")).expect_err("not a path");

        assert_eq!(error, "no path was given");
    }

    /// A scan describes the filesystem as it was. A row whose file is already
    /// gone must not become a move of whatever took its name.
    #[test]
    fn a_target_that_is_gone_is_refused() {
        let fixture = Fixture::new();

        let error =
            move_to_trash(&fixture.root, &fixture.root.join("missing")).expect_err("nothing there");

        assert!(error.contains("cannot be resolved"), "{error}");
    }

    #[test]
    fn the_scan_root_itself_is_refused() {
        let fixture = Fixture::new();

        let error = move_to_trash(&fixture.root, &fixture.root).expect_err("not the root");

        assert!(error.contains("cannot delete the scan root"), "{error}");
    }

    #[test]
    fn a_relative_path_is_refused_rather_than_resolved_against_the_process() {
        let fixture = Fixture::new();

        let error = move_to_trash(&fixture.root, Path::new("child")).expect_err("not absolute");

        assert!(error.contains("not absolute"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_leaving_the_scan_root_is_refused() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        let outside = fixture.root.with_extension("outside");
        fs::create_dir(&outside).unwrap();
        let link = fixture.root.join("escape");
        symlink(&outside, &link).unwrap();

        let error = move_to_trash(&fixture.root, &link).expect_err("resolves outside");
        let survived = outside.exists();
        fs::remove_dir_all(&outside).unwrap();

        assert!(error.contains("outside the scan root"), "{error}");
        assert!(survived, "the link target must not have been touched");
    }

    #[cfg(unix)]
    #[test]
    fn a_system_location_is_refused_even_with_a_scan_root_that_contains_it() {
        let error = move_to_trash(Path::new("/"), Path::new("/etc")).expect_err("system location");

        assert!(error.contains("system-critical"), "{error}");
    }

    /// The one test that actually moves something. It leaves an item in the
    /// Trash, which is the point: a mock here would prove only that the mock
    /// was called.
    ///
    /// What it can assert is that the file left its original location. macOS
    /// exposes no supported way to enumerate the Trash — the `trash` crate
    /// compiles its listing API out on this platform — so asserting the item
    /// arrived would mean guessing a name inside `~/.Trash`, where the system
    /// renames on collision.
    #[test]
    fn a_real_file_leaves_its_directory_for_the_trash() {
        let fixture = Fixture::new();
        let doomed = fixture.file("disposable.txt");
        // Resolved before the move, because afterwards there is nothing left to
        // resolve and the comparison would pass for the wrong reason.
        let expected = doomed.canonicalize().expect("the fixture file exists");

        let moved = move_to_trash(&fixture.root, &doomed).expect("the platform has a trash");

        assert_eq!(moved, expected, "the reported path is the resolved one");
        assert!(!doomed.exists(), "the file is no longer where it was");
    }

    /// Planning is validation and nothing else. Nothing may move before the
    /// user has confirmed.
    #[test]
    fn planning_resolves_without_moving_anything() {
        let fixture = Fixture::new();
        let kept = fixture.file("kept.txt");

        let planned = plan(&fixture.root, &kept).expect("a valid target");

        assert!(planned.is_absolute());
        assert!(kept.exists(), "planning is not doing");
    }
}
