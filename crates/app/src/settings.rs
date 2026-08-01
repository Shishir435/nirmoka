//! The one thing the shell remembers between launches.
//!
//! A backend choice that vanished on quit would be a setting in name only, so it
//! is written to a file. Everything else the app knows — trees, scans, node ids
//! — is deliberately in-memory and dies with the process.
//!
//! # Failure is not fatal, in either direction
//!
//! A missing file is the normal first run. An unreadable or malformed one is a
//! file somebody hand-edited, and the answer to both is the same: fall back to
//! the platform default and carry on. Refusing to start because a preferences
//! file has a stray comma would be a disk tool that cannot be opened to fix the
//! disk that broke it.
//!
//! Saving *does* report its failure, because a silent one is worse: the user
//! changes a setting, it appears to take, and it is gone next launch.
//!
//! # Where it lives
//!
//! From the `directories` crate, never a `~/Library` or `%APPDATA%` literal —
//! invariant 3.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use nirmoka_adapter::Preference;

const FILE: &str = "settings.json";

/// Where preferences are stored, or `None` if the platform has no home
/// directory to put them in.
///
/// `None` is survivable: the app runs, the choice applies for the session, and
/// nothing is persisted. A sandbox or a service account with no home is not a
/// reason to refuse to open a window.
///
/// # Why the arguments are not the bundle identifier
///
/// `tauri.conf.json` identifies this as `app.nirmoka.desktop`, and mirroring it
/// as `("app", "nirmoka", "desktop")` looked like the tidy answer. It is wrong
/// on Linux: `ProjectDirs` builds the XDG path from the **application name
/// alone**, ignoring qualifier and organization, so it produced
/// `~/.config/desktop` — a directory naming nothing, beside every other program
/// that made the same mistake.
///
/// The three parts are therefore chosen for what each platform does with them:
///
/// | Platform | Result                                              |
/// | -------- | --------------------------------------------------- |
/// | macOS    | `~/Library/Application Support/app.nirmoka.Nirmoka` |
/// | Linux    | `~/.config/nirmoka`                                 |
/// | Windows  | `%APPDATA%\nirmoka\Nirmoka\config`                  |
///
/// Caught by CI on Linux, which is the only reason the macOS-shaped guess did
/// not ship.
pub fn settings_path() -> Option<PathBuf> {
    ProjectDirs::from("app", "nirmoka", "Nirmoka").map(|dirs| dirs.config_dir().join(FILE))
}

/// Append-only, human-readable deletion journal.
pub fn operation_log_path() -> Option<PathBuf> {
    ProjectDirs::from("app", "nirmoka", "Nirmoka")
        .map(|dirs| dirs.data_local_dir().join("operations.jsonl"))
}

/// Read the stored preference, or the platform default if there is not one.
pub fn load() -> Preference {
    settings_path()
        .as_deref()
        .map(load_from)
        .unwrap_or_default()
}

