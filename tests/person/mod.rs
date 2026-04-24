//! Integration tests for `bosshogg person` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

const TEST_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";
const TEST_DISTINCT_ID: &str = "user@example.com";

fn person_fixture(uuid: &str, distinct_id: &str) -> serde_json::Value {
    json!({
        "id": uuid,
        "uuid": uuid,
        "distinct_ids": [distinct_id, "alt_id_123"],
        "properties": {
            "email": distinct_id,
            "plan": "pro"
        },
        "is_identified": true,
        "created_at": "2026-01-01T00:00:00Z",
        "name": "Test User"
    })
}

/// Mount the distinct_id resolution mock (GET /persons/?distinct_id=...).
async fn mount_resolve_mock(server: &MockServer, uuid: &str, distinct_id: &str) {
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/persons/"))
        .and(query_param("distinct_id", distinct_id))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [person_fixture(uuid, distinct_id)]
        })))
        .mount(server)
        .await;
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn person_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/persons/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                person_fixture(TEST_UUID, "alice@example.com"),
                person_fixture("660e8400-e29b-41d4-a716-446655440001", "bob@example.com")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["person", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("alice@example.com"))
        .stdout(contains("bob@example.com"));
}

// ── 2. list with --distinct-id filter ────────────────────────────────────────

#[tokio::test]
async fn person_list_with_distinct_id_filter() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/persons/"))
        .and(query_param("distinct_id", TEST_DISTINCT_ID))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [person_fixture(TEST_UUID, TEST_DISTINCT_ID)]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "person",
            "list",
            "--distinct-id",
            TEST_DISTINCT_ID,
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains(TEST_UUID));
}

// ── 3. get resolves distinct_id to UUID then fetches ─────────────────────────

#[tokio::test]
async fn person_get_resolves_uuid_then_fetches() {
    let h = TestHarness::new().await;

    // Step 1: resolve distinct_id -> UUID
    mount_resolve_mock(&h.server, TEST_UUID, TEST_DISTINCT_ID).await;

    // Step 2: GET by UUID
    Mock::given(method("GET"))
        .and(path(format!(
            "/api/environments/999999/persons/{TEST_UUID}/"
        )))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(person_fixture(TEST_UUID, TEST_DISTINCT_ID)),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["person", "get", TEST_DISTINCT_ID, "--json"])
        .assert()
        .success()
        .stdout(contains(TEST_UUID))
        .stdout(contains("Test User"));
}

// ── 4. delete issues hard DELETE (not soft) ───────────────────────────────────

#[tokio::test]
async fn person_delete_issues_hard_delete() {
    let h = TestHarness::new().await;

    // Step 1: resolve
    mount_resolve_mock(&h.server, TEST_UUID, TEST_DISTINCT_ID).await;

    // Step 2: DELETE (hard — persons not in SOFT_DELETE_RESOURCES)
    Mock::given(method("DELETE"))
        .and(path(format!(
            "/api/environments/999999/persons/{TEST_UUID}/"
        )))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "person", "delete", TEST_DISTINCT_ID, "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""));
}

// ── 5. update-property POSTs to update_property endpoint ─────────────────────

#[tokio::test]
async fn person_update_property_posts() {
    let h = TestHarness::new().await;

    mount_resolve_mock(&h.server, TEST_UUID, TEST_DISTINCT_ID).await;

    Mock::given(method("POST"))
        .and(path(format!(
            "/api/environments/999999/persons/{TEST_UUID}/update_property/"
        )))
        .and(body_partial_json(json!({"$set": {"plan": "enterprise"}})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "person",
            "update-property",
            TEST_DISTINCT_ID,
            "--key",
            "plan",
            "--value",
            "enterprise",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 6. delete-property POSTs to delete_property endpoint ─────────────────────

#[tokio::test]
async fn person_delete_property_posts() {
    let h = TestHarness::new().await;

    mount_resolve_mock(&h.server, TEST_UUID, TEST_DISTINCT_ID).await;

    Mock::given(method("POST"))
        .and(path(format!(
            "/api/environments/999999/persons/{TEST_UUID}/delete_property/"
        )))
        .and(body_partial_json(json!({"$unset": ["plan"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "person",
            "delete-property",
            TEST_DISTINCT_ID,
            "--key",
            "plan",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 7. split POSTs to split endpoint ─────────────────────────────────────────

#[tokio::test]
async fn person_split_posts_main_distinct_id() {
    let h = TestHarness::new().await;

    mount_resolve_mock(&h.server, TEST_UUID, TEST_DISTINCT_ID).await;

    Mock::given(method("POST"))
        .and(path(format!(
            "/api/environments/999999/persons/{TEST_UUID}/split/"
        )))
        .and(body_partial_json(json!({"main_distinct_id": "main_user"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "person",
            "split",
            TEST_DISTINCT_ID,
            "--main-distinct-id",
            "main_user",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 9. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn person_delete_without_yes_is_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19997"
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
        .args(["person", "delete", "user@example.com", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}
