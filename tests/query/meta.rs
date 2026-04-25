use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn query_status_returns_running_state() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/query/q-1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query_status": { "id": "q-1", "complete": false }
        })))
        .mount(&h.server)
        .await;
    h.cmd()
        .args(["query", "status", "q-1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"complete\":false"));
}

#[tokio::test]
async fn query_cancel_issues_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path("/api/environments/999999/query/q-1/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;
    h.cmd()
        .args(["query", "cancel", "q-1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

#[tokio::test]
async fn query_log_returns_entries() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/query/q-1/log/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [ { "timestamp": "2026-04-21T12:00:00Z", "line": "plan accepted" } ]
        })))
        .mount(&h.server)
        .await;
    h.cmd()
        .args(["query", "log", "q-1", "--json"])
        .assert()
        .success()
        .stdout(contains("plan accepted"));
}
