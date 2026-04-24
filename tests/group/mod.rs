//! Integration tests for `bosshogg group` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn group_fixture(type_index: i32, key: &str) -> serde_json::Value {
    json!({
        "group_type_index": type_index,
        "group_key": key,
        "group_properties": {
            "name": "Acme Corp",
            "plan": "enterprise"
        },
        "created_at": "2026-01-01T00:00:00Z"
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn group_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/groups/"))
        .and(query_param("group_type_index", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                group_fixture(0, "acme_corp"),
                group_fixture(0, "globex_inc")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["group", "list", "--group-type-index", "0", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("acme_corp"))
        .stdout(contains("globex_inc"));
}

// ── 2. list with --group-type-index filter ────────────────────────────────────

#[tokio::test]
async fn group_list_with_type_filter() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/groups/"))
        .and(query_param("group_type_index", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [group_fixture(2, "team_alpha")]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["group", "list", "--group-type-index", "2", "--json"])
        .assert()
        .success()
        .stdout(contains("team_alpha"));
}

// ── 3. find fetches single group ──────────────────────────────────────────────

#[tokio::test]
async fn group_find_fetches_single_group() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/groups/find/"))
        .and(query_param("group_type_index", "0"))
        .and(query_param("group_key", "acme_corp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(group_fixture(0, "acme_corp")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "group",
            "find",
            "--group-type-index",
            "0",
            "--group-key",
            "acme_corp",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("acme_corp"))
        .stdout(contains("\"group_type_index\":0"));
}

// ── 4. property-definitions returns defs ─────────────────────────────────────

#[tokio::test]
async fn group_property_definitions_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/groups/property_definitions/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"name": "plan", "type": "String", "property_type": "String"},
                {"name": "seats", "type": "Numeric", "property_type": "Numeric"}
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["group", "property-definitions", "--json"])
        .assert()
        .success()
        .stdout(contains("plan"))
        .stdout(contains("seats"));
}

// ── 5. update-property POSTs to update_property ───────────────────────────────

#[tokio::test]
async fn group_update_property_posts() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/groups/update_property/"))
        .and(body_partial_json(json!({
            "group_type_index": 0,
            "group_key": "acme_corp",
            "$set": {"plan": "enterprise"}
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "group",
            "update-property",
            "--group-type-index",
            "0",
            "--group-key",
            "acme_corp",
            "--prop-key",
            "plan",
            "--prop-value",
            "enterprise",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 6. delete-property POSTs to delete_property ───────────────────────────────

#[tokio::test]
async fn group_delete_property_posts() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/groups/delete_property/"))
        .and(body_partial_json(json!({
            "group_type_index": 0,
            "group_key": "acme_corp",
            "$unset": ["plan"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "group",
            "delete-property",
            "--group-type-index",
            "0",
            "--group-key",
            "acme_corp",
            "--prop-key",
            "plan",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 7. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn group_update_property_without_yes_blocked_in_non_tty() {
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
        .args([
            "group",
            "update-property",
            "--group-type-index",
            "0",
            "--group-key",
            "acme_corp",
            "--prop-key",
            "plan",
            "--prop-value",
            "enterprise",
            "--json",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "update-property without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}
