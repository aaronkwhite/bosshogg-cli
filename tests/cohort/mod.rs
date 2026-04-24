//! Integration tests for `bosshogg cohort` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn cohort_fixture(id: i64, name: &str, is_static: bool) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test cohort",
        "groups": null,
        "deleted": false,
        "filters": {"properties": []},
        "query": null,
        "is_calculating": false,
        "created_by": {"id": 1, "email": "test@example.com"},
        "created_at": "2026-01-01T00:00:00Z",
        "last_calculation": "2026-04-01T00:00:00Z",
        "errors_calculating": 0,
        "count": 42,
        "is_static": is_static,
        "experiment_set": []
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn cohort_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/cohorts/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                cohort_fixture(1, "Power Users", false),
                cohort_fixture(2, "New Signups", true)
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["cohort", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Power Users"))
        .stdout(contains("New Signups"));
}

// ── 2. get by numeric id ──────────────────────────────────────────────────────

#[tokio::test]
async fn cohort_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/cohorts/42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(cohort_fixture(
            42,
            "My Cohort",
            false,
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["cohort", "get", "42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":42"))
        .stdout(contains("My Cohort"));
}

// ── 3. create with --name ─────────────────────────────────────────────────────

#[tokio::test]
async fn cohort_create_with_name() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/cohorts/"))
        .and(body_partial_json(json!({"name": "New Cohort"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(cohort_fixture(
            100,
            "New Cohort",
            false,
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["cohort", "create", "--name", "New Cohort", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Cohort"));
}

// ── 4. create static cohort ───────────────────────────────────────────────────

#[tokio::test]
async fn cohort_create_static() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/cohorts/"))
        .and(body_partial_json(
            json!({"name": "Static Cohort", "is_static": true}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(cohort_fixture(
            101,
            "Static Cohort",
            true,
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "cohort",
            "create",
            "--name",
            "Static Cohort",
            "--static",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"id\":101"));
}

// ── 5. update (name change) ───────────────────────────────────────────────────

#[tokio::test]
async fn cohort_update_name_patches() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/cohorts/55/"))
        .and(body_partial_json(json!({"name": "Renamed Cohort"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(cohort_fixture(
            55,
            "Renamed Cohort",
            false,
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "cohort",
            "update",
            "55",
            "--name",
            "Renamed Cohort",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed Cohort\""));
}

// ── 6. delete (soft via client.delete) ───────────────────────────────────────

#[tokio::test]
async fn cohort_delete_issues_patch_deleted_true() {
    let h = TestHarness::new().await;

    // Soft-delete routes through PATCH {deleted: true}
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/cohorts/77/"))
        .and(body_partial_json(json!({"deleted": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = cohort_fixture(77, "To Delete", false);
            f["deleted"] = json!(true);
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "cohort", "delete", "77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("\"id\":77"));
}

// ── 7. members (paginated) ────────────────────────────────────────────────────

#[tokio::test]
async fn cohort_members_paginated() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/cohorts/10/persons/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                {
                    "id": "uuid-1",
                    "uuid": "uuid-1",
                    "distinct_ids": ["user@example.com"]
                },
                {
                    "id": "uuid-2",
                    "uuid": "uuid-2",
                    "distinct_ids": ["other@example.com"]
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["cohort", "members", "10", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("uuid-1"));
}

// ── 8. add-person to static cohort ───────────────────────────────────────────

#[tokio::test]
async fn cohort_add_person_to_static_cohort() {
    let h = TestHarness::new().await;

    // First: GET to verify cohort is static
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/cohorts/20/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(cohort_fixture(
            20,
            "Static Cohort",
            true,
        )))
        .mount(&h.server)
        .await;

    // Then: PATCH to add person
    Mock::given(method("PATCH"))
        .and(path(
            "/api/projects/999999/cohorts/20/add_persons_to_static_cohort/",
        ))
        .and(body_partial_json(json!({
            "person_uuids": ["aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "cohort",
            "add-person",
            "20",
            "--person-id",
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 9. remove-person from static cohort ──────────────────────────────────────

#[tokio::test]
async fn cohort_remove_person_from_static_cohort() {
    let h = TestHarness::new().await;

    // First: GET to verify cohort is static
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/cohorts/21/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(cohort_fixture(
            21,
            "Static Cohort",
            true,
        )))
        .mount(&h.server)
        .await;

    // Then: PATCH to remove person
    Mock::given(method("PATCH"))
        .and(path(
            "/api/projects/999999/cohorts/21/remove_person_from_static_cohort/",
        ))
        .and(body_partial_json(json!({
            "person_uuid": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "cohort",
            "remove-person",
            "21",
            "--person-id",
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 10. add-person rejected on dynamic cohort ─────────────────────────────────

#[tokio::test]
async fn cohort_add_person_rejected_on_dynamic_cohort() {
    let h = TestHarness::new().await;

    // GET returns a dynamic cohort (is_static = false)
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/cohorts/30/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(cohort_fixture(
            30,
            "Dynamic Cohort",
            false,
        )))
        .mount(&h.server)
        .await;

    let output = h
        .cmd()
        .args([
            "--yes",
            "cohort",
            "add-person",
            "30",
            "--person-id",
            "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "add-person on dynamic cohort should fail"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stderr}{stdout}");
    assert!(
        combined.contains("dynamic"),
        "error should mention 'dynamic': {combined}"
    );
}

// ── 11. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn cohort_delete_without_yes_is_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19998"
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
        .args(["cohort", "delete", "42", "--json"])
        .output()
        .unwrap();

    // Must NOT succeed without --yes when not interactive
    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 12. activity endpoint ─────────────────────────────────────────────────────

#[tokio::test]
async fn cohort_activity_returns_log() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/cohorts/5/activity/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "activity": "created",
                    "created_at": "2026-01-01T00:00:00Z",
                    "user": {"email": "admin@example.com"}
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["cohort", "activity", "5", "--json"])
        .assert()
        .success()
        .stdout(contains("created"));
}
