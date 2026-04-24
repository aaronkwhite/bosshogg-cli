use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_evaluate_posts_to_flags_v2_with_project_token() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/flags"))
        .and(body_partial_json(json!({
            "api_key": "phc_project",
            "distinct_id": "u-1"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "featureFlags": { "checkout-redesign": true, "homepage-v2": "variant-a" }
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "flag",
            "evaluate",
            "--distinct-id",
            "u-1",
            "--project-token",
            "phc_project",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("checkout-redesign"))
        .stdout(contains("variant-a"));
}
