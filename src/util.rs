//! Small shared helpers.
//!
//! - `is_short_id` distinguishes a PostHog short-ID (6–8 alphanumeric) from
//!   a human-friendly key or numeric ID. Used for routing lookups to the
//!   correct REST path.
//! - `parse_since` accepts ISO-8601 dates, RFC3339 timestamps, and relative
//!   strings like `7d`, `2h`, `30m`. Returns UTC.
//! - `redact_key` is the only way an API key is allowed to appear in logs.

use chrono::{DateTime, Duration, NaiveDate, Utc};

use crate::error::{BosshoggError, Result};

pub fn is_short_id(s: &str) -> bool {
    // PostHog short_ids are 6–8 alphanumeric characters AND always contain
    // at least one letter — numeric-only strings are numeric IDs, not short_ids.
    // Without the letter guard, a numeric id like `8156147` would be misrouted
    // through the short_id lookup path (list+filter by short_id) and fail
    // with NOT_FOUND instead of being parsed as `8_156_147`.
    let len = s.len();
    (6..=8).contains(&len)
        && s.chars().all(|c| c.is_ascii_alphanumeric())
        && s.chars().any(|c| c.is_ascii_alphabetic())
}

pub fn parse_since(input: &str) -> Result<DateTime<Utc>> {
    if let Some((num, unit)) = split_relative(input) {
        let n: i64 = num
            .parse()
            .map_err(|_| BosshoggError::BadRequest(format!("bad relative time: {input}")))?;
        let dur = match unit {
            "d" => Duration::days(n),
            "h" => Duration::hours(n),
            "m" => Duration::minutes(n),
            "s" => Duration::seconds(n),
            _ => {
                return Err(BosshoggError::BadRequest(format!(
                    "unknown unit in {input}"
                )));
            }
        };
        return Ok(Utc::now() - dur);
    }

    if let Ok(t) = DateTime::parse_from_rfc3339(input) {
        return Ok(t.with_timezone(&Utc));
    }

    if let Ok(d) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        return Ok(d.and_hms_opt(0, 0, 0).unwrap().and_utc());
    }

    Err(BosshoggError::BadRequest(format!(
        "unrecognized time: {input} (try `7d`, `2026-04-01`, or RFC3339)"
    )))
}

fn split_relative(s: &str) -> Option<(&str, &str)> {
    let last = s.chars().last()?;
    if !"dhms".contains(last) {
        return None;
    }
    let (num, unit) = s.split_at(s.len() - 1);
    if num.chars().all(|c| c.is_ascii_digit()) && !num.is_empty() {
        Some((num, unit))
    } else {
        None
    }
}

/// Redact an API key for log output. Keeps the prefix (`phx_`, `phc_`, etc.)
/// and the last four characters.
pub fn redact_key(key: &str) -> String {
    if key.len() < 8 {
        return "***".into();
    }
    let prefix_end = key.find('_').map(|i| i + 1).unwrap_or(0);
    let (prefix, rest) = key.split_at(prefix_end);
    let tail = &rest[rest.len().saturating_sub(4)..];
    format!("{prefix}***{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_id_vs_key() {
        assert!(is_short_id("aB3xY9"));
        assert!(is_short_id("xYz12a34"));
        assert!(!is_short_id("my-feature-flag"));
        assert!(!is_short_id("123"));
        assert!(!is_short_id("has_underscore"));
        // Numeric IDs of short-id length must NOT be classified as short_ids —
        // they are numeric IDs and must route to the direct path.
        assert!(!is_short_id("123456"));
        assert!(!is_short_id("8156147"));
        assert!(!is_short_id("12345678"));
    }

    #[test]
    fn parse_since_relative() {
        let now = chrono::Utc::now();
        let t = parse_since("7d").unwrap();
        let diff = now.signed_duration_since(t).num_days();
        assert!((6..=8).contains(&diff), "got {diff} days");

        let t = parse_since("2h").unwrap();
        let diff = now.signed_duration_since(t).num_hours();
        assert!((1..=3).contains(&diff));
    }

    #[test]
    fn parse_since_rfc3339() {
        let t = parse_since("2026-01-01T00:00:00Z").unwrap();
        assert_eq!(t.format("%Y-%m-%d").to_string(), "2026-01-01");
    }

    #[test]
    fn parse_since_iso_date() {
        let t = parse_since("2026-04-01").unwrap();
        assert_eq!(t.format("%Y-%m-%d").to_string(), "2026-04-01");
    }

    #[test]
    fn redact_key_keeps_prefix_and_last_four() {
        assert_eq!(redact_key("phx_abcdef1234567890"), "phx_***7890");
        assert_eq!(redact_key("short"), "***");
        assert_eq!(redact_key(""), "***");
    }
}
