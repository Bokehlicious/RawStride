//! Shared types, error handling and configuration used by every RawStride crate.
//!
//! Responsibilities (planned):
//! - Common identifiers (asset id, volume id, session id)
//! - The workspace-wide error type
//! - Application configuration and well-known paths
//!
//! This crate must stay dependency-light: no I/O, no FFI, no async runtime.

/// Formats a byte count as a human-readable string using binary units (1 KB = 1024 B).
///
/// Values below 1024 are shown as whole bytes; larger values are shown with one
/// decimal in KB, MB or GB. A value that would round up to `1024.0` in its unit
/// is promoted to the next unit, so the output never reads e.g. `"1024.0 KB"`.
/// Values beyond the GB range stay in GB.
#[must_use]
pub fn format_bytes(bytes: u64) -> String {
    const STEP: f64 = 1024.0;
    // Smallest value that `{:.1}` would print as "1024.0".
    const PROMOTE_AT: f64 = 1023.95;

    if bytes < 1024 {
        return format!("{bytes} B");
    }

    // Precision loss for very large u64 values is irrelevant at one decimal.
    let mut value = bytes as f64 / STEP;
    let mut unit = "KB";
    for next in ["MB", "GB"] {
        if value < PROMOTE_AT {
            break;
        }
        value /= STEP;
        unit = next;
    }

    format!("{value:.1} {unit}")
}

#[cfg(test)]
mod tests {
    use super::format_bytes;

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn test_format_bytes_below_kb() {
        assert_eq!(format_bytes(512), "512 B");
    }

    #[test]
    fn test_format_bytes_largest_byte_value() {
        assert_eq!(format_bytes(1023), "1023 B");
    }

    #[test]
    fn test_format_bytes_one_kb() {
        assert_eq!(format_bytes(1024), "1.0 KB");
    }

    #[test]
    fn test_format_bytes_one_point_five_kb() {
        assert_eq!(format_bytes(1536), "1.5 KB");
    }

    #[test]
    fn test_format_bytes_two_kb() {
        assert_eq!(format_bytes(2048), "2.0 KB");
    }

    #[test]
    fn test_format_bytes_largest_value_shown_in_kb() {
        // 1_048_524 / 1024 = 1023.949… which still prints as 1023.9.
        assert_eq!(format_bytes(1_048_524), "1023.9 KB");
    }

    #[test]
    fn test_format_bytes_rounds_up_into_mb_instead_of_1024_kb() {
        assert_eq!(format_bytes(1_048_575), "1.0 MB");
    }

    #[test]
    fn test_format_bytes_one_mb() {
        assert_eq!(format_bytes(1_048_576), "1.0 MB");
    }

    #[test]
    fn test_format_bytes_one_point_five_mb() {
        assert_eq!(format_bytes(1_572_864), "1.5 MB");
    }

    #[test]
    fn test_format_bytes_rounds_up_into_gb_instead_of_1024_mb() {
        assert_eq!(format_bytes(1_073_741_823), "1.0 GB");
    }

    #[test]
    fn test_format_bytes_one_gb() {
        assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
    }

    #[test]
    fn test_format_bytes_one_point_five_gb() {
        assert_eq!(format_bytes(1_610_612_736), "1.5 GB");
    }

    #[test]
    fn test_format_bytes_max_does_not_panic_and_stays_in_gb() {
        assert_eq!(format_bytes(u64::MAX), "17179869184.0 GB");
    }
}
