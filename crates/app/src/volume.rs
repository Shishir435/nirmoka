//! Filesystem capacity for the desktop shell.
//!
//! This does not walk the disk. It asks the operating system's `df` utility
//! about the volume containing an already-selected path, keeping volume
//! capacity distinct from bytes reached by an ncdu scan.

use std::path::Path;
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
    parse_df(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(not(target_os = "macos"))]
pub fn info(_path: &Path) -> Result<VolumeInfo, String> {
    Err("volume information is available in the macOS beta only".to_string())
}

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
    Ok(VolumeInfo {
        mount_point: columns[5..].join(" "),
        total_bytes: blocks(1)?,
        used_bytes: blocks(2)?,
        free_bytes: blocks(3)?,
    })
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
}
