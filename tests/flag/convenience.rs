use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_enable_patches_active_true() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/"))
        .and(query_param("search", "checkout"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [{ "id": 101, "key": "checkout", "active": false, "filters": {} }]
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/feature_flags/101/"))
        .and(body_partial_json(json!({ "active": true })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 101,
            "key": "checkout",
            "active": true,
            "filters": {}
        })))
        .mount(&h.server)
        .await;
    h.cmd()
        .args(["flag", "enable", "checkout", "--json"])
        .assert()
        .success()
        .stdout(contains("\"active\":true"));
}

#[tokio::test]
async fn flag_disable_patches_active_false() {
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
        .and(body_partial_json(json!({ "active": false })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 101,
            "key": "checkout",
            "active": false,
            "filters": {}
        })))
        .mount(&h.server)
        .await;
    h.cmd()
        .args(["flag", "disable", "checkout", "--json"])
        .assert()
        .success()
        .stdout(contains("\"active\":false"));
}

#[tokio::test]
async fn flag_rollout_25_patches_filter_groups() {
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
        .and(body_partial_json(json!({
            "filters": { "groups": [{ "rollout_percentage": 25 }] }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 101,
            "key": "checkout",
            "active": true,
            "filters": { "groups": [{ "rollout_percentage": 25 }] },
            "rollout_percentage": 25
        })))
        .mount(&h.server)
        .await;
    h.cmd()
        .args(["flag", "rollout", "checkout", "25", "--json"])
        .assert()
        .success()
        .stdout(contains("\"rollout_percentage\":25"));
}
