use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_list_filters_and_prints_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/"))
        .and(query_param("active", "true"))
        .and(query_param("search", "checkout"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [{
                "id": 101, "key": "checkout-redesign", "name": "Checkout v2",
                "active": true, "filters": { "groups": [] }, "rollout_percentage": 25
            }]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["flag", "list", "--active", "--search", "checkout", "--json"])
        .assert()
        .success()
        .stdout(contains("\"key\":\"checkout-redesign\""))
        .stdout(contains("\"count\":1"));
}
