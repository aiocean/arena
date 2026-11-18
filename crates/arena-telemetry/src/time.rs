//! Time as the files keep it: microseconds since the Unix epoch, written as UTC (`2026-11-18T10:00:00.123456Z`), a
//! file a UTC day (`2026-11-18`). No time zone is ever read: every date here is UTC.

use std::time::{SystemTime, UNIX_EPOCH};

/// A second, in microseconds.
pub const SECOND: i64 = 1_000_000;
/// A day, in microseconds.
pub const DAY: i64 = 86_400 * SECOND;

/// Now, microseconds since the epoch.
pub fn now_us() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros() as i64)
}

/// The days since the epoch of a civil date (Howard Hinnant's days-from-civil).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day since the epoch (civil-from-days).
pub fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// The UTC day of `us`, as a file is named: `2026-11-18`.
pub fn day(us: i64) -> String {
    let (y, m, d) = civil_from_days(us.div_euclid(DAY));
    format!("{y:04}-{m:02}-{d:02}")
}

/// `us` as a line's `ts`: `2026-11-18T10:00:00.123456Z`, always 27 characters (years 0 to 9999).
pub fn format_ts(us: i64) -> String {
    let rem = us.rem_euclid(DAY);
    let s = rem / SECOND;
    format!(
        "{}T{:02}:{:02}:{:02}.{:06}Z",
        day(us),
        s / 3600,
        s % 3600 / 60,
        s % 60,
        rem % SECOND
    )
}

/// The length of a `ts` as `format_ts` writes it.
pub const TS_LEN: usize = 27;

/// A UTC time: `2026-11-18`, `2026-11-18T10`, `2026-11-18T10:05`, `2026-11-18T10:05:30`, with a fraction or not, a
/// `T` or a space, a trailing `Z` or not.
pub fn parse_ts(s: &str) -> Option<i64> {
    let s = s.trim().trim_end_matches('Z');
    let (date, clock) = match s.split_once(['T', ' ']) {
        Some((d, c)) => (d, c),
        None => (s, ""),
    };
    let mut ymd = date.splitn(3, '-');
    let y: i64 = ymd.next()?.parse().ok()?;
    let m: u32 = ymd.next()?.parse().ok()?;
    let d: u32 = ymd.next()?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let mut us = days_from_civil(y, m, d) * DAY;
    if !clock.is_empty() {
        let (hms, frac) = clock.split_once('.').unwrap_or((clock, ""));
        let mut parts = hms.splitn(3, ':');
        let h: i64 = parts.next()?.parse().ok()?;
        let mi: i64 = parts.next().map_or(Ok(0), str::parse).ok()?;
        let se: i64 = parts.next().map_or(Ok(0), str::parse).ok()?;
        if h > 23 || mi > 59 || se > 60 {
            return None;
        }
        us += (h * 3600 + mi * 60 + se) * SECOND;
        if !frac.is_empty() {
            if !frac.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            let digits: String = frac.chars().chain(std::iter::repeat('0')).take(6).collect();
            us += digits.parse::<i64>().ok()?;
        }
    }
    Some(us)
}

/// A span of time: `90s`, `15m`, `2h`, `1d`, `2w` (a bare number is seconds), in microseconds.
pub fn parse_duration(s: &str) -> Option<i64> {
    let s = s.trim();
    let split = s.find(|c: char| !(c.is_ascii_digit() || c == '.'))?;
    let (n, unit) = if split == 0 {
        return None;
    } else {
        s.split_at(split)
    };
    let n: f64 = n.parse().ok()?;
    let unit = match unit {
        "ms" => 0.001,
        "s" => 1.0,
        "m" => 60.0,
        "h" => 3600.0,
        "d" => 86_400.0,
        "w" => 604_800.0,
        _ => return None,
    };
    Some((n * unit * SECOND as f64) as i64)
}

/// As `parse_duration`, a bare number taken as seconds.
pub fn parse_ago(s: &str) -> Option<i64> {
    s.trim()
        .parse::<f64>()
        .ok()
        .map(|n| (n * SECOND as f64) as i64)
        .or_else(|| parse_duration(s))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn days_round_trip_and_match_known_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 2, 29), 11_016);
        for d in [-1000, 0, 11_016, 20_775, 30_000] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
    }

    #[test]
    fn ts_formats_and_parses_back() {
        assert_eq!(format_ts(0), "1970-01-01T00:00:00.000000Z");
        let t = 1_794_942_538_123_456;
        let s = format_ts(t);
        assert_eq!(s, "2026-11-17T19:08:58.123456Z");
        assert_eq!(s.len(), TS_LEN);
        assert_eq!(parse_ts(&s), Some(t));
        assert_eq!(day(t), "2026-11-17");
        assert_eq!(parse_ts("2026-11-17"), Some(t - (t % DAY)));
        assert_eq!(parse_ts("2026-11-17T19:08"), Some(t - 58_123_456));
        assert_eq!(parse_ts("2026-11-17 19:08:58.1"), Some(t - 23_456));
        assert_eq!(parse_ts("2026-13-01"), None);
        assert_eq!(parse_ts("yesterday"), None);
    }

    #[test]
    fn durations_take_units() {
        assert_eq!(parse_duration("90s"), Some(90 * SECOND));
        assert_eq!(parse_duration("15m"), Some(900 * SECOND));
        assert_eq!(parse_duration("2h"), Some(7200 * SECOND));
        assert_eq!(parse_duration("1.5d"), Some(129_600 * SECOND));
        assert_eq!(parse_duration("h"), None);
        assert_eq!(parse_duration("3y"), None);
        assert_eq!(parse_ago("30"), Some(30 * SECOND));
    }
}
