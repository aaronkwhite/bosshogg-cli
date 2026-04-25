//! Integration tests for `bosshogg alert` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture ────────────────────────────────────────────────────────────

fn alert_fixture(id: &str, name: &str, state: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "enabled": true,
        "state": state,
        "threshold": {"type": "absolute", "bounds": {"lower": 0.0}},
        "created_at": "2026-01-01T00:00:00Z",
        "last_checked_at": "2026-04-01T12:00:00Z",
        "next_check_at": "2026-04-01T13:00:00Z"
    })
}

// ── 1. list ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn alert_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/alerts/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                alert_fixture("alert-uuid-1", "Signup Drop", "Not firing"),
                alert_fixture("alert-uuid-2", "Revenue Spike", "Firing"),
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["alert", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Signup Drop"))
        .stdout(contains("Revenue Spike"));
}

// ── 2. get ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn alert_get_by_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/alerts/alert-uuid-42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(alert_fixture(
            "alert-uuid-42",
            "My Alert",
            "Not firing",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["alert", "get", "alert-uuid-42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"alert-uuid-42\""))
        .stdout(contains("My Alert"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn alert_create_with_required_fields() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/alerts/"))
        .and(body_partial_json(json!({
            "name": "New Alert",
            "insight": 99
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(alert_fixture(
            "alert-uuid-new",
            "New Alert",
            "Not firing",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "alert",
            "create",
            "--name",
            "New Alert",
            "--insight",
            "99",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Alert"));
}

// ── 4. update ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn alert_update_patches_enabled() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/alerts/alert-uuid-55/"))
        .and(body_partial_json(json!({"enabled": false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(alert_fixture(
            "alert-uuid-55",
            "My Alert",
            "Not firing",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "alert",
            "update",
            "alert-uuid-55",
            "--enabled",
            "false",
            "--json",
        ])
        .assert()
        .success();
}

// ── 5. delete (hard delete) ───────────────────────────────────────────────────

#[tokio::test]
async fn alert_delete_issues_hard_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/alerts/alert-uuid-77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "alert", "delete", "alert-uuid-77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("alert-uuid-77"));
}

// ── 6. delete requires --yes ──────────────────────────────────────────────────

#[tokio::test]
async fn alert_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19980"
api_key = "phx_testkey"
project_id = "1"
env_id = "1"
"#,
    )
    .unwrap();

    let output = Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["alert", "delete", "alert-uuid-x", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY"
    );
}
