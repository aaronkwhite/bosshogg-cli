//! HogQL end-to-end smoke test.
//!
//! Trivially verifies the entire query pathway:
//!   auth resolution → client → /query/ → response shape → JSON output
//!
//! Runs live; skipped in PR CI. Paired with `live.rs` in scheduled runs.

use std::env;

use assert_cmd::Command;
use serde_json::Value;

fn token() -> Option<String> {
    env::var("POSTHOG_CLI_TOKEN")
        .or_else(|_| env::var("POSTHOG_CLI_API_KEY"))
        .ok()
}

#[test]
#[ignore] // live
fn select_one_roundtrip() {
    let Some(tok) = token() else {
        panic!("POSTHOG_CLI_TOKEN required");
    };

    let out = Command::cargo_bin("bosshogg")
        .unwrap()
        .env_clear()
        .env("POSTHOG_CLI_TOKEN", tok)
        .env("POSTHOG_CLI_PROJECT_ID", "999999")
        .env("POSTHOG_CLI_HOST", "https://us.posthog.com")
        .env("HOME", env::temp_dir())
        .args(["query", "run", "SELECT 1", "--json"])
        .output()
        .expect("spawn");
    assert!(
        out.status.success(),
        "exit={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8(out.stdout).unwrap();
    let v: Value = serde_json::from_str(&stdout).expect("json");

    let results = v
        .get("results")
        .and_then(|r| r.as_array())
        .expect("results");
    assert_eq!(results.len(), 1);
    let row = results[0].as_array().expect("row array");
    assert_eq!(row.len(), 1);
    assert_eq!(row[0].as_i64(), Some(1));

    // columns+types are informational — assert their presence, not values
    assert!(v.get("columns").is_some());
    assert!(v.get("types").is_some());
}

#[test]
#[ignore] // live
fn select_one_roundtrip_via_stdin() {
    let Some(tok) = token() else {
        panic!("POSTHOG_CLI_TOKEN required");
    };

    let mut cmd = Command::cargo_bin("bosshogg").unwrap();
    cmd.env_clear()
        .env("POSTHOG_CLI_TOKEN", tok)
        .env("POSTHOG_CLI_PROJECT_ID", "999999")
        .env("POSTHOG_CLI_HOST", "https://us.posthog.com")
        .env("HOME", env::temp_dir())
        .args(["query", "run", "--file", "-", "--json"])
        .write_stdin("SELECT 1");
    let out = cmd.output().expect("spawn");
    assert!(
        out.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&out.stderr)
    );
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["results"][0][0].as_i64(), Some(1));
}
