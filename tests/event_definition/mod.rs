//! Integration tests for `bosshogg event-definition` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn event_def_fixture(id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test event",
        "tags": [],
        "owner": null,
        "last_seen_at": "2026-04-01T00:00:00Z",
        "created_at": "2026-01-01T00:00:00Z",
        "last_updated_at": "2026-04-01T00:00:00Z",
        "verified": false,
        "verified_at": null,
        "verified_by": null,
        "is_action": false,
        "post_to_slack": false
    })
}

// ── 1. list returns results ───────────────────────────────────────────────────

#[tokio::test]
async fn event_definition_list_returns_results() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/event_definitions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                event_def_fixture("uuid-1", "$pageview"),
                event_def_fixture("uuid-2", "$identify")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["event-definition", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("$pageview"))
        .stdout(contains("$identify"));
}

// ── 2. get by id ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn event_definition_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/event_definitions/uuid-42/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(event_def_fixture("uuid-42", "sign_up")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["event-definition", "get", "uuid-42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"uuid-42\""))
        .stdout(contains("sign_up"));
}

// ── 3. update (description) ───────────────────────────────────────────────────

#[tokio::test]
async fn event_definition_update_patches_description() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/event_definitions/uuid-55/"))
        .and(body_partial_json(json!({"description": "Updated desc"})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = event_def_fixture("uuid-55", "$pageview");
            f["description"] = json!("Updated desc");
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "event-definition",
            "update",
            "uuid-55",
            "--description",
            "Updated desc",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("Updated desc"));
}

// ── 4. delete (hard delete via HTTP DELETE) ───────────────────────────────────

#[tokio::test]
async fn event_definition_delete_issues_hard_delete() {
    let h = TestHarness::new().await;

    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/event_definitions/uuid-77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "event-definition", "delete", "uuid-77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""));
}

// ── 5. by-name lookup ─────────────────────────────────────────────────────────

#[tokio::test]
async fn event_definition_by_name_lookup() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/event_definitions/by_name/"))
        .and(query_param("name", "$pageview"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(event_def_fixture("uuid-byname", "$pageview")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["event-definition", "by-name", "$pageview", "--json"])
        .assert()
        .success()
        .stdout(contains("$pageview"))
        .stdout(contains("uuid-byname"));
}

// ── 6. destructive ops require --yes ─────────────────────────────────────────

#[tokio::test]
async fn event_definition_delete_without_yes_blocked_in_non_tty() {
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
        .args(["event-definition", "delete", "uuid-99", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}