/// Write the preference, creating the directory if it is not there yet.
pub fn save(preference: &Preference) -> Result<(), String> {
    let path = settings_path().ok_or_else(|| {
        "this system has no configuration directory, so the choice applies to this session only"
            .to_string()
    })?;

    save_to(&path, preference)
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

/// The readable half, against an explicit path so it can be tested.
///
/// Every failure returns the default. A settings file is not a source of truth
/// worth crashing over — it is a convenience over asking again.
pub fn load_from(path: &Path) -> Preference {
    let Ok(text) = fs::read_to_string(path) else {
        return Preference::default();
    };

    serde_json::from_str(&text).unwrap_or_default()
}

/// The writable half, against an explicit path so it can be tested.
///
/// Writes a temporary file and renames it over the target, because the naive
/// `fs::write` is a truncate followed by a write: a crash or a full disk between
/// the two leaves a half-written file, `load_from` degrades it to the default,
/// and the user's choice is gone. `rename` within a directory is atomic, so the
/// file on disk is either the old preference or the new one and never neither.
pub fn save_to(path: &Path, preference: &Preference) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(preference)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    // Beside the target rather than in a temp directory: `rename` is only
    // atomic within one filesystem, and `/tmp` is often a different one.
    let staged = path.with_extension("json.tmp");
    fs::write(&staged, format!("{json}\n"))?;

    fs::rename(&staged, path).inspect_err(|_| {
        // A rename that failed leaves the staged file behind. Nothing reads it,
        // but leaving litter next to a settings file is how a directory
        // accumulates files nobody can explain.
        let _ = fs::remove_file(&staged);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A temp directory that removes itself, so these tests leave nothing behind
    /// and do not need a dev-dependency to say so.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("nirmoka-settings-{name}"));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).expect("a temp directory");
            Self(path)
        }

        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_choice_survives_being_written_and_read_back() {
        let dir = TempDir::new("round-trip");
        let path = dir.join(FILE);

        save_to(&path, &Preference::of("mole")).expect("written");
        assert_eq!(load_from(&path), Preference::of("mole"));
    }

    /// The first run, and the most common state this code is in.
    #[test]
    fn a_missing_file_is_the_platform_default_not_an_error() {
        let dir = TempDir::new("missing");
        assert_eq!(
            load_from(&dir.join("never-written.json")),
            Preference::platform_default()
        );
    }

    /// A hand-edited file must not stop the app from opening. The disk tool
    /// being unopenable is a worse outcome than a forgotten preference.
    #[test]
    fn a_corrupt_file_falls_back_rather_than_failing_to_start() {
        let dir = TempDir::new("corrupt");
        let path = dir.join(FILE);

        for bad in ["{ not json", "", "[]", r#"{"chosen": 7}"#] {
            fs::write(&path, bad).expect("written");
            assert_eq!(
                load_from(&path),
                Preference::platform_default(),
                "{bad:?} should degrade to the default"
            );
        }
    }

    /// Clearing a choice has to be storable, or "go back to the default" would
    /// be a one-way door: the file would keep naming the old backend.
    #[test]
    fn clearing_a_choice_is_stored_rather_than_leaving_the_old_one() {
        let dir = TempDir::new("cleared");
        let path = dir.join(FILE);

        save_to(&path, &Preference::of("mole")).expect("written");
        save_to(&path, &Preference::platform_default()).expect("written");

        assert_eq!(load_from(&path), Preference::platform_default());
    }

    /// A settings file is either the old preference or the new one.
    ///
    /// The failure this guards against is a truncate that succeeds and a write
    /// that does not: `load_from` degrades a half-written file to the default,
    /// so a crash mid-save would silently discard a choice the user had made
    /// once and never touched again.
    #[test]
    fn a_save_leaves_no_half_written_file_behind() {
        let dir = TempDir::new("atomic");
        let path = dir.join(FILE);
        let staged = path.with_extension("json.tmp");

        save_to(&path, &Preference::of("ncdu")).expect("written");
        save_to(&path, &Preference::of("mole")).expect("written");

        assert!(
            !staged.exists(),
            "the staging file outlived the save that made it"
        );
        assert_eq!(load_from(&path), Preference::of("mole"));
    }

    #[test]
    fn saving_creates_the_directory_it_needs() {
        let dir = TempDir::new("nested");
        let path = dir.join("a").join("b").join(FILE);

        save_to(&path, &Preference::of("ncdu")).expect("written");
        assert!(path.exists());
        assert_eq!(load_from(&path), Preference::of("ncdu"));
    }

    /// The settings must land somewhere that names this application.
    ///
    /// Not a style check. `ProjectDirs` uses different parts of its three
    /// arguments on different platforms — Linux uses only the application name —
    /// so a triple that reads correctly on macOS can silently put the file in
    /// `~/.config/desktop` on Linux. This test is what caught exactly that.
    ///
    /// The directory is checked rather than the whole path, so a machine whose
    /// home happens to contain "nirmoka" cannot pass it by accident.
    #[test]
    fn the_settings_path_names_this_application_on_every_platform() {
        let Some(path) = settings_path() else {
            return; // No home directory on this machine; that is a valid state.
        };

        assert!(path.is_absolute(), "{}", path.display());
        assert!(path.ends_with(FILE), "{}", path.display());

        let directory = path
            .parent()
            .and_then(|parent| parent.file_name())
            .expect("a settings file lives in a directory")
            .to_string_lossy()
            .to_lowercase();

        // Windows nests a `config` directory under the application name, so the
        // parent itself is allowed to be that.
        let named = directory.contains("nirmoka")
            || (directory == "config" && path.to_string_lossy().to_lowercase().contains("nirmoka"));

        assert!(named, "settings would land in {}", path.display());
    }
}
