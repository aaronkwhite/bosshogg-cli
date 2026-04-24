use crate::common::TestHarness;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn doctor_runs_all_checks_and_emits_json_array() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/users/@me/"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("date", "Tue, 21 Apr 2026 12:00:00 GMT")
                .set_body_json(serde_json::json!({
                    "email": "a@b.com",
                    "team": { "id": 999999 },
                    "organization": { "id": "org-1" }
                })),
        )
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 999999, "name": "main"
        })))
        .mount(&h.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "id": 999999, "name": "main"
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["doctor", "--json"])
        .assert()
        .success()
        .stdout(contains("\"name\":\"key_alive\""))
        .stdout(contains("\"name\":\"project_access\""))
        .stdout(contains("\"name\":\"env_access\""))
        .stdout(contains("\"summary\":{\"ok\":true"));
}
