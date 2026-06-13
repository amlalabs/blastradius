//! Minimal UTC timestamp formatting (RFC3339) without pulling in a date crate.

use std::time::{SystemTime, UNIX_EPOCH};

/// Current time as `YYYY-MM-DDTHH:MM:SSZ`. Falls back to the epoch on error.
pub fn now_iso8601() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    iso8601_from_unix(secs)
}

/// Convert unix seconds (UTC) to an RFC3339 string. Uses Howard Hinnant's
/// civil-from-days algorithm.
pub fn iso8601_from_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;

    // days since 1970-01-01 → civil (y, m, d).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, hour, minute, second
    )
}

/// Parse a relative duration like `7d`, `12h`, `30m`, `2w` into seconds.
///
/// Suffixes: `m` minutes, `h` hours, `d` days, `w` weeks. Returns `None` on any
/// malformed input (empty, bad suffix, non-numeric, or overflow) so callers can
/// route invalid duration values to a usage error rather than panic.
pub fn parse_duration(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.len() < 2 {
        return None;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    let n: u64 = num.parse().ok()?;
    let mult: u64 = match unit {
        "m" => 60,
        "h" => 3_600,
        "d" => 86_400,
        "w" => 604_800,
        _ => return None,
    };
    n.checked_mul(mult)
}

/// Reverse of [`iso8601_from_unix`]: parse an RFC3339 UTC timestamp
/// (`YYYY-MM-DDTHH:MM:SSZ`, optional fractional seconds and `Z`) into unix
/// seconds using the days-from-civil algorithm. Returns `None` on malformed
/// input. Only the `Z`/UTC form is supported (transcripts emit UTC).
pub fn unix_from_iso8601(s: &str) -> Option<u64> {
    let s = s.trim();
    // Split date and time on 'T' (also tolerate a space separator).
    let (date, rest) = s.split_once('T').or_else(|| s.split_once(' '))?;
    let mut d = date.splitn(3, '-');
    let year: i64 = d.next()?.parse().ok()?;
    let month: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }

    // Strip trailing 'Z' and any timezone/fractional part; keep HH:MM:SS.
    let rest = rest.trim_end_matches('Z');
    let time_part = rest.split(['+', '.']).next().unwrap_or(rest);
    let mut t = time_part.splitn(3, ':');
    let hour: i64 = t.next()?.parse().ok()?;
    let minute: i64 = t.next()?.parse().ok()?;
    let second: i64 = t.next().unwrap_or("0").parse().ok()?;
    if hour >= 24 || minute >= 60 || second >= 61 {
        return None;
    }

    // days_from_civil (Howard Hinnant) — inverse of the civil-from-days above.
    let y = if month <= 2 { year - 1 } else { year };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = if month > 2 { month - 3 } else { month + 9 }; // [0, 11]
    let doy = (153 * mp + 2) / 5 + day - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    let days = era * 146_097 + doe - 719_468;
    if days < 0 {
        return None;
    }
    let secs = days as u64 * 86_400 + hour as u64 * 3_600 + minute as u64 * 60 + second as u64;
    Some(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_epochs() {
        assert_eq!(iso8601_from_unix(0), "1970-01-01T00:00:00Z");
        // 2026-06-08T12:00:00Z = 1780920000
        assert_eq!(iso8601_from_unix(1_780_920_000), "2026-06-08T12:00:00Z");
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("7d"), Some(604_800));
        assert_eq!(parse_duration("12h"), Some(43_200));
        assert_eq!(parse_duration("30m"), Some(1_800));
        assert_eq!(parse_duration("2w"), Some(1_209_600));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("d"), None);
        assert_eq!(parse_duration("7x"), None);
        assert_eq!(parse_duration("abc"), None);
    }

    #[test]
    fn unix_iso_roundtrip() {
        for secs in [0u64, 1_780_920_000, 1_000_000_000, 86_400] {
            let iso = iso8601_from_unix(secs);
            assert_eq!(unix_from_iso8601(&iso), Some(secs), "roundtrip {iso}");
        }
        assert_eq!(unix_from_iso8601("not-a-date"), None);
        assert_eq!(
            unix_from_iso8601("2026-06-08T12:00:00.123Z"),
            Some(1_780_920_000)
        );
    }
}
