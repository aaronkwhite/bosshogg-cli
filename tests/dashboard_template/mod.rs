//! Integration tests for `bosshogg dashboard-template` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture ────────────────────────────────────────────────────────────

fn template_fixture(id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "template_name": name,
        "dashboard_description": "A template",
        "scope": "team",
        "is_featured": false,
        "deleted": false,
        "team_id": 42,
        "created_at": "2026-01-01T00:00:00Z",
        "created_by": {"id": 1, "email": "test@example.com"}
    })
}

// ── 1. list ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn dashboard_template_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/dashboard_templates/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                template_fixture("dt-uuid-1", "Product Analytics"),
                template_fixture("dt-uuid-2", "Marketing Funnel"),
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dashboard-template", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Product Analytics"))
        .stdout(contains("Marketing Funnel"));
}

// ── 2. get ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn dashboard_template_get_by_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/dashboard_templates/dt-uuid-42/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(template_fixture("dt-uuid-42", "My Template")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dashboard-template", "get", "dt-uuid-42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"dt-uuid-42\""))
        .stdout(contains("My Template"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn dashboard_template_create_with_name() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/dashboard_templates/"))
        .and(body_partial_json(json!({"template_name": "New Template"})))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(template_fixture("dt-uuid-new", "New Template")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "dashboard-template",
            "create",
            "--name",
            "New Template",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Template"));
}

// ── 4. use (instantiate via dashboard create) ─────────────────────────────────

#[tokio::test]
async fn dashboard_template_use_creates_dashboard() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/dashboards/"))
        .and(body_partial_json(json!({
            "name": "My New Dashboard",
            "use_template": "dt-uuid-42"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 123,
            "name": "My New Dashboard",
            "description": "",
            "created_at": "2026-04-01T00:00:00Z"
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "dashboard-template",
            "use",
            "dt-uuid-42",
            "--new-name",
            "My New Dashboard",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"use\""))
        .stdout(contains("My New Dashboard"))
        .stdout(contains("dt-uuid-42"));
}
