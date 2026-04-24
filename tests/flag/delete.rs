use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_delete_issues_patch_deleted_true() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/"))
        .and(query_param("search", "checkout"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [{ "id": 101, "key": "checkout", "active": true, "filters": {} }]
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/feature_flags/101/"))
        .and(body_partial_json(json!({ "deleted": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 101, "key": "checkout", "deleted": true, "filters": {}
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["flag", "delete", "checkout", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"deleted\":\"checkout\""));
}
