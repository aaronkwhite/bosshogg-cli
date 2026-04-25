//! Integration tests for `bosshogg evaluation` subcommands.
//!
//! Note: evaluations use `/api/environments/{project_id}/evaluations/`
//! (the `env_id` configured in the test harness = "999999").

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn evaluation_fixture(id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "Test evaluation",
        "enabled": true,
        "source": "return true",
        "created_at": "2026-04-01T00:00:00Z",
        "updated_at": "2026-04-02T00:00:00Z",
        "deleted": false,
        "created_by": {"id": 1, "email": "admin@example.com"}
    })
}

// ── evaluation list ───────────────────────────────────────────────────────────

#[tokio::test]
async fn evaluation_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/evaluations/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                evaluation_fixture("ev-1", "Quality Check"),
                evaluation_fixture("ev-2", "Safety Check")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["evaluation", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("ev-1"))
        .stdout(contains("ev-2"));
}

// ── evaluation get ────────────────────────────────────────────────────────────

#[tokio::test]
async fn evaluation_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/evaluations/ev-abc/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(evaluation_fixture("ev-abc", "My Eval")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["evaluation", "get", "ev-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"ev-abc\""))
        .stdout(contains("My Eval"));
}

// ── evaluation test-hog ───────────────────────────────────────────────────────

#[tokio::test]
async fn evaluation_test_hog() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/evaluations/test_hog/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "event_uuid": "evt-uuid-1",
                    "trace_id": "trace-abc",
                    "input_preview": "Hello, how are you?",
                    "output_preview": "I am fine, thanks.",
                    "result": true,
                    "reasoning": null,
                    "error": null
                },
                {
                    "event_uuid": "evt-uuid-2",
                    "trace_id": null,
                    "input_preview": "What is 2+2?",
                    "output_preview": "5",
                    "result": false,
                    "reasoning": null,
                    "error": null
                }
            ],
            "message": ""
        })))
        .mount(&h.server)
        .await;

    let hog_file = h.config_path.parent().unwrap().join("eval.hog");
    std::fs::write(&hog_file, "return true").unwrap();

    h.cmd()
        .args([
            "evaluation",
            "test-hog",
            "--hog-file",
            hog_file.to_str().unwrap(),
            "--sample-count",
            "2",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("evt-uuid-1"))
        .stdout(contains("evt-uuid-2"));
}

// ── test-hog human-readable output ───────────────────────────────────────────

#[tokio::test]
async fn evaluation_test_hog_human_output() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/evaluations/test_hog/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "event_uuid": "evt-human",
                    "trace_id": null,
                    "input_preview": "test",
                    "output_preview": "pass",
                    "result": true,
                    "reasoning": null,
                    "error": null
                }
            ],
            "message": "Tested 1 event"
        })))
        .mount(&h.server)
        .await;

    let hog_file = h.config_path.parent().unwrap().join("eval_human.hog");
    std::fs::write(&hog_file, "return true").unwrap();

    h.cmd()
        .args([
            "evaluation",
            "test-hog",
            "--hog-file",
            hog_file.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(contains("evt-human"))
        .stdout(contains("PASS"));
}
