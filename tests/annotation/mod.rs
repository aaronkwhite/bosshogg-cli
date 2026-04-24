//! Integration tests for `bosshogg annotation` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn annotation_fixture(id: i64, content: &str, date_marker: &str) -> serde_json::Value {
    json!({
        "id": id,
        "content": content,
        "date_marker": date_marker,
        "creation_type": "USER",
        "dashboard_item": null,
        "insight_short_id": null,
        "insight_name": null,
        "scope": "project",
        "deleted": false,
        "created_by": {"id": 1, "email": "admin@example.com"},
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-04-01T00:00:00Z"
    })
}

// ── 1. list returns results ───────────────────────────────────────────────────

#[tokio::test]
async fn annotation_list_returns_results() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/annotations/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                annotation_fixture(1, "Deploy v1.0", "2026-01-15T12:00:00Z"),
                annotation_fixture(2, "Marketing campaign start", "2026-02-01T00:00:00Z")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["annotation", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Deploy v1.0"))
        .stdout(contains("Marketing campaign start"));
}

// ── 2. get by id ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn annotation_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/annotations/42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(annotation_fixture(
            42,
            "Release day",
            "2026-04-01T00:00:00Z",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["annotation", "get", "42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":42"))
        .stdout(contains("Release day"));
}

// ── 3. create annotation ──────────────────────────────────────────────────────

#[tokio::test]
async fn annotation_create_with_content_and_date() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/annotations/"))
        .and(body_partial_json(json!({
            "content": "Feature launch",
            "date_marker": "2026-04-01T00:00:00Z"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(annotation_fixture(
            100,
            "Feature launch",
            "2026-04-01T00:00:00Z",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "annotation",
            "create",
            "--content",
            "Feature launch",
            "--date-marker",
            "2026-04-01T00:00:00Z",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("\"id\":100"));
}

// ── 4. create with scope ──────────────────────────────────────────────────────

#[tokio::test]
async fn annotation_create_with_scope() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/annotations/"))
        .and(body_partial_json(json!({
            "content": "Org-wide event",
            "scope": "organization"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json({
            let mut f = annotation_fixture(101, "Org-wide event", "2026-04-01T00:00:00Z");
            f["scope"] = json!("organization");
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "annotation",
            "create",
            "--content",
            "Org-wide event",
            "--date-marker",
            "2026-04-01T00:00:00Z",
            "--scope",
            "organization",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 5. update content ─────────────────────────────────────────────────────────

#[tokio::test]
async fn annotation_update_content_patches() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/annotations/55/"))
        .and(body_partial_json(json!({"content": "Updated content"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(annotation_fixture(
            55,
            "Updated content",
            "2026-04-01T00:00:00Z",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "annotation",
            "update",
            "55",
            "--content",
            "Updated content",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("Updated content"));
}

// ── 6. delete (soft via client.delete) ───────────────────────────────────────

#[tokio::test]
async fn annotation_delete_issues_patch_deleted_true() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/annotations/77/"))
        .and(body_partial_json(json!({"deleted": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = annotation_fixture(77, "To Delete", "2026-01-01T00:00:00Z");
            f["deleted"] = json!(true);
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "annotation", "delete", "77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("\"id\":77"));
}

// ── 7. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn annotation_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19994"
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
        .args(["annotation", "delete", "42", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 8. list with date filters ─────────────────────────────────────────────────

#[tokio::test]
async fn annotation_list_with_date_filters() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/annotations/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [
                annotation_fixture(5, "Mid-year review", "2026-06-01T00:00:00Z")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "annotation",
            "list",
            "--after",
            "2026-01-01T00:00:00Z",
            "--before",
            "2026-12-31T23:59:59Z",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("Mid-year review"));
}
