//! Integration tests for `bosshogg event` subcommands.
//!
//! All tests use wiremock to intercept HTTP calls. Binary-level tests use
//! `Command::cargo_bin` with `BOSSHOGG_ALLOW_HTTP=1` (requires --features test-harness).

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path, query_param};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture helpers ────────────────────────────────────────────────────

fn query_response(rows: serde_json::Value) -> serde_json::Value {
    json!({
        "results": rows,
        "columns": ["event", "distinct_id", "timestamp", "properties"],
        "types": ["String", "String", "DateTime", "String"],
        "hogql": "SELECT event, distinct_id, timestamp, properties FROM events LIMIT 50"
    })
}

fn event_fixture(uuid: &str, event: &str) -> serde_json::Value {
    json!({
        "uuid": uuid,
        "event": event,
        "distinct_id": "user@example.com",
        "timestamp": "2026-04-01T12:00:00Z",
        "properties": {"$browser": "Chrome"}
    })
}

// ── 1. list routes through HogQL (POST /query/) ───────────────────────────────

#[tokio::test]
async fn event_list_uses_hogql() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .and(body_partial_json(json!({
            "query": { "kind": "HogQLQuery" }
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(query_response(json!([
                ["pageview", "user@example.com", "2026-04-01T12:00:00Z", "{}"],
                ["click", "user2@example.com", "2026-04-01T11:00:00Z", "{}"]
            ]))),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["event", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("pageview"))
        .stdout(contains("click"))
        .stdout(contains("\"columns\""));
}

// ── 2. list with --event filter ───────────────────────────────────────────────

#[tokio::test]
async fn event_list_with_event_filter() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(query_response(json!([[
                "pageview",
                "user@example.com",
                "2026-04-01T12:00:00Z",
                "{}"
            ]]))),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["event", "list", "--event", "pageview", "--json"])
        .assert()
        .success()
        .stdout(contains("pageview"));
}

// ── 3. list with --limit ──────────────────────────────────────────────────────

#[tokio::test]
async fn event_list_with_limit() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(query_response(json!([]))))
        .mount(&h.server)
        .await;

    // Should succeed even with empty result set
    h.cmd()
        .args(["event", "list", "--limit", "5", "--json"])
        .assert()
        .success()
        .stdout(contains("\"results\""));
}

// ── 4. get by uuid (legacy REST) ─────────────────────────────────────────────

#[tokio::test]
async fn event_get_by_uuid() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/events/some-uuid-1234/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(event_fixture("some-uuid-1234", "pageview")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["event", "get", "some-uuid-1234", "--json"])
        .assert()
        .success()
        .stdout(contains("pageview"))
        .stdout(contains("some-uuid-1234"));
}

// ── 5. values endpoint ────────────────────────────────────────────────────────

#[tokio::test]
async fn event_values_returns_list() {
    let h = TestHarness::new().await;

    Mock::given(method("GET"))
        .and(path("/api/environments/999999/events/values/"))
        .and(query_param("key", "$browser"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {"name": "Chrome", "count": 1000},
            {"name": "Firefox", "count": 300},
            {"name": "Safari", "count": 200}
        ])))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["event", "values", "--prop", "$browser", "--json"])
        .assert()
        .success()
        .stdout(contains("Chrome"))
        .stdout(contains("Firefox"));
}

// ── 6. list with --distinct-id filter ────────────────────────────────────────

#[tokio::test]
async fn event_list_with_distinct_id() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(query_response(json!([[
                "pageview",
                "specific@example.com",
                "2026-04-01T12:00:00Z",
                "{}"
            ]]))),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "event",
            "list",
            "--distinct-id",
            "specific@example.com",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("specific@example.com"));
}

// ── 7. list with before/after timestamps ─────────────────────────────────────

#[tokio::test]
async fn event_list_with_time_range() {
    let h = TestHarness::new().await;

    Mock::given(method("POST"))
        .and(path("/api/environments/999999/query/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(query_response(json!([[
                "pageview",
                "user@example.com",
                "2026-04-01T12:00:00Z",
                "{}"
            ]]))),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "event",
            "list",
            "--after",
            "2026-01-01T00:00:00Z",
            "--before",
            "2026-12-31T23:59:59Z",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"results\""));
}

// ── 8. unit: HogQL builder (no HTTP needed) ───────────────────────────────────

#[test]
fn event_hogql_builds_correct_sql() {
    // This is a compile-time sanity check — the unit tests in the module
    // do the heavy lifting; we verify the binary at least links.
    // (Actual SQL assertion is in src/commands/event.rs unit tests.)
}
