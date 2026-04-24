//! Integration tests for `bosshogg insight` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. In-process tests use
//! `Client::for_test` (skips TLS). Binary-level tests use `Command::cargo_bin`
//! with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn insight_fixture(id: i64, short_id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "short_id": short_id,
        "name": name,
        "description": "A test insight",
        "filters": {"events": [{"id": "$pageview"}]},
        "result": null,
        "deleted": false,
        "tags": [],
        "saved": true,
        "favorited": false,
        "last_refresh": "2026-04-01T00:00:00Z",
        "refreshing": false,
        "timezone": "UTC",
        "created_at": "2026-03-01T00:00:00Z",
        "updated_at": "2026-04-01T00:00:00Z"
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn insight_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                insight_fixture(10, "aAbBcC", "Pageviews"),
                insight_fixture(11, "dDeEfF", "Signups")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("aAbBcC"))
        .stdout(contains("Pageviews"));
}

// ── 2. list with --search filter ──────────────────────────────────────────────

#[tokio::test]
async fn insight_list_with_search_filter() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/"))
        .and(query_param("search", "funnel"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [insight_fixture(20, "xXyYzZ", "Signup Funnel")]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "list", "--search", "funnel", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("Signup Funnel"));
}

// ── 3. get by short_id (list filter + fetch-by-id flow) ───────────────────────

#[tokio::test]
async fn insight_get_by_short_id_resolves_then_fetches() {
    let h = TestHarness::new().await;

    // Step 1: list with ?short_id= to resolve numeric id
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/"))
        .and(query_param("short_id", "aAbBcC"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [insight_fixture(42, "aAbBcC", "Pageviews")]
        })))
        .mount(&h.server)
        .await;

    // Step 2: fetch by numeric id
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(insight_fixture(
            42,
            "aAbBcC",
            "Pageviews",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "get", "aAbBcC", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":42"))
        .stdout(contains("\"short_id\":\"aAbBcC\""))
        .stdout(contains("Pageviews"));
}

// ── 4. get by numeric id ──────────────────────────────────────────────────────

#[tokio::test]
async fn insight_get_by_numeric_id() {
    let h = TestHarness::new().await;

    // Should NOT hit the list endpoint — goes directly to /insights/55/
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/55/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(insight_fixture(
            55,
            "numId55",
            "Direct Get",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "get", "55", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":55"))
        .stdout(contains("Direct Get"));
}

// ── 5. refresh appends ?refresh=true ─────────────────────────────────────────

#[tokio::test]
async fn insight_refresh_appends_query_param() {
    let h = TestHarness::new().await;

    // resolve short_id
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/"))
        .and(query_param("short_id", "rRsStt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [insight_fixture(77, "rRsStt", "Refresh Me")]
        })))
        .mount(&h.server)
        .await;

    // refresh call — must include ?refresh=true
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/77/"))
        .and(query_param("refresh", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = insight_fixture(77, "rRsStt", "Refresh Me");
            f["last_refresh"] = json!("2026-04-21T12:00:00Z");
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "refresh", "rRsStt", "--json"])
        .assert()
        .success()
        .stdout(contains("\"short_id\":\"rRsStt\""))
        .stdout(contains("2026-04-21T12:00:00Z"));
}

// ── 6. create --filters-file POSTs the body ───────────────────────────────────

