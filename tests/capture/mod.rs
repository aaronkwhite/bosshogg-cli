//! Integration tests for `bosshogg capture` subcommands.
//!
//! Tests verify the public endpoint shape (/i/v0/e, /batch) and that
//! project_token (phc_) is required — the personal key (phx_) is not sent.

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn write_cfg_with_token(tmp: &TempDir, server_uri: &str) -> std::path::PathBuf {
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            r#"current_context = "t"
[contexts.t]
host = "{server_uri}"
api_key = "phx_testkey"
project_token = "phc_projecttoken123"
project_id = "999999"
env_id = "999999"
"#
        ),
    )
    .unwrap();
    cfg
}

fn write_cfg_without_token(tmp: &TempDir, server_uri: &str) -> std::path::PathBuf {
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            r#"current_context = "t"
[contexts.t]
host = "{server_uri}"
api_key = "phx_testkey"
project_id = "999999"
env_id = "999999"
"#
        ),
    )
    .unwrap();
    cfg
}

// ── 1. capture event success ──────────────────────────────────────────────────

#[tokio::test]
async fn capture_event_posts_to_ingest_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/i/v0/e"))
        .and(body_partial_json(json!({
            "api_key": "phc_projecttoken123",
            "event": "page_view",
            "distinct_id": "user-abc"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": 1})))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg_with_token(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "capture",
            "event",
            "--event",
            "page_view",
            "--distinct-id",
            "user-abc",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"status\":1"));
}

// ── 2. capture event with properties file ────────────────────────────────────

#[tokio::test]
async fn capture_event_with_properties_file() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/i/v0/e"))
        .and(body_partial_json(json!({
            "event": "sign_up",
            "distinct_id": "user-xyz",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": 1})))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg_with_token(&tmp, &server.uri());
    let props_file = tmp.path().join("props.json");
    std::fs::write(&props_file, r#"{"plan": "pro", "referrer": "google"}"#).unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "capture",
            "event",
            "--event",
            "sign_up",
            "--distinct-id",
            "user-xyz",
            "--properties-file",
            props_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"status\":1"));
}

// ── 3. capture batch ──────────────────────────────────────────────────────────

#[tokio::test]
async fn capture_batch_posts_to_batch_endpoint() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/batch"))
        .and(body_partial_json(json!({
            "api_key": "phc_projecttoken123",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": 1})))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg_with_token(&tmp, &server.uri());
    let events_file = tmp.path().join("events.jsonl");
    std::fs::write(
        &events_file,
        "{\"event\": \"click\", \"distinct_id\": \"u1\"}\n{\"event\": \"view\", \"distinct_id\": \"u2\"}\n",
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "capture",
            "batch",
            "--file",
            events_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"status\":1"));
}

// ── 4. capture identify ───────────────────────────────────────────────────────

#[tokio::test]
async fn capture_identify_posts_identify_event() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/i/v0/e"))
        .and(body_partial_json(json!({
            "event": "$identify",
            "distinct_id": "alice@example.com",
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": 1})))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg_with_token(&tmp, &server.uri());
    let props_file = tmp.path().join("person.json");
    std::fs::write(&props_file, r#"{"name": "Alice", "plan": "enterprise"}"#).unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "capture",
            "identify",
            "--distinct-id",
            "alice@example.com",
            "--properties-file",
            props_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"status\":1"));
}

// ── 5. missing project_token fails with clear error ───────────────────────────

#[tokio::test]
async fn capture_event_missing_project_token_fails() {
    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg_without_token(&tmp, "http://127.0.0.1:19997");

    let out = Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "capture",
            "event",
            "--event",
            "test",
            "--distinct-id",
            "u1",
        ])
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "should fail when project_token is missing"
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("project_token") || stderr.contains("phc_"),
        "error should mention project_token: {stderr}"
    );
}

// ── 6. capture without --yes blocked in non-tty ───────────────────────────────

#[tokio::test]
async fn capture_event_without_yes_blocked() {
    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg_with_token(&tmp, "http://127.0.0.1:19998");

    let out = Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        // NOT passing --yes
        .args(["capture", "event", "--event", "test", "--distinct-id", "u1"])
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "capture without --yes should fail in non-TTY"
    );
}

// ── 7. batch with empty file fails ───────────────────────────────────────────

#[tokio::test]
async fn capture_batch_empty_file_fails() {
    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg_with_token(&tmp, "http://127.0.0.1:19999");
    let events_file = tmp.path().join("empty.jsonl");
    std::fs::write(&events_file, "").unwrap();

    let out = Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "capture",
            "batch",
            "--file",
            events_file.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!out.status.success(), "batch with empty file should fail");
}
