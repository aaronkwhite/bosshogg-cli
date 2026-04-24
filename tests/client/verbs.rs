//! Wiremock-backed HTTP verb tests. `https_only(true)` in the client would
//! refuse wiremock's http URL, so these tests construct a client via
//! `Client::for_test` which skips the https_only guard.

use bosshogg::client::{Client, ResolvedAuth};
use serde::Deserialize;
use serde_json::json;
use wiremock::matchers::{bearer_token, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Flag {
    id: i64,
    key: String,
}

fn mk_client(base_url: &str) -> Client {
    let auth = ResolvedAuth {
        api_key: "phx_test".into(),
        host: base_url.into(),
        project_id: Some("1".into()),
        env_id: Some("1".into()),
        org_id: None,
        context_name: Some("test".into()),
    };
    Client::for_test(auth, false).unwrap()
}

#[tokio::test]
async fn get_deserializes_typed_struct() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1/feature_flags/42/"))
        .and(bearer_token("phx_test"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 42, "key": "foo"})))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let flag: Flag = c.get("/api/projects/1/feature_flags/42/").await.unwrap();
    assert_eq!(
        flag,
        Flag {
            id: 42,
            key: "foo".into()
        }
    );
}

#[tokio::test]
async fn rate_limit_retries_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 1, "key": "ok"})))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let flag: Flag = c.get("/x").await.unwrap();
    assert_eq!(flag.id, 1);
}

#[tokio::test]
async fn rate_limit_gives_up_after_max_attempts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(429).insert_header("Retry-After", "0"))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let err = c.get::<Flag>("/x").await.unwrap_err();
    assert_eq!(err.error_code(), "RATE_LIMITED");
    assert_eq!(err.retry_after_s(), Some(0));
}

#[tokio::test]
async fn four_oh_three_with_scope_maps_to_auth_scope() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "type": "authentication_error",
            "code": "permission_denied",
            "detail": "API key is missing scope 'query:read'"
        })))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let err = c.get::<Flag>("/x").await.unwrap_err();
    assert_eq!(err.error_code(), "AUTH_SCOPE");
}

#[tokio::test]
async fn four_oh_four_maps_to_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let err = c.get::<Flag>("/x").await.unwrap_err();
    assert_eq!(err.error_code(), "NOT_FOUND");
}

#[tokio::test]
async fn five_oh_three_maps_to_upstream() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/x"))
        .respond_with(ResponseTemplate::new(503))
        .expect(3)
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let err = c.get::<Flag>("/x").await.unwrap_err();
    assert_eq!(err.error_code(), "UPSTREAM");
}

#[tokio::test]
async fn post_patch_delete_smoke() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/1/feature_flags/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({"id": 1, "key": "new"})))
        .mount(&server)
        .await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/1/feature_flags/1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 1, "key": "new"})))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let created: Flag = c
        .post("/api/projects/1/feature_flags/", &json!({"key": "new"}))
        .await
        .unwrap();
    assert_eq!(created.id, 1);
    let updated: Flag = c
        .patch(
            "/api/projects/1/feature_flags/1/",
            &json!({"active": false}),
        )
        .await
        .unwrap();
    assert_eq!(updated.key, "new");
    c.delete("/api/projects/1/feature_flags/1/").await.unwrap();
}

#[allow(dead_code)] // deserialize-only; populated by serde in get_paginated tests
#[derive(Debug, Deserialize, PartialEq, Clone)]
struct Page {
    count: i64,
    next: Option<String>,
    previous: Option<String>,
    results: Vec<Flag>,
}

#[tokio::test]
async fn get_paginated_follows_next_until_exhausted() {
    let server = MockServer::start().await;
    let p2 = format!("{}/api/projects/1/feature_flags/?cursor=abc", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/projects/1/feature_flags/"))
        .and(wiremock::matchers::query_param("cursor", "abc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 3,
            "next": null,
            "previous": null,
            "results": [{"id": 3, "key": "c"}]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/projects/1/feature_flags/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 3,
            "next": p2,
            "previous": null,
            "results": [{"id": 1, "key": "a"}, {"id": 2, "key": "b"}]
        })))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let out: Vec<Flag> = c
        .get_paginated("/api/projects/1/feature_flags/", None)
        .await
        .unwrap();
    assert_eq!(out.len(), 3);
    assert_eq!(out[2].key, "c");
}

#[tokio::test]
async fn get_paginated_respects_limit_cap() {
    let server = MockServer::start().await;
    let p2 = format!("{}/api/projects/1/feature_flags/?cursor=abc", server.uri());
    Mock::given(method("GET"))
        .and(path("/api/projects/1/feature_flags/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 10,
            "next": p2,
            "previous": null,
            "results": [{"id": 1, "key": "a"}, {"id": 2, "key": "b"}]
        })))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let out: Vec<Flag> = c
        .get_paginated("/api/projects/1/feature_flags/", Some(1))
        .await
        .unwrap();
    assert_eq!(out.len(), 1);
}
