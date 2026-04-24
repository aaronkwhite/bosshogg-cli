//! HogQL `Client::query` tests: sync path, async polling, auto-LIMIT injection,
//! error surface for malformed SQL.

use bosshogg::client::{Client, QueryKind, ResolvedAuth};
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn mk_client(base_url: &str) -> Client {
    let auth = ResolvedAuth {
        api_key: "phx_test".into(),
        host: base_url.into(),
        project_id: Some("1".into()),
        env_id: Some("42".into()),
        org_id: None,
        context_name: Some("test".into()),
    };
    Client::for_test(auth, false).unwrap()
}

#[tokio::test]
async fn sync_query_returns_typed_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/42/query/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [["foo", 5], ["bar", 3]],
            "columns": ["event", "c"],
            "types": ["String", "UInt64"],
            "hogql": "SELECT event, count() AS c FROM events GROUP BY event\nLIMIT 100"
        })))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let r = c
        .query(
            "SELECT event, count() AS c FROM events GROUP BY event",
            QueryKind::HogQL,
            false,
        )
        .await
        .unwrap();
    assert_eq!(r.columns, vec!["event", "c"]);
    assert_eq!(r.results.len(), 2);
    assert!(r.hogql.unwrap().contains("LIMIT 100"));
}

#[tokio::test]
async fn sync_query_preserves_existing_limit() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/42/query/"))
        .and(body_partial_json(json!({
            "query": {
                "kind": "HogQLQuery",
                "query": "SELECT 1 FROM events LIMIT 5"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [[1]],
            "columns": ["c"],
            "types": ["UInt64"]
        })))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let _ = c
        .query("SELECT 1 FROM events LIMIT 5", QueryKind::HogQL, false)
        .await
        .unwrap();
}

#[tokio::test]
async fn async_query_polls_to_completion() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/42/query/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query_status": {
                "id": "qid-1",
                "complete": false
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/environments/42/query/qid-1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query_status": {"id": "qid-1", "complete": false}
        })))
        .up_to_n_times(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/environments/42/query/qid-1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "query_status": {
                "id": "qid-1",
                "complete": true,
                "results": [[1, 2], [3, 4]],
                "columns": ["a", "b"],
                "types": ["UInt64", "UInt64"]
            }
        })))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let r = c.query("SELECT 1", QueryKind::HogQL, true).await.unwrap();
    assert_eq!(r.results.len(), 2);
    assert_eq!(r.columns, vec!["a", "b"]);
}

#[tokio::test]
async fn hogql_syntax_error_maps_to_hogql_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/42/query/"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "type": "validation_error",
            "code": "invalid_query",
            "detail": "SyntaxError: line 1 col 8: expected expression"
        })))
        .mount(&server)
        .await;

    let c = mk_client(&server.uri());
    let err = c
        .query("SELECT", QueryKind::HogQL, false)
        .await
        .unwrap_err();
    assert_eq!(err.error_code(), "BAD_REQUEST");
}
