use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn query_run_async_polls_until_complete() {
    let h = TestHarness::new().await;

    // Enqueue response
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query_status": { "id": "q-1", "complete": false }
        })))
        .up_to_n_times(1)
        .mount(&h.server)
        .await;

    // First poll: still running
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/query/q-1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query_status": { "id": "q-1", "complete": false }
        })))
        .up_to_n_times(1)
        .mount(&h.server)
        .await;

    // Second poll: complete with results
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/query/q-1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query_status": {
                "id": "q-1",
                "complete": true,
                "results": [[1]],
                "columns": ["n"],
                "types": ["UInt8"]
            }
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["query", "run", "SELECT 1", "--async", "--json"])
        .assert()
        .success()
        .stdout(contains("\"columns\":[\"n\"]"));
}
