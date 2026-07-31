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
//! invariant 3. The identifier matches `tauri.conf.json`, so the settings sit
//! beside whatever else the bundle owns rather than in a second location with a
//! different name.

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
pub fn settings_path() -> Option<PathBuf> {
    // Matches `identifier` in tauri.conf.json: app.nirmoka.desktop.
    ProjectDirs::from("app", "nirmoka", "desktop").map(|dirs| dirs.config_dir().join(FILE))
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
pub fn save_to(path: &Path, preference: &Preference) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(preference)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    fs::write(path, format!("{json}\n"))
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

    #[test]
    fn saving_creates_the_directory_it_needs() {
        let dir = TempDir::new("nested");
        let path = dir.join("a").join("b").join(FILE);

        save_to(&path, &Preference::of("ncdu")).expect("written");
        assert!(path.exists());
        assert_eq!(load_from(&path), Preference::of("ncdu"));
    }

    /// Not `~/Library` and not `%APPDATA%` spelled out — invariant 3.
    #[test]
    fn the_settings_path_is_under_a_real_config_directory() {
        let Some(path) = settings_path() else {
            return; // No home directory on this machine; that is a valid state.
        };

        assert!(path.is_absolute());
        assert!(path.ends_with(FILE));
        assert!(path.to_string_lossy().contains("nirmoka"));
    }
}
