//! Size formatting. Sizes are always `u64` bytes internally; formatting happens
//! at the edge so sorting and arithmetic never operate on strings.

const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];

/// Format a byte count for display, using 1024-based units.
///
/// Deliberately terse and fixed-width-ish so size columns align in a table.
pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }

    let mut value = bytes as f64;
    let mut unit = 0usize;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if value < 10.0 {
        format!("{value:.2} {}", UNITS[unit])
    } else if value < 100.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn formats_bytes_below_one_kib() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(999), "999 B");
    }

    #[test]
    fn steps_through_units() {
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
    }

    #[test]
    fn reduces_precision_as_magnitude_grows() {
        assert_eq!(format_bytes(15 * 1024), "15.0 KB");
        assert_eq!(format_bytes(512 * 1024), "512 KB");
    }

    #[test]
    fn saturates_at_the_largest_known_unit() {
        // u64::MAX must not panic or run off the end of UNITS.
        assert!(format_bytes(u64::MAX).ends_with(" PB"));
    }
}
