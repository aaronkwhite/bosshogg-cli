use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn query_run_inline_returns_rows() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .and(body_partial_json(json!({
            "query": { "kind": "HogQLQuery" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [["pageview", 42]],
            "columns": ["event", "cnt"],
            "types": ["String", "UInt64"],
            "hogql": "SELECT event, count() FROM events GROUP BY event LIMIT 100"
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "query",
            "run",
            "SELECT event, count() FROM events GROUP BY event",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"columns\":[\"event\",\"cnt\"]"))
        .stdout(contains("\"results\":[[\"pageview\",42]]"));
}
