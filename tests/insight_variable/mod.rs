//! Integration tests for `bosshogg insight-variable` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture ────────────────────────────────────────────────────────────

fn variable_fixture(id: &str, name: &str, vtype: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "type": vtype,
        "default_value": null,
        "code_name": name,
        "values": null,
        "created_by": 1,
        "created_at": "2026-01-01T00:00:00Z"
    })
}

// ── 1. list ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insight_variable_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/insight_variables/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                variable_fixture("iv-uuid-1", "date_range", "Date"),
                variable_fixture("iv-uuid-2", "event_limit", "Number"),
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight-variable", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("date_range"))
        .stdout(contains("event_limit"));
}

// ── 2. get ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insight_variable_get_by_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/insight_variables/iv-uuid-42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(variable_fixture(
            "iv-uuid-42",
            "my_var",
            "String",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight-variable", "get", "iv-uuid-42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"iv-uuid-42\""))
        .stdout(contains("my_var"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insight_variable_create_with_type() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/insight_variables/"))
        .and(body_partial_json(json!({
            "name": "my_variable",
            "type": "String"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(variable_fixture(
            "iv-uuid-new",
            "my_variable",
            "String",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "insight-variable",
            "create",
            "--name",
            "my_variable",
            "--type",
            "String",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("my_variable"));
}

// ── 4. update ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insight_variable_update_name() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/insight_variables/iv-uuid-55/"))
        .and(body_partial_json(json!({"name": "renamed_var"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(variable_fixture(
            "iv-uuid-55",
            "renamed_var",
            "Date",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "insight-variable",
            "update",
            "iv-uuid-55",
            "--name",
            "renamed_var",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"id\":\"iv-uuid-55\""))
        .stdout(contains("renamed_var"));
}

// ── 5. delete ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn insight_variable_delete_issues_hard_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/insight_variables/iv-uuid-77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "insight-variable",
            "delete",
            "iv-uuid-77",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("iv-uuid-77"));
}

// ── 6. delete requires --yes ──────────────────────────────────────────────────

#[tokio::test]
async fn insight_variable_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19970"
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
        .args(["insight-variable", "delete", "iv-uuid-x", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY"
    );
}
