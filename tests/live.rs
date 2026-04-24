//! Live integration tests against PostHog project 999999 (US Cloud).
//!
//! All tests in this file are `#[ignore]`. PR CI does not run them.
//! They run:
//!   - nightly on `main` via GitHub Actions (a future `live.yml` workflow)
//!   - locally with `POSTHOG_CLI_TOKEN=phx_... cargo test --test live -- --ignored`
//!
//! Every test is a READ: no writes, no mutations. Writes are covered by
//! `rest_shapes.rs` against wiremock fixtures.

mod common;

use std::env;

use assert_cmd::Command;
use serde_json::Value;

fn live_env() -> Option<String> {
    let token = env::var("POSTHOG_CLI_TOKEN")
        .or_else(|_| env::var("POSTHOG_CLI_API_KEY"))
        .or_else(|_| env::var("POSTHOG_API_KEY"))
        .ok()?;
    Some(token)
}

fn project_id() -> String {
    env::var("POSTHOG_CLI_PROJECT_ID").unwrap_or_else(|_| "999999".to_string())
}

fn host() -> String {
    env::var("POSTHOG_CLI_HOST").unwrap_or_else(|_| "https://us.posthog.com".to_string())
}

fn env_id() -> Option<String> {
    env::var("POSTHOG_CLI_ENV_ID").ok()
}

fn run_bosshogg(args: &[&str]) -> (std::process::ExitStatus, String, String) {
    let Some(token) = live_env() else {
        panic!("live_env required — call from inside #[ignore] test only");
    };
    let mut cmd = Command::cargo_bin("bosshogg").unwrap();
    cmd.env_clear()
        .env("POSTHOG_CLI_TOKEN", token)
        .env("POSTHOG_CLI_PROJECT_ID", project_id())
        .env("POSTHOG_CLI_HOST", host())
        .env("HOME", std::env::temp_dir());
    if let Some(eid) = env_id() {
        cmd.env("POSTHOG_CLI_ENV_ID", eid);
    }
    let out = cmd.args(args).output().expect("failed to run bosshogg");
    (
        out.status,
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn parse<T: serde::de::DeserializeOwned>(s: &str) -> T {
    serde_json::from_str::<T>(s).unwrap_or_else(|e| panic!("not valid json ({e}): {s}"))
}

#[test]
#[ignore] // live-only
fn live_whoami_returns_email() {
    let (status, stdout, stderr) = run_bosshogg(&["whoami", "--json"]);
    assert!(status.success(), "stderr={stderr}");
    let v: Value = parse(&stdout);
    assert!(
        v.get("email").and_then(|e| e.as_str()).is_some(),
        "missing email: {stdout}"
    );
    assert_eq!(v.get("error"), None, "should not be error envelope");
}

#[test]
#[ignore]
fn live_doctor_reports_ok() {
    let (status, stdout, _) = run_bosshogg(&["doctor", "--json"]);
    assert!(status.success(), "doctor exited non-zero: {stdout}");
    let v: Value = parse(&stdout);
    let summary = v.get("summary").expect("summary missing");
    assert_eq!(
        summary.get("ok").and_then(|o| o.as_bool()),
        Some(true),
        "doctor failed; full output: {stdout}"
    );
}

#[test]
#[ignore]
fn live_flag_list_has_rows() {
    let (status, stdout, stderr) = run_bosshogg(&["flag", "list", "--json", "--limit", "5"]);
    assert!(status.success(), "stderr={stderr}");
    let v: Value = parse(&stdout);
    let results = v
        .get("results")
        .and_then(|r| r.as_array())
        .expect("results array");
    if results.is_empty() {
        eprintln!("live_flag_list_has_rows: project has zero flags; shape-asserting only");
        return;
    }
    let first = &results[0];
    assert!(first.get("key").and_then(|k| k.as_str()).is_some());
    assert!(first.get("id").and_then(|i| i.as_i64()).is_some());
}

#[test]
#[ignore]
fn live_schema_hogql_has_tables() {
    let (status, stdout, stderr) = run_bosshogg(&["schema", "hogql", "--json"]);
    assert!(status.success(), "stderr={stderr}");
    let v: Value = parse(&stdout);
    let tables = v
        .get("tables")
        .and_then(|t| t.as_array())
        .expect("tables array");
    assert!(
        tables.len() >= 5,
        "expected >=5 HogQL tables, got {}",
        tables.len()
    );
    // events is always present
    let has_events = tables
        .iter()
        .any(|t| t.get("name").and_then(|n| n.as_str()) == Some("events"));
    assert!(has_events, "events table missing from schema");
}

#[test]
#[ignore]
fn live_query_run_select_one() {
    let (status, stdout, stderr) = run_bosshogg(&["query", "run", "SELECT 1", "--json"]);
    assert!(status.success(), "stderr={stderr}");
    let v: Value = parse(&stdout);
    let results = v
        .get("results")
        .and_then(|r| r.as_array())
        .expect("results");
    assert_eq!(results.len(), 1);
    let row = results[0].as_array().expect("row is array");
    assert_eq!(row[0].as_i64(), Some(1));
}

#[test]
#[ignore]
fn live_auth_token_prints_phx() {
    let (status, stdout, _) = run_bosshogg(&["auth", "token"]);
    assert!(status.success());
    let trimmed = stdout.trim();
    assert!(
        trimmed.starts_with("phx_"),
        "expected phx_ prefix: {trimmed}"
    );
    assert!(trimmed.len() > 20, "suspiciously short token");
}

#[test]
#[ignore]
fn live_flag_get_by_key() {
    // Pick the first flag from list, then round-trip through `flag get`.
    let (_, list_out, _) = run_bosshogg(&["flag", "list", "--json", "--limit", "1"]);
    let list: Value = parse(&list_out);
    let Some(key) = list["results"][0]["key"].as_str() else {
        eprintln!("live_flag_get_by_key: project has zero flags; skipping round-trip");
        return;
    };

    let (status, stdout, stderr) = run_bosshogg(&["flag", "get", key, "--json"]);
    assert!(status.success(), "stderr={stderr}");
    let flag: Value = parse(&stdout);
    assert_eq!(flag.get("key").and_then(|k| k.as_str()), Some(key));
}
