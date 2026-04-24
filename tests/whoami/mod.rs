use crate::common::TestHarness;
use predicates::str::contains;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[tokio::test]
async fn whoami_prints_user_and_team() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/users/@me/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "email": "alice@example.com",
            "first_name": "Alice",
            "uuid": "u-1",
            "organization": { "id": "org-1", "name": "Acme" },
            "team": { "id": 999999, "name": "main" },
            "scopes": ["query:read", "feature_flag:write"]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["whoami", "--json"])
        .assert()
        .success()
        .stdout(contains("alice@example.com"))
        .stdout(contains("999999"))
        .stdout(contains("query:read"));
}
