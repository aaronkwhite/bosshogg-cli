//! Output safety rules — the guardrails against context-window nukes.
//!
//! ## HogQL auto-LIMIT
//!
//! `bosshogg query run "SELECT * FROM events"` would cheerfully return
//! 10 million rows. We inject `LIMIT 100` when:
//! - the parsed SQL has no `LIMIT` clause (outside comments/strings), and
//! - the caller did not pass `--no-limit`.
//!
//! This uses a tiny hand-rolled tokenizer (comment + string-literal aware).
//! We deliberately DO NOT take a full HogQL parser dependency — a slightly
//! over-eager LIMIT is always safe. If in doubt, we add one.

pub fn inject_hogql_limit(sql: &str) -> String {
    if has_top_level_limit(sql) {
        return sql.to_string();
    }
    let trimmed = sql.trim_end_matches(|c: char| c.is_whitespace() || c == ';');
    format!("{trimmed}\nLIMIT 100")
}

fn has_top_level_limit(sql: &str) -> bool {
    let bytes = sql.as_bytes();
    let mut i = 0usize;
    let n = bytes.len();

    while i < n {
        let c = bytes[i];

        if c == b'-' && i + 1 < n && bytes[i + 1] == b'-' {
            while i < n && bytes[i] != b'\n' {
                i += 1;
            }
            continue;
        }

        if c == b'/' && i + 1 < n && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < n && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(n);
            continue;
        }

        if matches!(c, b'\'' | b'"' | b'`') {
            let quote = c;
            i += 1;
            while i < n && bytes[i] != quote {
                if bytes[i] == b'\\' && i + 1 < n {
                    i += 2;
                } else {
                    i += 1;
                }
            }
            i = (i + 1).min(n);
            continue;
        }

        if (c == b'L' || c == b'l') && matches_keyword_ci(bytes, i, b"LIMIT") {
            let before = if i == 0 { b' ' } else { bytes[i - 1] };
            let after = if i + 5 < n { bytes[i + 5] } else { b' ' };
            if !is_ident_byte(before) && !is_ident_byte(after) {
                return true;
            }
        }

        i += 1;
    }
    false
}

fn matches_keyword_ci(bytes: &[u8], start: usize, kw: &[u8]) -> bool {
    if start + kw.len() > bytes.len() {
        return false;
    }
    for (a, b) in bytes[start..start + kw.len()].iter().zip(kw.iter()) {
        if a.to_ascii_uppercase() != *b {
            return false;
        }
    }
    true
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_limit_100_when_absent() {
        assert_eq!(
            inject_hogql_limit("SELECT count() FROM events"),
            "SELECT count() FROM events\nLIMIT 100"
        );
    }

    #[test]
    fn keeps_existing_limit_uppercase() {
        let sql = "SELECT 1 FROM events LIMIT 50";
        assert_eq!(inject_hogql_limit(sql), sql);
    }

    #[test]
    fn keeps_existing_limit_lowercase() {
        let sql = "select event from events limit 10";
        assert_eq!(inject_hogql_limit(sql), sql);
    }

    #[test]
    fn ignores_limit_inside_string_literal() {
        let sql = "SELECT 'no LIMIT here' AS s FROM events";
        let out = inject_hogql_limit(sql);
        assert!(out.ends_with("LIMIT 100"), "got: {out}");
    }

    #[test]
    fn respects_trailing_semicolon() {
        assert_eq!(
            inject_hogql_limit("SELECT 1 FROM events;"),
            "SELECT 1 FROM events\nLIMIT 100"
        );
    }

    #[test]
    fn ignores_comment_limit() {
        let sql = "-- LIMIT 5\nSELECT 1 FROM events";
        let out = inject_hogql_limit(sql);
        assert!(out.ends_with("LIMIT 100"), "got: {out}");
    }
}
