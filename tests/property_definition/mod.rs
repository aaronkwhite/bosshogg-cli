//! Integration tests for `bosshogg property-definition` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn prop_def_fixture(id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test property",
        "tags": [],
        "is_numerical": false,
        "property_type": "String",
        "is_seen_on_filtered_events": null,
        "verified": false,
        "created_at": "2026-01-01T00:00:00Z",
        "last_updated_at": "2026-04-01T00:00:00Z",
        "last_seen_at": "2026-04-20T00:00:00Z"
    })
}

// ── 1. list returns results ───────────────────────────────────────────────────

#[tokio::test]
async fn property_definition_list_returns_results() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/property_definitions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                prop_def_fixture("p-uuid-1", "$browser"),
                prop_def_fixture("p-uuid-2", "$os")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["property-definition", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("$browser"))
        .stdout(contains("$os"));
}

// ── 2. get by id ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn property_definition_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/property_definitions/p-uuid-42/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(prop_def_fixture("p-uuid-42", "revenue")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["property-definition", "get", "p-uuid-42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"p-uuid-42\""))
        .stdout(contains("revenue"));
}

// ── 3. list with type filter ──────────────────────────────────────────────────

#[tokio::test]
async fn property_definition_list_with_type_filter() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/property_definitions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [prop_def_fixture("p-uuid-3", "email")]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["property-definition", "list", "--type", "person", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("email"));
}

// ── 4. update (description) ───────────────────────────────────────────────────

#[tokio::test]
async fn property_definition_update_patches_description() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/property_definitions/p-uuid-55/"))
        .and(body_partial_json(json!({"description": "Updated desc"})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = prop_def_fixture("p-uuid-55", "$browser");
            f["description"] = json!("Updated desc");
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "property-definition",
            "update",
            "p-uuid-55",
            "--description",
            "Updated desc",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("Updated desc"));
}

// ── 5. delete (hard delete) ───────────────────────────────────────────────────

#[tokio::test]
async fn property_definition_delete_issues_hard_delete() {
    let h = TestHarness::new().await;

    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/property_definitions/p-uuid-77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "property-definition",
            "delete",
            "p-uuid-77",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""));
}

// ── 6. seen-together ─────────────────────────────────────────────────────────

#[tokio::test]
async fn property_definition_seen_together_returns_data() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/projects/999999/property_definitions/seen_together/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"name": "$browser", "count": 120}]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "property-definition",
            "seen-together",
            "--event1",
            "$pageview",
            "--event2",
            "$identify",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("$browser"));
}

// ── 7. destructive ops require --yes ─────────────────────────────────────────

#[tokio::test]
async fn property_definition_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19992"
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
        .args(["property-definition", "delete", "p-uuid-99", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}
