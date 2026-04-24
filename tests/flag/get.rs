use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_get_resolves_key_via_list_then_fetches_by_id() {
    let h = TestHarness::new().await;
    // List filtered by search=key resolves id
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/"))
        .and(query_param("search", "checkout-redesign"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [{
                "id": 101, "key": "checkout-redesign", "name": "Checkout",
                "active": true, "filters": {}
            }]
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/101/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 101, "key": "checkout-redesign", "name": "Checkout v2",
            "active": true, "filters": { "groups": [{"rollout_percentage": 25}] },
            "rollout_percentage": 25
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["flag", "get", "checkout-redesign", "--json"])
        .assert()
        .success()
        .stdout(contains("\"rollout_percentage\":25"))
        .stdout(contains("\"id\":101"));
}
