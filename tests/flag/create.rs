use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_create_posts_payload_and_returns_id() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/feature_flags/"))
        .and(body_partial_json(
            json!({ "key": "new-flag", "name": "New Flag" }),
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "id": 202, "key": "new-flag", "name": "New Flag", "active": false,
            "filters": { "groups": [] }
        })))
        .mount(&h.server)
        .await;

    let filters = h.config_path.parent().unwrap().join("filters.json");
    std::fs::write(&filters, r#"{"groups": []}"#).unwrap();

    h.cmd()
        .args([
            "flag",
            "create",
            "--key",
            "new-flag",
            "--name",
            "New Flag",
            "--filters-file",
            filters.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"id\":202"))
        .stdout(contains("\"ok\":true"));
}
