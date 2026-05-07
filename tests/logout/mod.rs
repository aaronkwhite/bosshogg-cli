//! Integration tests for `bosshogg logout` — local credential removal.
//!
//! Logout is purely local (no HTTP), so these are filesystem-only tests
//! against an isolated `BOSSHOGG_CONFIG` path.

use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;
use tempfile::TempDir;

const PRE_FILLED_CONFIG: &str = r#"current_context = "us"

[contexts.us]
host = "https://us.posthog.com"
region = "us"
api_key = "phx_us_key"
project_id = "999999"
env_id = "999999"

[contexts.eu]
host = "https://eu.posthog.com"
region = "eu"
api_key = "phx_eu_key"
project_id = "888888"
env_id = "888888"
"#;

fn write_config(path: &std::path::Path) {
    std::fs::write(path, PRE_FILLED_CONFIG).unwrap();
}

fn bh(cfg: &std::path::Path) -> Command {
    let mut c = Command::cargo_bin("bosshogg").unwrap();
    c.env("BOSSHOGG_CONFIG", cfg)
        .env_remove("POSTHOG_CLI_TOKEN")
        .env_remove("POSTHOG_CLI_API_KEY")
        .env_remove("POSTHOG_API_KEY");
    c
}

#[test]
fn logout_default_removes_current_context() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_config(&cfg);

    bh(&cfg).arg("logout").assert().success();

    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        !saved.contains("phx_us_key"),
        "us context should be gone:\n{saved}"
    );
    assert!(
        saved.contains("phx_eu_key"),
        "eu context should remain:\n{saved}"
    );
    assert!(
        !saved.contains("current_context = \"us\""),
        "current_context should be cleared:\n{saved}"
    );
}

#[test]
fn logout_specific_context_leaves_current_alone() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_config(&cfg);

    bh(&cfg)
        .args(["logout", "--context", "eu"])
        .assert()
        .success();

    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        !saved.contains("phx_eu_key"),
        "eu context should be gone:\n{saved}"
    );
    assert!(
        saved.contains("phx_us_key"),
        "us context should remain:\n{saved}"
    );
    assert!(
        saved.contains("current_context = \"us\""),
        "current_context = us should still be set:\n{saved}"
    );
}

#[test]
fn logout_all_wipes_every_context() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_config(&cfg);

    bh(&cfg).args(["logout", "--all"]).assert().success();

    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        !saved.contains("phx_us_key") && !saved.contains("phx_eu_key"),
        "no api keys should remain:\n{saved}"
    );
    assert!(
        !saved.contains("current_context = "),
        "current_context should be cleared:\n{saved}"
    );
}

#[test]
fn logout_unknown_context_errors() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_config(&cfg);

    bh(&cfg)
        .args(["logout", "--context", "nonexistent"])
        .assert()
        .failure()
        .stderr(contains("nonexistent"));
}

#[test]
fn logout_with_no_current_and_no_args_errors() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "").unwrap();

    bh(&cfg)
        .arg("logout")
        .assert()
        .failure()
        .stderr(contains("no current context"));
}

#[test]
fn logout_json_output_matches_schema() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_config(&cfg);

    let out = bh(&cfg)
        .args(["logout", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let parsed: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(parsed["ok"], serde_json::json!(true));
    assert!(parsed["deleted"].is_array());
    let deleted: Vec<String> = parsed["deleted"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert_eq!(deleted, vec!["us".to_string()]);
    assert!(parsed["note"].as_str().unwrap().contains("PostHog"));
}

#[test]
fn logout_all_on_empty_config_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "").unwrap();

    bh(&cfg)
        .args(["logout", "--all", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"deleted\":[]"));
}

#[test]
fn login_then_logout_round_trip() {
    // Mirrors the on-stage demo loop: a saved context disappears after logout.
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    write_config(&cfg);

    // pre-condition
    bh(&cfg)
        .args(["config", "get-contexts", "--json"])
        .assert()
        .success()
        .stdout(contains("\"us\""));

    // logout
    bh(&cfg).arg("logout").assert().success();

    // post-condition
    bh(&cfg)
        .args(["config", "get-contexts", "--json"])
        .assert()
        .success()
        .stdout(contains("\"contexts\":[{\"name\":\"eu\""))
        .stdout(predicates::str::contains("\"us\"").not());
}
