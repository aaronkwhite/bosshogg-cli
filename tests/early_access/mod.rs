//! Integration tests for `bosshogg early-access` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn ea_fixture(id: &str, name: &str, stage: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "A test early access feature",
        "stage": stage,
        "feature_flag": {"id": 99, "key": "my-flag"},
        "feature_flag_id": 99,
        "documentation_url": "https://docs.example.com/feature",
        "created_at": "2026-01-01T00:00:00Z"
    })
}

// ── 1. list returns typed vec ─────────────────────────────────────────────────

#[tokio::test]
async fn early_access_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/early_access_feature/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                ea_fixture("ea-uuid-1", "Dark Mode", "beta"),
                ea_fixture("ea-uuid-2", "AI Assistant", "alpha")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["early-access", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Dark Mode"))
        .stdout(contains("AI Assistant"));
}

// ── 2. get by UUID ────────────────────────────────────────────────────────────

#[tokio::test]
async fn early_access_get_by_id() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path(
            "/api/projects/999999/early_access_feature/ea-uuid-42/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(ea_fixture(
            "ea-uuid-42",
            "My Feature",
            "beta",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["early-access", "get", "ea-uuid-42", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"ea-uuid-42\""))
        .stdout(contains("My Feature"));
}

// ── 3. create early access feature ───────────────────────────────────────────

#[tokio::test]
async fn early_access_create_with_required_fields() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/projects/999999/early_access_feature/"))
        .and(body_partial_json(json!({
            "name": "New Feature",
            "stage": "alpha",
            "feature_flag_id": 55
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(ea_fixture(
            "ea-uuid-new",
            "New Feature",
            "alpha",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "early-access",
            "create",
            "--name",
            "New Feature",
            "--description",
            "A new alpha feature",
            "--stage",
            "alpha",
            "--feature-flag-id",
            "55",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("New Feature"));
}

// ── 4. update (stage change) ──────────────────────────────────────────────────

#[tokio::test]
async fn early_access_update_stage_patches() {
    let h = TestHarness::new().await;

    Mock::given(method("PATCH"))
        .and(path(
            "/api/projects/999999/early_access_feature/ea-uuid-55/",
        ))
        .and(body_partial_json(json!({"stage": "beta"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(ea_fixture(
            "ea-uuid-55",
            "My Feature",
            "beta",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "early-access",
            "update",
            "ea-uuid-55",
            "--stage",
            "beta",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"stage\":\"beta\""));
}

// ── 5. delete (hard delete) ───────────────────────────────────────────────────

#[tokio::test]
async fn early_access_delete_issues_hard_delete() {
    let h = TestHarness::new().await;

    Mock::given(method("DELETE"))
        .and(path(
            "/api/projects/999999/early_access_feature/ea-uuid-77/",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "early-access", "delete", "ea-uuid-77", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("ea-uuid-77"));
}

// ── 6. destructive op requires --yes ─────────────────────────────────────────

#[tokio::test]
async fn early_access_delete_without_yes_blocked_in_non_tty() {
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
        // NOT passing --yes
        .args(["early-access", "delete", "ea-uuid-x", "--json"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "delete without --yes should not succeed in non-TTY: stdout={}",
        String::from_utf8_lossy(&output.stdout)
    );
}

// ── 7. update requires --yes ──────────────────────────────────────────────────

#[tokio::test]
async fn early_access_update_without_yes_blocked_in_non_tty() {
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
        .args(["early-access", "update", "ea-uuid-x", "--stage", "beta"])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "update without --yes should not succeed in non-TTY"
    );
}
