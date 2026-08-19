//! Filesystem capacity for the desktop shell.
//!
//! This does not walk the disk. It asks the operating system's `df` utility
//! about the volume containing an already-selected path, keeping volume
//! capacity distinct from bytes reached by an ncdu scan.
//!
//! Capacity is the one number this project can report without a backend, which
//! is what lets the window open on something informative instead of an empty
//! state telling the user to type a path.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::dto::VolumeInfo;

#[cfg(target_os = "macos")]
pub fn info(path: &Path) -> Result<VolumeInfo, String> {
    let output = Command::new("df")
        .args(["-kP"])
        .arg(path)
        .output()
        .map_err(|error| format!("could not read volume information: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let mut value = parse_df(&String::from_utf8_lossy(&output.stdout))?;
    value.name = name_for(
        &value.mount_point,
        boot_volume_name(&mounted_volumes()).as_deref(),
    );
    Ok(value)
}

#[cfg(not(target_os = "macos"))]
pub fn info(_path: &Path) -> Result<VolumeInfo, String> {
    Err("volume information is available in the macOS beta only".to_string())
}

/// The helpers below parse and label macOS `df` output. They are compiled for
/// the tests everywhere so that every platform's CI covers them, and into the
/// binary only where something calls them.
#[cfg(any(target_os = "macos", test))]
fn parse_df(output: &str) -> Result<VolumeInfo, String> {
    let line = output
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .ok_or_else(|| "df returned no volume row".to_string())?;
    let columns: Vec<_> = line.split_whitespace().collect();
    if columns.len() < 6 {
        return Err("df returned an unexpected volume row".to_string());
    }
    let blocks = |index: usize| -> Result<u64, String> {
        columns[index]
            .parse::<u64>()
            .map(|value| value.saturating_mul(1024))
            .map_err(|_| "df returned a non-numeric capacity".to_string())
    };
    let mount_point = columns[5..].join(" ");
    Ok(VolumeInfo {
        // Filled in by the caller, which is the only place that can look at
        // `/Volumes`. Parsing and naming are separate so the parser stays a
        // pure function over text.
        name: mount_point.clone(),
        mount_point,
        total_bytes: blocks(1)?,
        used_bytes: blocks(2)?,
        free_bytes: blocks(3)?,
    })
}

/// `(entry name, symlink target)` for everything in `/Volumes`.
///
/// The boot volume appears there as a symlink to `/` rather than as a mount, so
/// its user-facing name is not recoverable from `df` output alone.
#[cfg(any(target_os = "macos", test))]
fn mounted_volumes() -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir("/Volumes") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let target = std::fs::read_link(entry.path()).ok()?;
            Some((entry.file_name().to_string_lossy().into_owned(), target))
        })
        .collect()
}

/// The name of whichever `/Volumes` entry points at the root filesystem.
#[cfg(any(target_os = "macos", test))]
fn boot_volume_name(entries: &[(String, PathBuf)]) -> Option<String> {
    entries
        .iter()
        .find(|(_, target)| target == Path::new("/"))
        .map(|(name, _)| name.clone())
}

/// What to call the volume a mount point belongs to.
///
/// Three cases, and no guessing in any of them. The boot volume group reports
/// two mount points — `/` for the sealed system volume and
/// `/System/Volumes/Data` for everything a user owns — and neither string is
/// what the Finder calls it, so both defer to the name in `/Volumes`. Anything
/// mounted under `/Volumes` is already named by its mount point. Anything else
/// keeps the mount point, which is at least true.
#[cfg(any(target_os = "macos", test))]
fn name_for(mount_point: &str, boot_name: Option<&str>) -> String {
    if matches!(mount_point, "/" | "/System/Volumes/Data") {
        // Apple's own term for this volume when its name cannot be read, rather
        // than the "Macintosh HD" default that a renamed disk would make wrong.
        return boot_name.unwrap_or("Startup disk").to_string();
    }
    mount_point
        .strip_prefix("/Volumes/")
        .filter(|rest| !rest.is_empty() && !rest.contains('/'))
        .unwrap_or(mount_point)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_posix_df_output() {
        let value = parse_df(
            "Filesystem 1024-blocks Used Available Capacity Mounted on\n/dev/disk3s1 1000 600 400 60% /System/Volumes/Data\n",
        )
        .unwrap();
        assert_eq!(value.total_bytes, 1_024_000);
        assert_eq!(value.used_bytes, 614_400);
        assert_eq!(value.free_bytes, 409_600);
        assert_eq!(value.mount_point, "/System/Volumes/Data");
    }

    /// Both halves of the boot volume group get the name the Finder shows, which
    /// is the whole reason this lookup exists: `df ~` reports
    /// `/System/Volumes/Data`, and nobody calls their disk that.
    #[test]
    fn both_boot_mount_points_take_the_name_from_volumes() {
        assert_eq!(name_for("/", Some("Macintosh HD")), "Macintosh HD");
        assert_eq!(
            name_for("/System/Volumes/Data", Some("Macintosh HD")),
            "Macintosh HD"
        );
        assert_eq!(name_for("/", Some("Shishir's SSD")), "Shishir's SSD");
    }

    /// A disk whose name cannot be read is described, not invented. "Macintosh
    /// HD" would be a guess that is wrong on every renamed disk.
    #[test]
    fn an_unreadable_boot_name_falls_back_to_apples_own_term() {
        assert_eq!(name_for("/", None), "Startup disk");
        assert_eq!(name_for("/System/Volumes/Data", None), "Startup disk");
    }

    #[test]
    fn a_mounted_volume_is_named_by_its_mount_point() {
        assert_eq!(name_for("/Volumes/Backup", None), "Backup");
        assert_eq!(name_for("/Volumes/Time Machine", None), "Time Machine");
    }

    /// Anything else keeps the mount point. A nested path under `/Volumes` is a
    /// directory on a volume rather than the volume, so the last component would
    /// name the wrong thing.
    #[test]
    fn anything_else_keeps_the_mount_point() {
        assert_eq!(
            name_for("/Volumes/Backup/nested", None),
            "/Volumes/Backup/nested"
        );
        assert_eq!(name_for("/private/var/vm", None), "/private/var/vm");
        assert_eq!(name_for("/Volumes/", None), "/Volumes/");
    }

    #[test]
    fn the_boot_volume_is_the_volumes_entry_pointing_at_root() {
        let entries = vec![
            ("Backup".to_string(), PathBuf::from("/dev/disk5s2")),
            ("Macintosh HD".to_string(), PathBuf::from("/")),
        ];
        assert_eq!(boot_volume_name(&entries).as_deref(), Some("Macintosh HD"));
        assert_eq!(boot_volume_name(&[]), None);
    }
}
