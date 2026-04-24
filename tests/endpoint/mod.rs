//! Integration tests for `bosshogg endpoint` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn endpoint_fixture(name: &str) -> serde_json::Value {
    json!({
        "name": name,
        "description": "A test endpoint",
        "query": {"kind": "HogQLQuery", "query": "SELECT count() FROM events"},
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-04-01T00:00:00Z",
        "created_by": {"id": 1, "email": "admin@example.com"},
        "is_materialized": false,
        "last_materialized_at": null
    })
}

// ── 1. list returns results ───────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_list_returns_results() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/endpoints/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                endpoint_fixture("daily-signups"),
                endpoint_fixture("weekly-revenue")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["endpoint", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("daily-signups"))
        .stdout(contains("weekly-revenue"));
}

// ── 2. get by name ────────────────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_get_by_name() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/endpoints/daily-signups/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(endpoint_fixture("daily-signups")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["endpoint", "get", "daily-signups", "--json"])
        .assert()
        .success()
        .stdout(contains("\"name\":\"daily-signups\""))
        .stdout(contains("HogQLQuery"));
}

// ── 3. create with query file ─────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_create_with_query_file() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/endpoints/"))
        .and(body_partial_json(json!({"name": "new-endpoint"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(endpoint_fixture("new-endpoint")))
        .mount(&h.server)
        .await;

    let sql_path = h.config_path.parent().unwrap().join("query.sql");
    std::fs::write(&sql_path, "SELECT count() FROM events").unwrap();

    h.cmd()
        .args([
            "endpoint",
            "create",
            "--name",
            "new-endpoint",
            "--query-file",
            sql_path.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("new-endpoint"));
}

// ── 4. update (description) ───────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_update_patches_description() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/endpoints/my-endpoint/"))
        .and(body_partial_json(
            json!({"description": "Updated description"}),
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = endpoint_fixture("my-endpoint");
            f["description"] = json!("Updated description");
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "endpoint",
            "update",
            "my-endpoint",
            "--description",
            "Updated description",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("Updated description"));
}

// ── 5. delete ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_delete_issues_delete() {
    let h = TestHarness::new().await;

    Mock::given(method("DELETE"))
        .and(path("/api/environments/999999/endpoints/old-endpoint/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "endpoint", "delete", "old-endpoint", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("old-endpoint"));
}

// ── 6. run ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_run_returns_results() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/endpoints/daily-signups/run/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "columns": ["count"],
            "results": [[42]]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["endpoint", "run", "daily-signups", "--json"])
        .assert()
        .success()
        .stdout(contains("42"));
}

// ── 7. materialize-status ─────────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_materialize_status_returns_data() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/endpoints/daily-signups/materialization_status/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ready",
            "last_materialized_at": "2026-04-20T00:00:00Z"
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["endpoint", "materialize-status", "daily-signups", "--json"])
        .assert()
        .success()
        .stdout(contains("ready"));
}

// ── 8. destructive ops require --yes ─────────────────────────────────────────

#[tokio::test]
async fn endpoint_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19991"
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
        .args(["endpoint", "delete", "some-endpoint", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 9. openapi spec ───────────────────────────────────────────────────────────

#[tokio::test]
async fn endpoint_openapi_returns_spec() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/endpoints/daily-signups/openapi.json/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "openapi": "3.0.0",
            "info": {"title": "daily-signups", "version": "1"}
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["endpoint", "openapi", "daily-signups", "--json"])
        .assert()
        .success()
        .stdout(contains("openapi"))
        .stdout(contains("3.0.0"));
}
