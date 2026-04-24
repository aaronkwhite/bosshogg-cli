//! Output subsystem.
//!
//! Every command emits output through exactly one of:
//! - `print_json` — compact, envelope-free (agent-friendly)
//! - `table::print` — TTY-only, comfy-table
//! - `print_error` — structured when `--json`, colored line otherwise
//!
//! Command code MUST NOT call `serde_json::to_string` or `println!("{}", ...)`
//! directly. Discipline here is how we keep the JSON contract stable.

pub mod color;
pub mod interactive;
pub mod safe;
pub mod table;

use serde::Serialize;
use serde_json::{Value, json};

use crate::BosshoggError;

pub fn print_json<T: Serialize>(value: &T) {
    let s = render_json_string(value);
    use std::io::Write;
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(out, "{s}");
}

fn render_json_string<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "{}".to_string())
}

pub fn is_interactive() -> bool {
    console::Term::stdout().is_term()
}

pub fn print_error(err: &BosshoggError, json_mode: bool) {
    use std::io::Write;
    let stderr = std::io::stderr();
    let mut out = stderr.lock();

    if json_mode || !console::Term::stderr().is_term() {
        let v = render_error_value(err);
        let _ = writeln!(out, "{}", render_json_string(&v));
        return;
    }

    let msg = format!("error: {err}");
    let _ = writeln!(out, "{}", color::red(&msg));
    if let Some(hint) = hint_for(err) {
        let _ = writeln!(out, "{}", color::dim(&format!("hint: {hint}")));
    }
}

/// Returns suggested flags an agent can retry with for the given error, or `None`
/// if no automated retry path exists.
///
/// Currently only the `query` rate-limit bucket has an async escape hatch.
/// Other buckets (e.g. `crud`) require the caller to wait and retry bare.
fn retry_with_for(err: &BosshoggError) -> Option<Vec<&'static str>> {
    if let BosshoggError::RateLimit { bucket, .. } = err {
        if bucket == "query" {
            return Some(vec!["--async"]);
        }
    }
    None
}

fn render_error_value(err: &BosshoggError) -> Value {
    let mut v = json!({
        "error": true,
        "code": err.error_code(),
        "message": err.to_string(),
    });

    if let Some(hint) = hint_for(err) {
        v["hint"] = Value::String(hint.into());
    }
    if let Some(r) = err.retry_after_s() {
        v["retry_after_s"] = Value::from(r);
    }
    if let Some(suggestions) = retry_with_for(err) {
        v["retry_with"] = serde_json::json!(suggestions);
    }
    v
}

fn hint_for(err: &BosshoggError) -> Option<&'static str> {
    use BosshoggError::*;
    Some(match err {
        MissingApiKey => "Run `bosshogg configure` or set POSTHOG_CLI_TOKEN.",
        InvalidApiKey => {
            "Key rejected — rotate via PostHog settings and re-run `bosshogg configure`."
        }
        MissingScope { .. } => {
            "Rotate the key with the required scope in PostHog → Settings → Personal API keys."
        }
        RateLimit { bucket, .. } if bucket == "query" => {
            "Query bucket is team-wide (2400/hr). Wait, or pass --async to queue."
        }
        RateLimit { .. } => "Rate limits are team-wide — rotating keys does not help.",
        NotFound(_) => {
            "Check the identifier and your current context (`bosshogg config current-context`)."
        }
        HogQL(_) => "Inspect the SQL; `bosshogg schema hogql` lists columns for the active project.",
        Config(_) => {
            "Run `bosshogg configure`, or set the env var named in the message (POSTHOG_CLI_*)."
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::BosshoggError;

    #[test]
    fn print_json_compact_no_envelope() {
        let v = serde_json::json!({"id": 1, "key": "foo"});
        let s = render_json_string(&v);
        assert_eq!(s, r#"{"id":1,"key":"foo"}"#);
    }

    #[test]
    fn render_error_json_shape_matches_conventions() {
        let err = BosshoggError::RateLimit {
            retry_after_s: 47,
            bucket: "query".into(),
        };
        let v = render_error_value(&err);
        assert_eq!(v["error"], true);
        assert_eq!(v["code"], "RATE_LIMITED");
        assert_eq!(v["retry_after_s"], 47);
        assert!(v["message"].as_str().unwrap().contains("rate limited"));
    }

    #[test]
    fn render_error_omits_retry_after_when_none() {
        let err = BosshoggError::NotFound("x".into());
        let v = render_error_value(&err);
        assert!(v.get("retry_after_s").is_none());
    }

    // --- retry_with tests (Opus review critical #6) ---

    #[test]
    fn render_error_query_rate_limit_emits_retry_with_and_retry_after() {
        let err = BosshoggError::RateLimit {
            bucket: "query".into(),
            retry_after_s: 47,
        };
        let v = render_error_value(&err);
        assert_eq!(v["retry_after_s"], 47);
        let retry_with = v["retry_with"]
            .as_array()
            .expect("retry_with should be an array");
        assert_eq!(retry_with, &[serde_json::json!("--async")]);
    }

    #[test]
    fn render_error_non_query_rate_limit_omits_retry_with() {
        let err = BosshoggError::RateLimit {
            bucket: "crud".into(),
            retry_after_s: 1,
        };
        let v = render_error_value(&err);
        // retry_after_s should still be present
        assert_eq!(v["retry_after_s"], 1);
        // but retry_with should NOT be present for non-query buckets
        assert!(v.get("retry_with").is_none());
    }

    #[test]
    fn render_error_not_found_omits_retry_with_and_retry_after() {
        let err = BosshoggError::NotFound("x".into());
        let v = render_error_value(&err);
        assert!(v.get("retry_with").is_none());
        assert!(v.get("retry_after_s").is_none());
    }
}