#[tokio::test]
async fn insight_create_posts_filters_body() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/insights/"))
        .and(body_partial_json(json!({
            "filters": {"events": [{"id": "$pageview"}]},
            "saved": false
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(insight_fixture(
            99,
            "newShrt",
            "Created Insight",
        )))
        .mount(&h.server)
        .await;

    // Write a filters file in the temp dir
    let filters_path = h.config_path.parent().unwrap().join("filters.json");
    std::fs::write(&filters_path, r#"{"events": [{"id": "$pageview"}]}"#).unwrap();

    h.cmd()
        .args([
            "insight",
            "create",
            "--filters-file",
            filters_path.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("\"short_id\":\"newShrt\""));
}

// ── 7. update --name PATCHes ──────────────────────────────────────────────────

#[tokio::test]
async fn insight_update_name_patches() {
    let h = TestHarness::new().await;

    // resolve short_id
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/"))
        .and(query_param("short_id", "aBcDeF"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [insight_fixture(30, "aBcDeF", "Old Name")]
        })))
        .mount(&h.server)
        .await;

    // PATCH with new name
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/insights/30/"))
        .and(body_partial_json(json!({"name": "New Name"})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = insight_fixture(30, "aBcDeF", "New Name");
            f["name"] = json!("New Name");
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes", "insight", "update", "aBcDeF", "--name", "New Name", "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"New Name\""));
}

// ── 8. delete issues PATCH {deleted:true} via soft-delete router ──────────────

#[tokio::test]
async fn insight_delete_issues_patch_deleted_true() {
    let h = TestHarness::new().await;

    // resolve short_id
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/"))
        .and(query_param("short_id", "dElEtE"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [insight_fixture(88, "dElEtE", "To Delete")]
        })))
        .mount(&h.server)
        .await;

    // soft-delete routes through PATCH {deleted: true}
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/insights/88/"))
        .and(body_partial_json(json!({"deleted": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = insight_fixture(88, "dElEtE", "To Delete");
            f["deleted"] = json!(true);
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "insight", "delete", "dElEtE", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("dElEtE"));
}

// ── 9. tag --add foo appends to tags ─────────────────────────────────────────

#[tokio::test]
async fn insight_tag_add_appends_tag() {
    let h = TestHarness::new().await;

    // resolve short_id
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/"))
        .and(query_param("short_id", "tAgTag"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [insight_fixture(50, "tAgTag", "Tagged Insight")]
        })))
        .mount(&h.server)
        .await;

    // fetch current insight to read existing tags
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/50/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(insight_fixture(
            50,
            "tAgTag",
            "Tagged Insight",
        )))
        .mount(&h.server)
        .await;

    // PATCH with new tags list
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/insights/50/"))
        .and(body_partial_json(json!({"tags": ["prod"]})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = insight_fixture(50, "tAgTag", "Tagged Insight");
            f["tags"] = json!(["prod"]);
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "tag", "tAgTag", "--add", "prod", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"tags\":[\"prod\"]"));
}

// ── 10. destructive op requires --yes (delete blocked without it) ─────────────

#[tokio::test]
async fn insight_delete_without_yes_is_blocked_in_non_tty() {
    // Without a real TTY, dialoguer will fail to open a prompt and the command
    // should exit with an error rather than silently proceeding.
    // We verify the binary either asks for --yes or errors — NOT that it succeeds.
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    // No server needed — the confirm prompt should fire before any HTTP call.
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19999"
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
        .args(["insight", "delete", "aAbBcC", "--json"])
        .output()
        .unwrap();

    // Must NOT succeed without --yes when not interactive
    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 11. activity endpoint is called ──────────────────────────────────────────

#[tokio::test]
async fn insight_activity_calls_correct_endpoint() {
    let h = TestHarness::new().await;

    // resolve numeric id directly (no short_id lookup needed)
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/60/activity/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "activity": "updated",
                    "created_at": "2026-04-01T10:00:00Z",
                    "user": {"email": "alice@example.com"}
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "activity", "60", "--json"])
        .assert()
        .success()
        .stdout(contains("updated"))
        .stdout(contains("alice@example.com"));
}

// ── 12. share calls sharing endpoint ─────────────────────────────────────────

#[tokio::test]
async fn insight_share_calls_sharing_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/70/sharing/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "access_token": "share_token_abc123"
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["insight", "share", "70", "--json"])
        .assert()
        .success()
        .stdout(contains("\"enabled\":true"))
        .stdout(contains("share_token_abc123"));
}
