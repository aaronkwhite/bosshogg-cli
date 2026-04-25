//! Integration tests for `bosshogg experiment` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn experiment_fixture(id: i64, name: &str, flag_key: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test experiment",
        "start_date": "2026-01-01T00:00:00Z",
        "end_date": null,
        "feature_flag_key": flag_key,
        "feature_flag": null,
        "exposure_cohort": null,
        "parameters": {"minimum_detectable_effect": 0.05},
        "secondary_metrics": [],
        "metrics": [],
        "saved_metrics": [],
        "archived": false,
        "deleted": false,
        "filters": {},
        "created_at": "2026-01-01T00:00:00Z",
        "created_by": {"id": 1, "email": "test@example.com"},
        "updated_at": "2026-04-01T00:00:00Z"
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn experiment_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/experiments/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                experiment_fixture(1, "Checkout Flow", "checkout-flow"),
                experiment_fixture(2, "Onboarding Test", "onboarding-test")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["experiment", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Checkout Flow"))
        .stdout(contains("Onboarding Test"));
}

// ── 2. get by numeric id ──────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/projects/999999/experiments/42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(experiment_fixture(
            42,
            "My Experiment",
            "my-flag",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["experiment", "get", "42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":42"))
        .stdout(contains("My Experiment"));
}

// ── 3. create experiment ──────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_create_with_parameters_file() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/"))
        .and(body_partial_json(json!({
            "name": "New Experiment",
            "feature_flag_key": "new-flag"
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(experiment_fixture(
            100,
            "New Experiment",
            "new-flag",
        )))
        .mount(&h.server)
        .await;

    // Write a parameters JSON file.
    let params_file = h.config_path.parent().unwrap().join("params.json");
    std::fs::write(&params_file, r#"{"minimum_detectable_effect": 0.05}"#).unwrap();

    h.cmd()
        .args([
            "experiment",
            "create",
            "--name",
            "New Experiment",
            "--feature-flag-key",
            "new-flag",
            "--parameters-file",
            params_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Experiment"));
}

// ── 4. update (name change) ───────────────────────────────────────────────────

#[tokio::test]
async fn experiment_update_name_patches() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/experiments/55/"))
        .and(body_partial_json(json!({"name": "Renamed Experiment"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(experiment_fixture(
            55,
            "Renamed Experiment",
            "my-flag",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "experiment",
            "update",
            "55",
            "--name",
            "Renamed Experiment",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed Experiment\""));
}

// ── 5. delete (hard delete) ───────────────────────────────────────────────────

#[tokio::test]
async fn experiment_delete_issues_hard_delete() {
    let h = TestHarness::new().await;

    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/experiments/77/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "delete", "77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("\"id\":77"));
}

// ── 6. archive experiment ─────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_archive_posts_to_archive_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/10/archive/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"archived": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "archive", "10", "--json"])
        .assert()
        .success()
        .stdout(contains("\"archived\":true"));
}

// ── 7. duplicate experiment ───────────────────────────────────────────────────

#[tokio::test]
async fn experiment_duplicate_posts_to_duplicate_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/20/duplicate/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(experiment_fixture(
            201,
            "Copy of My Experiment",
            "my-flag-copy",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "duplicate", "20", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":201"));
}

// ── 8. copy-to-project ────────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_copy_to_project_posts_with_team_id() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/30/copy_to_project/"))
        .and(body_partial_json(json!({"team_id": "999"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "experiment",
            "copy-to-project",
            "30",
            "--target-project-id",
            "999",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 9. create-exposure-cohort ─────────────────────────────────────────────────

#[tokio::test]
async fn experiment_create_exposure_cohort_posts_to_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path(
            "/api/projects/999999/experiments/40/create_exposure_cohort_for_experiment/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"cohort_id": 77})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "experiment",
            "create-exposure-cohort",
            "40",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"cohort_id\":77"));
}

// ── 10. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn experiment_delete_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19990"
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
        .args(["experiment", "delete", "42", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 11. launch experiment ─────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_launch_posts_to_launch_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/50/launch/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "launch", "50", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 12. end experiment ────────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_end_posts_to_end_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/51/end/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "end", "51", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 13. pause experiment ──────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_pause_posts_to_pause_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/52/pause/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "pause", "52", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 14. resume experiment ─────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_resume_posts_to_resume_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/53/resume/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "resume", "53", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 15. reset experiment ──────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_reset_posts_to_reset_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/54/reset/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "experiment", "reset", "54", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 16. ship-variant ──────────────────────────────────────────────────────────

#[tokio::test]
async fn experiment_ship_variant_posts_variant_key() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/experiments/55/ship_variant/"))
        .and(body_partial_json(json!({"variant_key": "test"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "experiment",
            "ship-variant",
            "55",
            "--variant",
            "test",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 17. recalculate-timeseries ────────────────────────────────────────────────

#[tokio::test]
async fn experiment_recalculate_timeseries_posts_to_endpoint() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path(
            "/api/projects/999999/experiments/56/recalculate_timeseries/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "experiment",
            "recalculate-timeseries",
            "56",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── 18. results (read-only, no --yes required) ────────────────────────────────

#[tokio::test]
async fn experiment_results_gets_timeseries_results() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/projects/999999/experiments/57/timeseries_results/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"variant": "test", "count": 100}]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "experiment",
            "results",
            "57",
            "--metric-uuid",
            "abc-123",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"results\""));
}

// ── 11. update requires --yes ─────────────────────────────────────────────────

#[tokio::test]
async fn experiment_update_without_yes_blocked_in_non_tty() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19991"
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
        .args(["experiment", "update", "42", "--name", "New Name", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "update without --yes should not succeed in non-TTY"
    );
}
