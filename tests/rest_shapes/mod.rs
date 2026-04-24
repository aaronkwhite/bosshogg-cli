//! REST-shape tests.
//!
//! Every typed serde struct that BossHogg exposes as a `--json` surface must
//! deserialize a real PostHog response fixture. Fixtures live in
//! `tests/fixtures/` and are considered golden. Re-record with:
//!
//! ```bash
//! POSTHOG_CLI_TOKEN=phx_... \
//! POSTHOG_CLI_PROJECT_ID=999999 \
//! BOSSHOGG_RECORD_FIXTURES=1 \
//! cargo test --test rest_shapes -- --ignored record_
//! ```
//!
//! The `record_` tests are `#[ignore]` so they never run in PR CI. They hit
//! the real API, sanitize the response, and overwrite the fixture on disk.

use std::fs;
use std::path::PathBuf;

use serde::de::DeserializeOwned;
use serde_json::Value;

use bosshogg::client::query::QueryResponse;
use bosshogg::commands::flag::Flag;

/// Load and deserialize a fixture. Panics with a helpful message if the
/// JSON doesn't fit the struct — that's the entire point of this test.
fn round_trip<T: DeserializeOwned>(fixture: &str) -> T {
    let path: PathBuf = ["tests", "fixtures", &format!("{fixture}.json")]
        .iter()
        .collect();
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("missing fixture {path:?}: {e}"));
    serde_json::from_slice::<T>(&bytes).unwrap_or_else(|e| {
        // Dump the parsed JSON for context, then the serde error.
        let as_value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        panic!(
            "fixture {fixture} failed to deserialize into {}:\n  {e}\n  json = {as_value}",
            std::any::type_name::<T>()
        );
    })
}

fn load_fixture_value(fixture: &str) -> Value {
    let path: PathBuf = ["tests", "fixtures", &format!("{fixture}.json")]
        .iter()
        .collect();
    let bytes = fs::read(&path).unwrap_or_else(|e| panic!("missing fixture {path:?}: {e}"));
    serde_json::from_slice(&bytes)
        .unwrap_or_else(|e| panic!("fixture {fixture} is not valid JSON: {e}"))
}

#[test]
fn flag_list_deserializes() {
    let list = load_fixture_value("flag_list");
    let results = list
        .get("results")
        .and_then(|r| r.as_array())
        .expect("results array");
    let count = list.get("count").and_then(|c| c.as_u64()).expect("count");
    assert!(count >= results.len() as u64);
    assert!(!results.is_empty(), "fixture should include rows");
    // Verify each result is a Flag
    for result in results {
        let _flag: Flag =
            serde_json::from_value(result.clone()).expect("each result should deserialize as Flag");
    }
}

#[test]
fn flag_get_deserializes() {
    let flag: Flag = round_trip("flag_get");
    assert!(!flag.key.is_empty(), "flag.key should be populated");
}

#[test]
fn users_me_deserializes() {
    let user = load_fixture_value("users_me");
    let email = user.get("email").and_then(|e| e.as_str()).expect("email");
    assert!(!email.is_empty());
}

#[test]
fn query_response_deserializes() {
    let resp: QueryResponse = round_trip("query_response");
    assert!(!resp.columns.is_empty());
    assert!(!resp.results.is_empty());
}

#[test]
fn async_query_status_deserializes() {
    let resp: QueryResponse = round_trip("async_query_status");
    assert!(resp.columns.is_empty() || !resp.results.is_empty());
}

#[test]
fn doctor_report_deserializes() {
    let report = load_fixture_value("doctor_checks");
    let checks = report
        .get("checks")
        .and_then(|c| c.as_array())
        .expect("checks");
    assert!(!checks.is_empty());
}

#[test]
fn hogql_schema_deserializes() {
    let schema = load_fixture_value("hogql_schema");
    let tables = schema
        .get("tables")
        .and_then(|t| t.as_array())
        .expect("tables");
    assert!(!tables.is_empty(), "expected at least one table");
}

// -------------------------------------------------------------------------
// Fixture recording (opt-in).
// -------------------------------------------------------------------------
//
// These tests are #[ignore] so PR CI never runs them. They:
//   1. Require POSTHOG_CLI_TOKEN + POSTHOG_CLI_PROJECT_ID + (optional) HOST.
//   2. Hit the real PostHog API for a single resource.
//   3. Sanitize PII (see sanitize_json).
//   4. Write the response to tests/fixtures/<name>.json.
//
// Enabled only when BOSSHOGG_RECORD_FIXTURES=1.

use std::env;

fn record_guard() -> Option<(String, String, String)> {
    if env::var("BOSSHOGG_RECORD_FIXTURES").ok().as_deref() != Some("1") {
        return None;
    }
    let token = env::var("POSTHOG_CLI_TOKEN")
        .or_else(|_| env::var("POSTHOG_CLI_API_KEY"))
        .expect("POSTHOG_CLI_TOKEN required to record fixtures");
    let project = env::var("POSTHOG_CLI_PROJECT_ID")
        .expect("POSTHOG_CLI_PROJECT_ID required (use 999999 for dogfood)");
    let host =
        env::var("POSTHOG_CLI_HOST").unwrap_or_else(|_| "https://us.posthog.com".to_string());
    Some((token, project, host))
}

/// Remove or redact fields that could leak PII or per-tenant identifiers.
/// We keep shape and types; we strip values.
fn sanitize_json(mut v: Value) -> Value {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(map) => {
                for (k, val) in map.iter_mut() {
                    if matches!(
                        k.as_str(),
                        "email"
                            | "first_name"
                            | "last_name"
                            | "distinct_id"
                            | "user_email"
                            | "created_by_email"
                    ) {
                        *val = Value::String("redacted@example.com".into());
                    } else if matches!(k.as_str(), "ip" | "remote_ip") {
                        *val = Value::String("0.0.0.0".into());
                    } else {
                        walk(val);
                    }
                }
            }
            Value::Array(arr) => arr.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut v);
    v
}

fn write_fixture(name: &str, value: &Value) {
    let path: PathBuf = ["tests", "fixtures", &format!("{name}.json")]
        .iter()
        .collect();
    let pretty = serde_json::to_string_pretty(value).expect("re-serialize");
    fs::write(&path, pretty).expect("write fixture");
    eprintln!("recorded {}", path.display());
}

async fn record(name: &str, url: &str) {
    let Some((token, _project, host)) = record_guard() else {
        return;
    };
    let client = reqwest::Client::builder().https_only(true).build().unwrap();
    let full = format!("{host}{url}");
    let resp = client
        .get(&full)
        .bearer_auth(&token)
        .send()
        .await
        .expect("request");
    assert!(
        resp.status().is_success(),
        "recording {name}: {}",
        resp.status()
    );
    let v: Value = resp.json().await.expect("json");
    write_fixture(name, &sanitize_json(v));
}

#[tokio::test]
#[ignore]
async fn record_flag_list() {
    let Some((_, project, _)) = record_guard() else {
        return;
    };
    record(
        "flag_list",
        &format!("/api/projects/{project}/feature_flags/?limit=5"),
    )
    .await;
}

#[tokio::test]
#[ignore]
async fn record_users_me() {
    record("users_me", "/api/users/@me/").await;
}
