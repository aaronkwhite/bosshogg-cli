//! Integration tests for `bosshogg llm-analytics` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn eval_report_fixture(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "evaluation": "ev-uuid-123",
        "frequency": "scheduled",
        "enabled": true,
        "deleted": false,
        "next_delivery_date": "2026-05-01T08:00:00Z",
        "last_delivered_at": null,
        "rrule": "FREQ=WEEKLY;BYDAY=MO",
        "timezone_name": "America/New_York",
        "delivery_targets": [{"type": "email", "value": "team@example.com"}],
        "max_sample_size": 100
    })
}

fn provider_key_fixture(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "provider": "openai",
        "name": "OpenAI Production",
        "state": "ok",
        "error_message": null,
        "api_key_masked": "sk-****1234",
        "created_at": "2026-04-01T00:00:00Z",
        "last_used_at": "2026-04-20T10:00:00Z",
        "created_by": {"id": 1, "email": "admin@example.com"}
    })
}

fn review_queue_item_fixture(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "trace_id": "trace-abc-123",
        "created_at": "2026-04-01T00:00:00Z",
        "updated_at": null,
        "status": "pending",
        "queue": {"id": "q-1", "name": "Default"}
    })
}

// ── models list ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_models_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/llm_analytics/models/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "models": ["gpt-4o", "gpt-4-turbo", "gpt-3.5-turbo"]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["llm-analytics", "models", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("gpt-4o"));
}

// ── evaluation-summary ────────────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_summary() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_summary/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "overall_assessment": "Generally good quality with some issues.",
            "pass_patterns": [],
            "fail_patterns": [],
            "na_patterns": [],
            "recommendations": ["Add more test cases"],
            "statistics": {
                "total_analyzed": 10,
                "pass_count": 8,
                "fail_count": 2,
                "na_count": 0
            }
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "llm-analytics",
            "evaluation-summary",
            "--evaluation-id",
            "ev-uuid-999",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("Generally good quality"));
}

// ── evaluation-reports list ───────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_reports_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_reports/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                eval_report_fixture("er-1"),
                eval_report_fixture("er-2")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["llm-analytics", "evaluation-reports", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("er-1"))
        .stdout(contains("er-2"));
}

// ── evaluation-reports get ────────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_reports_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_reports/er-abc/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(eval_report_fixture("er-abc")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "llm-analytics",
            "evaluation-reports",
            "get",
            "er-abc",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"id\":\"er-abc\""))
        .stdout(contains("ev-uuid-123"));
}

// ── evaluation-reports create ─────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_reports_create() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_reports/",
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(eval_report_fixture("er-new")))
        .mount(&h.server)
        .await;

    let config_file = h.config_path.parent().unwrap().join("er_config.json");
    std::fs::write(
        &config_file,
        r#"{"evaluation":"ev-uuid-1","frequency":"scheduled","enabled":true}"#,
    )
    .unwrap();

    h.cmd()
        .args([
            "--yes",
            "llm-analytics",
            "evaluation-reports",
            "create",
            "--config-file",
            config_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("er-new"));
}

// ── evaluation-reports update ─────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_reports_update() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_reports/er-upd/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(eval_report_fixture("er-upd")))
        .mount(&h.server)
        .await;

    let config_file = h.config_path.parent().unwrap().join("er_upd_config.json");
    std::fs::write(&config_file, r#"{"enabled":false}"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "llm-analytics",
            "evaluation-reports",
            "update",
            "er-upd",
            "--config-file",
            config_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("er-upd"));
}

// ── evaluation-reports delete (soft) ──────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_reports_delete() {
    let h = TestHarness::new().await;
    // delete sends PATCH {"deleted": true}
    Mock::given(method("PATCH"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_reports/er-del/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "er-del",
            "deleted": true
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "llm-analytics",
            "evaluation-reports",
            "delete",
            "er-del",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"soft-delete\""));
}

// ── evaluation-reports generate ───────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_reports_generate() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_reports/er-gen/generate/",
        ))
        .respond_with(ResponseTemplate::new(202))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "llm-analytics",
            "evaluation-reports",
            "generate",
            "er-gen",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"generate\""));
}

// ── evaluation-reports runs ───────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_evaluation_reports_runs() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/llm_analytics/evaluation_reports/er-runs/runs/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [
                {
                    "id": "run-1",
                    "status": "completed",
                    "created_at": "2026-04-20T10:00:00Z"
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "llm-analytics",
            "evaluation-reports",
            "runs",
            "er-runs",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("run-1"))
        .stdout(contains("completed"));
}

// ── provider-keys list ────────────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_provider_keys_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/llm_analytics/provider_keys/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                provider_key_fixture("pk-1"),
                provider_key_fixture("pk-2")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["llm-analytics", "provider-keys", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("pk-1"))
        .stdout(contains("pk-2"));
}

// ── provider-keys get ─────────────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_provider_keys_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/llm_analytics/provider_keys/pk-abc/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(provider_key_fixture("pk-abc")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["llm-analytics", "provider-keys", "get", "pk-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"pk-abc\""))
        .stdout(contains("openai"));
}

// ── provider-keys validate ────────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_provider_keys_validate() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/llm_analytics/provider_keys/pk-val/validate/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "pk-val",
            "provider": "openai",
            "name": "My Key",
            "state": "ok",
            "error_message": null,
            "api_key_masked": "sk-****abcd",
            "created_at": "2026-04-01T00:00:00Z",
            "last_used_at": null,
            "created_by": {"id": 1}
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "llm-analytics",
            "provider-keys",
            "validate",
            "pk-val",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"state\":\"ok\""));
}

// ── review-queue list ─────────────────────────────────────────────────────────

#[tokio::test]
async fn llm_analytics_review_queue_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/llm_analytics/review_queue_items/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                review_queue_item_fixture("rqi-1"),
                review_queue_item_fixture("rqi-2")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["llm-analytics", "review-queue", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("rqi-1"))
        .stdout(contains("rqi-2"));
}
