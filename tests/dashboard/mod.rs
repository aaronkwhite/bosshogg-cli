//! Integration tests for `bosshogg dashboard` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn dashboard_fixture(id: i64, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test dashboard",
        "pinned": false,
        "created_at": "2026-01-01T00:00:00Z",
        "created_by": {"id": 1, "email": "test@example.com"},
        "is_shared": false,
        "deleted": false,
        "creation_mode": "default",
        "tags": [],
        "tiles": [],
        "filters": {},
        "variables": null,
        "restriction_level": 21,
        "effective_privilege_level": 21
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn dashboard_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/dashboards/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                dashboard_fixture(1, "Traffic Overview"),
                dashboard_fixture(2, "Conversion Funnel")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dashboard", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Traffic Overview"))
        .stdout(contains("Conversion Funnel"));
}

// ── 2. get by numeric id ──────────────────────────────────────────────────────

#[tokio::test]
async fn dashboard_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/dashboards/42/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(dashboard_fixture(42, "My Dashboard")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dashboard", "get", "42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":42"))
        .stdout(contains("My Dashboard"));
}

// ── 3. refresh calls run_insights endpoint ────────────────────────────────────

#[tokio::test]
async fn dashboard_refresh_calls_run_insights() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/dashboards/10/run_insights/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "ok",
            "refreshed": 5
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dashboard", "refresh", "10", "--json"])
        .assert()
        .success()
        .stdout(contains("\"status\":\"ok\""));
}

// ── 4. create with --name ─────────────────────────────────────────────────────

#[tokio::test]
async fn dashboard_create_with_name() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/dashboards/"))
        .and(body_partial_json(json!({"name": "New Dashboard"})))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(dashboard_fixture(100, "New Dashboard")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dashboard", "create", "--name", "New Dashboard", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Dashboard"));
}

// ── 5. update (name change) ───────────────────────────────────────────────────

#[tokio::test]
async fn dashboard_update_name_patches() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/dashboards/55/"))
        .and(body_partial_json(json!({"name": "Renamed Dashboard"})))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(dashboard_fixture(55, "Renamed Dashboard")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "dashboard",
            "update",
            "55",
            "--name",
            "Renamed Dashboard",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed Dashboard\""));
}

// ── 6. delete (soft via client.delete) ───────────────────────────────────────

#[tokio::test]
async fn dashboard_delete_issues_patch_deleted_true() {
    let h = TestHarness::new().await;

    // Soft-delete routes through PATCH {deleted: true}
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/dashboards/77/"))
        .and(body_partial_json(json!({"deleted": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut f = dashboard_fixture(77, "To Delete");
            f["deleted"] = json!(true);
            f
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "dashboard", "delete", "77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("\"id\":77"));
}

// ── 7. tiles add: GET-then-PATCH insight.dashboards ──────────────────────────
//
// Modern PostHog attaches an insight to a dashboard by PATCHing the INSIGHT
// with an updated `dashboards` array. The legacy PATCH-dashboard.tiles path
// is silently dropped on current accounts (returns 200 with empty tiles).

#[tokio::test]
async fn dashboard_tiles_add_get_merge_patch() {
    let h = TestHarness::new().await;

    // GET insight 99 — no existing dashboard attachments.
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/99/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 99, "short_id": "AbC12345", "name": "x",
            "dashboards": []
        })))
        .mount(&h.server)
        .await;

    // PATCH insight 99 with dashboards=[20].
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/insights/99/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 99, "short_id": "AbC12345", "name": "x",
            "dashboards": [20]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "dashboard",
            "tiles",
            "add",
            "20",
            "--insight",
            "99",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"id\":99"));
}

// ── 7b. add preserves the insight's existing dashboard attachments ────────────

#[tokio::test]
async fn dashboard_tiles_add_preserves_existing_tiles() {
    use wiremock::matchers::body_partial_json;

    let h = TestHarness::new().await;

    // GET insight 99 — already attached to dashboards 10 and 11.
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/99/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 99, "short_id": "AbC12345", "name": "x",
            "dashboards": [10, 11]
        })))
        .mount(&h.server)
        .await;

    // PATCH body must include all three dashboard ids (existing + new).
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/insights/99/"))
        .and(body_partial_json(json!({
            "dashboards": [10, 11, 20]
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 99, "short_id": "AbC12345", "name": "x",
            "dashboards": [10, 11, 20]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "dashboard",
            "tiles",
            "add",
            "20",
            "--insight",
            "99",
            "--json",
        ])
        .assert()
        .success();
}

// ── 8. tiles remove: GET dashboard → PATCH insight.dashboards minus id ──────

#[tokio::test]
async fn dashboard_tiles_remove_patches_insight_dashboards() {
    use wiremock::matchers::body_partial_json;

    let h = TestHarness::new().await;

    // GET dashboard 20 → tile 7 points at insight 77.
    let mut dash = dashboard_fixture(20, "Dash");
    dash["tiles"] = json!([{"id": 7, "insight": {"id": 77}}]);
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/dashboards/20/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(dash))
        .mount(&h.server)
        .await;

    // GET insight 77 → currently attached to dashboards [20, 30].
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/insights/77/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 77, "short_id": "AbC12345", "name": "x",
            "dashboards": [20, 30]
        })))
        .mount(&h.server)
        .await;

    // PATCH insight 77 with dashboards = [30] (20 removed).
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/insights/77/"))
        .and(body_partial_json(json!({ "dashboards": [30] })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 77, "short_id": "AbC12345", "name": "x",
            "dashboards": [30]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "dashboard",
            "tiles",
            "remove",
            "20",
            "--tile",
            "7",
            "--json",
        ])
        .assert()
        .success();
}

// ── 11. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn dashboard_delete_without_yes_is_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
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
        .args(["dashboard", "delete", "42", "--json"])
        .output()
        .unwrap();

    // Must NOT succeed without --yes when not interactive
    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 12. share calls sharing endpoint ─────────────────────────────────────────

#[tokio::test]
async fn dashboard_share_calls_sharing_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/dashboards/60/sharing/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "enabled": true,
            "access_token": "dash_share_token_xyz"
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dashboard", "share", "60", "--json"])
        .assert()
        .success()
        .stdout(contains("\"enabled\":true"))
        .stdout(contains("dash_share_token_xyz"));
}
