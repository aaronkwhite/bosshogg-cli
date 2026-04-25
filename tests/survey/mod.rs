//! Integration tests for `bosshogg survey` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn survey_fixture(id: &str, name: &str, survey_type: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test survey",
        "type": survey_type,
        "questions": [{"type": "open", "question": "How are you?"}],
        "appearance": null,
        "conditions": null,
        "start_date": null,
        "end_date": null,
        "linked_flag": null,
        "targeting_flag": null,
        "internal_targeting_flag": null,
        "created_at": "2026-01-01T00:00:00Z",
        "updated_at": "2026-04-01T00:00:00Z",
        "archived": false,
        "enable_partial_responses": false,
        "responses_limit": null,
        "iteration_count": null,
        "current_iteration": null
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn survey_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                survey_fixture("uuid-s1", "NPS Survey", "popover"),
                survey_fixture("uuid-s2", "Product Feedback", "api")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["survey", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("NPS Survey"))
        .stdout(contains("Product Feedback"));
}

// ── 2. get by UUID ────────────────────────────────────────────────────────────

#[tokio::test]
async fn survey_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/uuid-abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(survey_fixture(
            "uuid-abc",
            "My Survey",
            "widget",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["survey", "get", "uuid-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"uuid-abc\""))
        .stdout(contains("My Survey"));
}

// ── 3. create survey ──────────────────────────────────────────────────────────

#[tokio::test]
async fn survey_create_with_questions_file() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/surveys/"))
        .and(body_partial_json(
            json!({"name": "New Survey", "type": "popover"}),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(survey_fixture(
            "uuid-new",
            "New Survey",
            "popover",
        )))
        .mount(&h.server)
        .await;

    let q_file = h.config_path.parent().unwrap().join("questions.json");
    std::fs::write(
        &q_file,
        r#"[{"type": "open", "question": "What do you think?"}]"#,
    )
    .unwrap();

    h.cmd()
        .args([
            "survey",
            "create",
            "--name",
            "New Survey",
            "--type",
            "popover",
            "--questions-file",
            q_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Survey"));
}

// ── 4. update (name change) ───────────────────────────────────────────────────

#[tokio::test]
async fn survey_update_name_patches() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/surveys/uuid-55/"))
        .and(body_partial_json(json!({"name": "Renamed Survey"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(survey_fixture(
            "uuid-55",
            "Renamed Survey",
            "popover",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "survey",
            "update",
            "uuid-55",
            "--name",
            "Renamed Survey",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed Survey\""));
}

// ── 5. delete (hard delete) ───────────────────────────────────────────────────

#[tokio::test]
async fn survey_delete_issues_hard_delete() {
    let h = TestHarness::new().await;

    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/surveys/uuid-77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "survey", "delete", "uuid-77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("uuid-77"));
}

// ── 6. activity endpoint ──────────────────────────────────────────────────────

#[tokio::test]
async fn survey_activity_returns_log() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/uuid-5/activity/"))
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
        .args(["survey", "activity", "uuid-5", "--json"])
        .assert()
        .success()
        .stdout(contains("created"));
}

// ── 7. duplicate to projects ──────────────────────────────────────────────────

#[tokio::test]
async fn survey_duplicate_to_projects() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path(
            "/api/projects/999999/surveys/uuid-20/duplicate_to_projects/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "survey",
            "duplicate",
            "uuid-20",
            "--target-project-ids",
            "100,200",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 8. archive-response ───────────────────────────────────────────────────────

#[tokio::test]
async fn survey_archive_response_posts_to_endpoint() {
    let h = TestHarness::new().await;

    let resp_uuid = "resp-uuid-1234";
    Mock::given(method("POST"))
        .and(path(format!(
            "/api/projects/999999/surveys/uuid-s1/responses/{resp_uuid}/archive/"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "survey",
            "archive-response",
            "uuid-s1",
            "--response-uuid",
            resp_uuid,
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 9. survey stats ───────────────────────────────────────────────────────────

#[tokio::test]
async fn survey_stats_returns_aggregates() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/uuid-s5/stats/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "responses": 42,
            "response_rate": 0.68
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["survey", "stats", "uuid-s5", "--json"])
        .assert()
        .success()
        .stdout(contains("42"))
        .stdout(contains("0.68"));
}

// ── 10. survey project-stats ──────────────────────────────────────────────────

#[tokio::test]
async fn survey_project_stats_returns_all_surveys_aggregate() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/stats/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "total_surveys": 5,
            "total_responses": 200
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["survey", "project-stats", "--json"])
        .assert()
        .success()
        .stdout(contains("total_surveys"))
        .stdout(contains("200"));
}

// ── 11. survey responses-count ────────────────────────────────────────────────

#[tokio::test]
async fn survey_responses_count_returns_counter() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/responses_count/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1234
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["survey", "responses-count", "--json"])
        .assert()
        .success()
        .stdout(contains("1234"));
}

// ── 12. survey project-activity ───────────────────────────────────────────────

#[tokio::test]
async fn survey_project_activity_returns_log() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/activity/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "activity": "created",
                    "created_at": "2026-04-01T00:00:00Z",
                    "user": {"email": "admin@example.com"}
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["survey", "project-activity", "--json"])
        .assert()
        .success()
        .stdout(contains("created"));
}

// ── 13. survey summarize ──────────────────────────────────────────────────────

#[tokio::test]
async fn survey_summarize_posts_to_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path(
            "/api/projects/999999/surveys/uuid-sum/summarize_responses/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "summary": "Users generally feel positive about the product."
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "survey", "summarize", "uuid-sum", "--json"])
        .assert()
        .success()
        .stdout(contains("summary"))
        .stdout(contains("positive"));
}

// ── destructive op requires --yes ─────────────────────────────────────────────

#[tokio::test]
async fn survey_delete_without_yes_blocked_in_non_tty() {
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
        // NOT passing --yes
        .args(["survey", "delete", "uuid-x", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 10. list with --archived flag ─────────────────────────────────────────────

#[tokio::test]
async fn survey_list_with_archived_flag() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/surveys/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [
                {
                    "id": "uuid-archived",
                    "name": "Old Survey",
                    "type": "popover",
                    "questions": [],
                    "archived": true,
                    "created_at": "2025-01-01T00:00:00Z",
                    "updated_at": "2025-06-01T00:00:00Z"
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["survey", "list", "--archived", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("Old Survey"));
}
