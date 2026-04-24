use crate::common::TestHarness;
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn flag_update_accepts_yes_flag() {
    let h = TestHarness::with_project("1", "1").await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1/feature_flags/"))
        .and(query_param("search", "foo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1, "next": null, "previous": null,
            "results": [{"id": 42, "key": "foo", "name": null, "active": true, "filters": {}}]
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/1/feature_flags/42/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": 42, "key": "foo", "active": false, "filters": {}
        })))
        .mount(&h.server)
        .await;

    let output = h.cmd()
        .args(["--yes", "--json", "flag", "update", "foo", "--disabled"])
        .output()
        .unwrap();

    // Must accept --yes at the clap level (no "unexpected argument").
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("unexpected argument"),
        "--yes rejected: {stderr}"
    );
    assert!(output.status.success(), "expected success: stderr={stderr}");
}
