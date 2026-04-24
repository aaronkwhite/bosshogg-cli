use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_dependents_returns_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/"))
        .and(query_param("search", "root"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [{ "id": 50, "key": "root", "active": true, "filters": {} }]
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path(
            "/api/projects/999999/feature_flags/50/dependent_flags/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": 51, "key": "child-a" },
            { "id": 52, "key": "child-b" }
        ])))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["flag", "dependents", "root", "--json"])
        .assert()
        .success()
        .stdout(contains("child-a"))
        .stdout(contains("child-b"));
}

#[tokio::test]
async fn flag_activity_returns_log() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/"))
        .and(query_param("search", "root"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [{ "id": 50, "key": "root", "active": true, "filters": {} }]
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/feature_flags/50/activity/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [ { "activity": "updated", "created_at": "2026-04-21T10:00:00Z", "user": { "email": "alice@example.com" } } ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["flag", "activity", "root", "--json"])
        .assert()
        .success()
        .stdout(contains("alice@example.com"));
}
