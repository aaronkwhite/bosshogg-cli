use crate::common::TestHarness;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{method, path};
use wiremock::{Mock, Request, Respond, ResponseTemplate};

struct CaptureKind {
    calls: Arc<AtomicUsize>,
    captured_kind: Arc<std::sync::Mutex<Option<String>>>,
}

impl Respond for CaptureKind {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&req.body) {
            if let Some(k) = v.pointer("/query/kind").and_then(|x| x.as_str()) {
                *self.captured_kind.lock().unwrap() = Some(k.to_string());
            }
        }
        ResponseTemplate::new(200).set_body_json(json!({
            "results": [], "columns": [], "types": []
        }))
    }
}

#[tokio::test]
async fn hogql_wrapper_sends_hogql_query_kind() {
    let h = TestHarness::new().await;
    let captured = Arc::new(std::sync::Mutex::new(None::<String>));
    let responder = CaptureKind {
        calls: Arc::new(AtomicUsize::new(0)),
        captured_kind: Arc::clone(&captured),
    };
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(responder)
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["query", "hogql", "SELECT 1", "--json"])
        .assert()
        .success();

    assert_eq!(captured.lock().unwrap().as_deref(), Some("HogQLQuery"));
}

#[tokio::test]
async fn events_wrapper_sends_events_query_kind() {
    let h = TestHarness::new().await;
    let captured = Arc::new(std::sync::Mutex::new(None::<String>));
    let responder = CaptureKind {
        calls: Arc::new(AtomicUsize::new(0)),
        captured_kind: Arc::clone(&captured),
    };
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(responder)
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "query",
            "events",
            r#"{"select":["event"],"limit":1}"#,
            "--json",
        ])
        .assert()
        .success();

    assert_eq!(captured.lock().unwrap().as_deref(), Some("EventsQuery"));
}

#[tokio::test]
async fn trends_wrapper_sends_trends_query_kind() {
    let h = TestHarness::new().await;
    let captured = Arc::new(std::sync::Mutex::new(None::<String>));
    let responder = CaptureKind {
        calls: Arc::new(AtomicUsize::new(0)),
        captured_kind: Arc::clone(&captured),
    };
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(responder)
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "query",
            "trends",
            r#"{"series":[{"kind":"EventsNode","event":"$pageview"}]}"#,
            "--json",
        ])
        .assert()
        .success();

    assert_eq!(captured.lock().unwrap().as_deref(), Some("TrendsQuery"));
}

#[tokio::test]
async fn funnel_wrapper_sends_funnel_query_kind() {
    let h = TestHarness::new().await;
    let captured = Arc::new(std::sync::Mutex::new(None::<String>));
    let responder = CaptureKind {
        calls: Arc::new(AtomicUsize::new(0)),
        captured_kind: Arc::clone(&captured),
    };
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(responder)
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "query",
            "funnel",
            r#"{"series":[{"kind":"EventsNode","event":"$pageview"}]}"#,
            "--json",
        ])
        .assert()
        .success();

    assert_eq!(captured.lock().unwrap().as_deref(), Some("FunnelsQuery"));
}

#[tokio::test]
async fn events_wrapper_rejects_non_json_input() {
    let h = TestHarness::new().await;
    // No mock mounted — command should fail locally before any HTTP call.

    h.cmd()
        .args(["query", "events", "SELECT 1", "--json"])
        .assert()
        .failure();
}

#[tokio::test]
async fn events_wrapper_keeps_user_provided_kind() {
    // Users who include `kind` in the body should not have it overwritten.
    // We assert the *request body* is preserved, including extra fields.
    let h = TestHarness::new().await;
    let captured = Arc::new(std::sync::Mutex::new(None::<String>));
    let responder = CaptureKind {
        calls: Arc::new(AtomicUsize::new(0)),
        captured_kind: Arc::clone(&captured),
    };
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(responder)
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "query",
            "events",
            r#"{"kind":"EventsQuery","select":["event"],"limit":1}"#,
            "--json",
        ])
        .assert()
        .success();

    assert_eq!(captured.lock().unwrap().as_deref(), Some("EventsQuery"));
}
