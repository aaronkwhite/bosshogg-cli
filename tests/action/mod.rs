//! Integration tests for `bosshogg action` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn action_fixture(id: i64, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test action",
        "post_to_slack": false,
        "slack_message_format": "",
        "steps": [{"event": "$pageview", "url": "/home"}],
        "deleted": false,
        "is_calculating": false,
        "created_at": "2026-01-01T00:00:00Z",
        "created_by": {"id": 1, "email": "admin@example.com"},
        "updated_at": "2026-04-01T00:00:00Z",
        "tags": [],
        "verified": false
    })
}

// ── 1. list returns results ───────────────────────────────────────────────────

#[tokio::test]
async fn action_list_returns_results() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/actions/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                action_fixture(1, "Sign Up"),
                action_fixture(2, "Purchase")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["action", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Sign Up"))
        .stdout(contains("Purchase"));
}

// ── 2. get by id ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn action_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/actions/42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(action_fixture(42, "My Action")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["action", "get", "42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":42"))
        .stdout(contains("My Action"));
}

// ── 3. create with steps file ─────────────────────────────────────────────────

#[tokio::test]
async fn action_create_with_steps_file() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/actions/"))
        .and(body_partial_json(json!({"name": "New Action"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(action_fixture(100, "New Action")))
        .mount(&h.server)
        .await;

    // Write a steps JSON file
    let steps_path = h.config_path.parent().unwrap().join("steps.json");
    std::fs::write(&steps_path, r#"[{"event": "$pageview"}]"#).unwrap();

    h.cmd()
        .args([
            "action",
            "create",
            "--name",
            "New Action",
            "--steps-file",
            steps_path.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Action"));
}

// ── 4. update (name change) ───────────────────────────────────────────────────

#[tokio::test]
async fn action_update_patches_name() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/actions/55/"))
        .and(body_partial_json(json!({"name": "Renamed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(action_fixture(55, "Renamed")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes", "action", "update", "55", "--name", "Renamed", "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed\""));
}

// ── 5. delete (soft via client.delete) ───────────────────────────────────────

#[tokio::test]
async fn action_delete_issues_patch_deleted_true() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/actions/77/"))
        .and(body_partial_json(json!({"deleted": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = action_fixture(77, "To Delete");
            f["deleted"] = json!(true);
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "action", "delete", "77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("\"id\":77"));
}

// ── 6. references endpoint ───────────────────────────────────────────────────

#[tokio::test]
async fn action_references_returns_data() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/actions/10/references/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"type": "insight", "id": 5, "name": "My Insight"}
        ])))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["action", "references", "10", "--json"])
        .assert()
        .success()
        .stdout(contains("insight"));
}

// ── 7. tag --add ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn action_tag_add_patches_tags() {
    let h = TestHarness::new().await;

    // GET to fetch current tags
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/actions/20/"))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = action_fixture(20, "Tagged Action");
            f["tags"] = json!(["existing"]);
            f
        }))
        .mount(&h.server)
        .await;

    // PATCH to update tags
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/actions/20/"))
        .and(body_partial_json(json!({"tags": ["existing", "new-tag"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = action_fixture(20, "Tagged Action");
            f["tags"] = json!(["existing", "new-tag"]);
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["action", "tag", "20", "--add", "new-tag", "--json"])
        .assert()
        .success()
        .stdout(contains("new-tag"));
}

// ── 8. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn action_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19995"
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
        .args(["action", "delete", "42", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}
