//! Integration tests for `bosshogg batch-export` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn export_fixture(id: &str, name: &str, interval: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "destination": {"type": "S3", "config": {"bucket": "my-bucket"}},
        "interval": interval,
        "paused": false,
        "created_at": "2026-01-01T00:00:00Z",
        "last_updated_at": "2026-04-01T00:00:00Z",
        "last_paused_at": null,
        "start_at": null,
        "end_at": null,
        "schema": null,
        "model": "events"
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/batch_exports/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                export_fixture("exp-1", "S3 Hourly", "hour"),
                export_fixture("exp-2", "BigQuery Daily", "day")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["batch-export", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("S3 Hourly"))
        .stdout(contains("BigQuery Daily"));
}

// ── 2. get by UUID ────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_get_by_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/batch_exports/exp-abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(export_fixture(
            "exp-abc",
            "S3 Export",
            "hour",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["batch-export", "get", "exp-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"exp-abc\""))
        .stdout(contains("S3 Export"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_create_with_destination_file() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/batch_exports/"))
        .and(body_partial_json(json!({"name": "New Export"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(export_fixture(
            "exp-new",
            "New Export",
            "hour",
        )))
        .mount(&h.server)
        .await;

    let dest_file = h.config_path.parent().unwrap().join("dest.json");
    std::fs::write(
        &dest_file,
        r#"{"type": "S3", "config": {"bucket": "test-bucket"}}"#,
    )
    .unwrap();

    h.cmd()
        .args([
            "batch-export",
            "create",
            "--name",
            "New Export",
            "--destination-file",
            dest_file.to_str().unwrap(),
            "--interval",
            "hour",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Export"));
}

// ── 4. update ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_update_name_patches() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/batch_exports/exp-55/"))
        .and(body_partial_json(json!({"name": "Renamed Export"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(export_fixture(
            "exp-55",
            "Renamed Export",
            "day",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "batch-export",
            "update",
            "exp-55",
            "--name",
            "Renamed Export",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed Export\""));
}

// ── 5. delete (hard delete) ───────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_delete_issues_hard_delete() {
    let h = TestHarness::new().await;

    // batch_exports is NOT in SOFT_DELETE_RESOURCES — expect true DELETE
    Mock::given(method("DELETE"))
        .and(path("/api/environments/999999/batch_exports/exp-77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "batch-export", "delete", "exp-77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("exp-77"));
}

// ── 6. pause ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_pause_patches_paused_true() {
    let h = TestHarness::new().await;
    let mut paused_fixture = export_fixture("exp-p1", "S3 Export", "hour");
    paused_fixture["paused"] = json!(true);
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/batch_exports/exp-p1/"))
        .and(body_partial_json(json!({"paused": true})))
        .respond_with(ResponseTemplate::new(200).set_body_json(paused_fixture))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "batch-export", "pause", "exp-p1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"paused\":true"));
}

// ── 7. unpause ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_unpause_patches_paused_false() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/batch_exports/exp-p2/"))
        .and(body_partial_json(json!({"paused": false})))
        .respond_with(ResponseTemplate::new(200).set_body_json(export_fixture(
            "exp-p2",
            "S3 Export",
            "hour",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "batch-export", "unpause", "exp-p2", "--json"])
        .assert()
        .success()
        .stdout(contains("\"paused\":false"));
}

// ── 8. backfills list ─────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_backfills_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/batch_exports/exp-b1/backfills/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "results": [{"id": "bf-1", "start_at": "2026-01-01T00:00:00Z", "status": "running"}]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["batch-export", "backfills", "list", "exp-b1", "--json"])
        .assert()
        .success()
        .stdout(contains("bf-1"));
}

// ── 9. backfills create ───────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_backfills_create() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/batch_exports/exp-b2/backfills/",
        ))
        .and(body_partial_json(
            json!({"start_at": "2026-01-01T00:00:00Z"}),
        ))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(json!({"id": "bf-new", "status": "starting"})),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "batch-export",
            "backfills",
            "create",
            "exp-b2",
            "--start-at",
            "2026-01-01T00:00:00Z",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("bf-new"));
}

// ── 10. backfills cancel ──────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_backfills_cancel() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/batch_exports/exp-b3/backfills/bf-99/cancel/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "cancelled"})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "batch-export",
            "backfills",
            "cancel",
            "exp-b3",
            "bf-99",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("cancelled"));
}

// ── 11. runs list ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_runs_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/batch_exports/exp-r1/runs/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "results": [{"id": "run-1", "status": "completed"}]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["batch-export", "runs", "list", "exp-r1", "--json"])
        .assert()
        .success()
        .stdout(contains("run-1"));
}

// ── 12. runs get ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_runs_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/batch_exports/exp-r2/runs/run-42/",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"id": "run-42", "status": "running"})),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["batch-export", "runs", "get", "exp-r2", "run-42", "--json"])
        .assert()
        .success()
        .stdout(contains("run-42"));
}

// ── 13. runs logs ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_runs_logs() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/batch_exports/exp-r3/runs/run-5/logs/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"timestamp": "2026-04-01T00:00:00Z", "level": "INFO", "message": "run started"}
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["batch-export", "runs", "logs", "exp-r3", "run-5", "--json"])
        .assert()
        .success()
        .stdout(contains("run started"));
}

// ── 14. runs cancel ───────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_runs_cancel() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/batch_exports/exp-r4/runs/run-10/cancel/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "cancelled"})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "batch-export",
            "runs",
            "cancel",
            "exp-r4",
            "run-10",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("cancelled"));
}

// ── 15. runs retry ────────────────────────────────────────────────────────────

#[tokio::test]
async fn batch_export_runs_retry() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/batch_exports/exp-r5/runs/run-20/retry/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"status": "retrying"})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "batch-export",
            "runs",
            "retry",
            "exp-r5",
            "run-20",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("retrying"));
}

// ── 16. destructive op requires --yes ────────────────────────────────────────

#[tokio::test]
async fn batch_export_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19994"
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
        .args(["batch-export", "delete", "exp-x", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY"
    );
}
