//! Integration tests for `bosshogg subscription` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn sub_fixture(id: i64, title: &str, target_type: &str, frequency: &str) -> serde_json::Value {
    json!({
        "id": id,
        "title": title,
        "target_type": target_type,
        "target_value": "team@example.com",
        "frequency": frequency,
        "interval": 1,
        "byweekday": null,
        "bysetpos": null,
        "start_date": null,
        "until_date": null,
        "insight": null,
        "dashboard": null,
        "next_delivery_date": "2026-04-22T09:00:00Z",
        "deleted": false,
        "created_by": {"id": 1, "email": "test@example.com"},
        "created_at": "2026-01-01T00:00:00Z"
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn subscription_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/subscriptions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                sub_fixture(1, "Weekly Report", "email", "weekly"),
                sub_fixture(2, "Daily Alert", "slack", "daily")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["subscription", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Weekly Report"))
        .stdout(contains("Daily Alert"));
}

// ── 2. get by numeric id ─────────────────────────────────────────────────────

#[tokio::test]
async fn subscription_get_by_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/subscriptions/42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sub_fixture(
            42,
            "My Subscription",
            "email",
            "weekly",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["subscription", "get", "42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":42"))
        .stdout(contains("My Subscription"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn subscription_create_email() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/subscriptions/"))
        .and(body_partial_json(json!({
            "title": "New Sub",
            "target_type": "email",
            "target_value": "user@example.com",
            "frequency": "daily"
        })))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(sub_fixture(99, "New Sub", "email", "daily")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "subscription",
            "create",
            "--title",
            "New Sub",
            "--target-type",
            "email",
            "--target-value",
            "user@example.com",
            "--frequency",
            "daily",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Sub"));
}

// ── 4. update ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn subscription_update_title_patches() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/subscriptions/5/"))
        .and(body_partial_json(json!({"title": "Renamed Sub"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(sub_fixture(
            5,
            "Renamed Sub",
            "email",
            "weekly",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "subscription",
            "update",
            "5",
            "--title",
            "Renamed Sub",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"title\":\"Renamed Sub\""));
}

// ── 5. delete (soft) ──────────────────────────────────────────────────────────

#[tokio::test]
async fn subscription_delete_issues_soft_delete() {
    let h = TestHarness::new().await;

    // subscriptions is in SOFT_DELETE_RESOURCES — expect PATCH {"deleted": true}
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/subscriptions/7/"))
        .and(body_partial_json(json!({"deleted": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 7, "deleted": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "subscription", "delete", "7", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("\"id\":7"));
}

// ── 6. test-delivery ──────────────────────────────────────────────────────────

#[tokio::test]
async fn subscription_test_delivery_posts_to_dash_endpoint() {
    let h = TestHarness::new().await;
    // NOTE: the endpoint uses a literal dash: test-delivery (not test_delivery)
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/subscriptions/10/test-delivery/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "subscription", "test-delivery", "10", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 7. deliveries ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn subscription_deliveries_returns_log() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/subscriptions/15/deliveries/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "results": [
                {"created_at": "2026-04-01T09:00:00Z", "status": "delivered"}
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["subscription", "deliveries", "15", "--json"])
        .assert()
        .success()
        .stdout(contains("delivered"));
}

// ── 8. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn subscription_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19995"
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
        // NOT passing --yes
        .args(["subscription", "delete", "1", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY"
    );
}

// ── 9. test-delivery requires --yes ──────────────────────────────────────────

#[tokio::test]
async fn subscription_test_delivery_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19996"
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
        // NOT passing --yes
        .args(["subscription", "test-delivery", "1", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "test-delivery without --yes should not succeed in non-TTY"
    );
}
