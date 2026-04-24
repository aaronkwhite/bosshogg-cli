//! Integration tests for `bosshogg hog-function` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn hog_fn_fixture(id: &str, name: &str, fn_type: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test hog function",
        "type": fn_type,
        "template_id": "template-abc",
        "enabled": true,
        "deleted": false,
        "hog": "return event",
        "inputs": {"key": "value"},
        "inputs_schema": [],
        "filters": null,
        "mappings": null,
        "masking": null,
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-04-01T00:00:00Z",
        "created_by": {"id": 1, "email": "test@example.com"}
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/hog_functions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                hog_fn_fixture("uuid-f1", "Slack Destination", "destination"),
                hog_fn_fixture("uuid-f2", "Event Transformer", "transformation")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["hog-function", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Slack Destination"))
        .stdout(contains("Event Transformer"));
}

// ── 2. get by UUID ────────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_get_by_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/hog_functions/uuid-abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(hog_fn_fixture(
            "uuid-abc",
            "My Function",
            "destination",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["hog-function", "get", "uuid-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"uuid-abc\""))
        .stdout(contains("My Function"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_create_with_template() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/hog_functions/"))
        .and(body_partial_json(json!({
            "name": "New Function",
            "template_id": "template-xyz"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(hog_fn_fixture(
            "uuid-new",
            "New Function",
            "destination",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "hog-function",
            "create",
            "--name",
            "New Function",
            "--template-id",
            "template-xyz",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Function"));
}

// ── 4. update (name change) ───────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_update_name_patches() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/hog_functions/uuid-55/"))
        .and(body_partial_json(json!({"name": "Renamed Function"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(hog_fn_fixture(
            "uuid-55",
            "Renamed Function",
            "destination",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "hog-function",
            "update",
            "uuid-55",
            "--name",
            "Renamed Function",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed Function\""));
}

// ── 5. delete (soft) ──────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_delete_issues_soft_delete() {
    let h = TestHarness::new().await;

    // hog_functions is in SOFT_DELETE_RESOURCES — expect PATCH {"deleted": true}.
    // Use a proper hex UUID so is_soft_delete_path recognises it as an id segment.
    let fn_id = "aabbccdd-eeff-0011-2233-445566778899";
    Mock::given(method("PATCH"))
        .and(path(format!(
            "/api/environments/999999/hog_functions/{fn_id}/"
        )))
        .and(body_partial_json(json!({"deleted": true})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": fn_id, "deleted": true})),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "hog-function", "delete", fn_id, "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""));
}

// ── 6. enable ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_enable_patches_enabled_true() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/hog_functions/uuid-e1/"))
        .and(body_partial_json(json!({"enabled": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json(hog_fn_fixture(
            "uuid-e1",
            "My Function",
            "destination",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "hog-function", "enable", "uuid-e1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"enabled\":true"));
}

// ── 7. disable ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_disable_patches_enabled_false() {
    let h = TestHarness::new().await;
    let mut disabled_fixture = hog_fn_fixture("uuid-d1", "My Function", "destination");
    disabled_fixture["enabled"] = json!(false);
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/hog_functions/uuid-d1/"))
        .and(body_partial_json(json!({"enabled": false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(disabled_fixture))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "hog-function", "disable", "uuid-d1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"enabled\":false"));
}

// ── 8. invoke ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_invoke_posts_event() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/hog_functions/uuid-inv/invocations/",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"status": "success", "logs": []})),
        )
        .mount(&h.server)
        .await;

    let ev_file = h.config_path.parent().unwrap().join("event.json");
    std::fs::write(&ev_file, r#"{"event": "pageview", "properties": {}}"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "hog-function",
            "invoke",
            "uuid-inv",
            "--event-file",
            ev_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("success"));
}

// ── 9. logs ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_logs_returns_entries() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/hog_functions/uuid-log/logs/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"timestamp": "2026-04-01T00:00:00Z", "level": "INFO", "message": "executed ok"}
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["hog-function", "logs", "uuid-log", "--json"])
        .assert()
        .success()
        .stdout(contains("executed ok"));
}

// ── 10. metrics ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_metrics_returns_data() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/hog_functions/uuid-m1/metrics/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "successes": 100,
            "failures": 2
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["hog-function", "metrics", "uuid-m1", "--json"])
        .assert()
        .success()
        .stdout(contains("100"));
}

// ── 11. enable-backfills ──────────────────────────────────────────────────────

#[tokio::test]
async fn hog_function_enable_backfills_posts() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/hog_functions/uuid-bf/enable_backfills/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "hog-function",
            "enable-backfills",
            "uuid-bf",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 12. destructive op requires --yes ────────────────────────────────────────

#[tokio::test]
async fn hog_function_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19993"
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
        .args(["hog-function", "delete", "uuid-x", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY"
    );
}
