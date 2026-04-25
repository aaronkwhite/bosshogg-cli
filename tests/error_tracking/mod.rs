//! Integration tests for `bosshogg error-tracking` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn fingerprint_fixture(id: &str, fp: &str) -> serde_json::Value {
    json!({
        "id": id,
        "fingerprint": fp,
        "status": "active",
        "last_seen": "2026-04-01T00:00:00Z",
        "first_seen": "2026-01-01T00:00:00Z",
        "occurrences": 15,
        "affected_users": 3,
        "assignee": null
    })
}

fn assignment_rule_fixture(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "filters": {"events": []},
        "assignee": {"id": "user-1", "email": "dev@example.com"},
        "order_key": 1
    })
}

fn grouping_rule_fixture(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "filters": {"type": "and", "values": []},
        "description": "Group by module",
        "assignee": null
    })
}

// ── fingerprints list ─────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_fingerprints_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/fingerprints/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                fingerprint_fixture("fp-1", "abc123"),
                fingerprint_fixture("fp-2", "def456")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "fingerprints", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("fp-1"))
        .stdout(contains("fp-2"));
}

// ── fingerprints get ──────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_fingerprints_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/fingerprints/fp-abc/",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(fingerprint_fixture("fp-abc", "hash999")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "fingerprints", "get", "fp-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"fp-abc\""))
        .stdout(contains("hash999"));
}

// ── assignment-rules list ─────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_assignment_rules_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/assignment_rules/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [assignment_rule_fixture("ar-1")]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "assignment-rules", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("ar-1"));
}

// ── assignment-rules create ───────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_assignment_rules_create() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/error_tracking/assignment_rules/",
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(assignment_rule_fixture("ar-new")))
        .mount(&h.server)
        .await;

    let filters_file = h.config_path.parent().unwrap().join("filters.json");
    std::fs::write(&filters_file, r#"{"events": []}"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "assignment-rules",
            "create",
            "--filters-file",
            filters_file.to_str().unwrap(),
            "--assignee-id",
            "user-uuid-1",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("ar-new"));
}

// ── assignment-rules get ──────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_assignment_rules_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/assignment_rules/ar-42/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(assignment_rule_fixture("ar-42")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "error-tracking",
            "assignment-rules",
            "get",
            "ar-42",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"id\":\"ar-42\""));
}

// ── assignment-rules delete ───────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_assignment_rules_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path(
            "/api/environments/999999/error_tracking/assignment_rules/ar-del/",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "assignment-rules",
            "delete",
            "ar-del",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("ar-del"));
}

// ── assignment-rules reorder ──────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_assignment_rules_reorder() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/error_tracking/assignment_rules/reorder/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    let order_file = h.config_path.parent().unwrap().join("order.json");
    std::fs::write(&order_file, r#"["ar-2", "ar-1"]"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "assignment-rules",
            "reorder",
            "--order-file",
            order_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── grouping-rules list ───────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_grouping_rules_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/grouping_rules/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [grouping_rule_fixture("gr-1")]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "grouping-rules", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("gr-1"));
}

// ── grouping-rules create ─────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_grouping_rules_create() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/error_tracking/grouping_rules/",
        ))
        .respond_with(ResponseTemplate::new(201).set_body_json(grouping_rule_fixture("gr-new")))
        .mount(&h.server)
        .await;

    let filters_file = h.config_path.parent().unwrap().join("grfilters.json");
    std::fs::write(&filters_file, r#"{"type":"and","values":[]}"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "grouping-rules",
            "create",
            "--filters-file",
            filters_file.to_str().unwrap(),
            "--description",
            "Group by module",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("gr-new"));
}

// ── grouping-rules get ────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_grouping_rules_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/grouping_rules/gr-99/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(grouping_rule_fixture("gr-99")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "grouping-rules", "get", "gr-99", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"gr-99\""));
}

// ── resolve-github ────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_resolve_github() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/resolve_github/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fingerprint": "abc123"})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "error-tracking",
            "resolve-github",
            "--organization",
            "myorg",
            "--repo",
            "myrepo",
            "--file",
            "src/main.rs",
            "--line",
            "42",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("abc123"));
}

// ── resolve-gitlab ────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_resolve_gitlab() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/resolve_gitlab/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"fingerprint": "gl-fp-1"})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "error-tracking",
            "resolve-gitlab",
            "--organization",
            "glorg",
            "--repo",
            "glrepo",
            "--file",
            "lib/foo.py",
            "--line",
            "10",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("gl-fp-1"));
}

// ── issues fixtures ───────────────────────────────────────────────────────────

fn issue_fixture(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": "NullPointerException",
        "status": "active",
        "last_seen": "2026-04-20T10:00:00Z",
        "first_seen": "2026-03-01T08:00:00Z",
        "occurrences": 42,
        "affected_users": 7,
        "assignee": null,
        "description": "Crash in request handler"
    })
}

// ── issues list ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/error_tracking/issues/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                issue_fixture("issue-1"),
                issue_fixture("issue-2")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "issues", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("issue-1"))
        .stdout(contains("issue-2"));
}

// ── issues get ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/issues/issue-abc/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(issue_fixture("issue-abc")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "issues", "get", "issue-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"issue-abc\""))
        .stdout(contains("NullPointerException"));
}

// ── issues activity ───────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_activity() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/issues/issue-x/activity/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "activity": "assigned",
                    "created_at": "2026-04-10T00:00:00Z",
                    "user": {"email": "dev@example.com"}
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "issues", "activity", "issue-x", "--json"])
        .assert()
        .success()
        .stdout(contains("assigned"));
}

// ── issues activity-list ──────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_activity_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/error_tracking/issues/activity/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {
                    "activity": "resolved",
                    "created_at": "2026-04-15T00:00:00Z",
                    "user": {"email": "admin@example.com"}
                }
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["error-tracking", "issues", "activity-list", "--json"])
        .assert()
        .success()
        .stdout(contains("resolved"));
}

// ── issues assign ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_assign() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/error_tracking/issues/issue-a/assign/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "issues",
            "assign",
            "issue-a",
            "--assignee-id",
            "42",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── issues cohort ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_cohort() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/error_tracking/issues/issue-b/cohort/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"cohort_id": "cohort-1"})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "issues",
            "cohort",
            "issue-b",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("cohort-1"));
}

// ── issues merge ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_merge() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/error_tracking/issues/issue-c/merge/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "issues",
            "merge",
            "issue-c",
            "--into",
            "issue-d",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── issues split ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_split() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/environments/999999/error_tracking/issues/issue-e/split/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    let fp_file = h.config_path.parent().unwrap().join("fingerprints.json");
    std::fs::write(&fp_file, r#"["fp-a","fp-b"]"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "issues",
            "split",
            "issue-e",
            "--fingerprints-file",
            fp_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── issues bulk ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn error_tracking_issues_bulk() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/environments/999999/error_tracking/issues/bulk/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .mount(&h.server)
        .await;

    let ids_file = h.config_path.parent().unwrap().join("issue_ids.json");
    std::fs::write(&ids_file, r#"["issue-1","issue-2"]"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "error-tracking",
            "issues",
            "bulk",
            "--ids-file",
            ids_file.to_str().unwrap(),
            "--action",
            "resolve",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"));
}

// ── destructive gating without --yes ─────────────────────────────────────────

#[tokio::test]
async fn error_tracking_create_without_yes_blocked() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19994"
api_key = "phx_testkey"
project_id = "1"
env_id = "1"
"#,
    )
    .unwrap();

    let filters_file = tmp.path().join("f.json");
    std::fs::write(&filters_file, r#"{"events":[]}"#).unwrap();

    let out = Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "error-tracking",
            "assignment-rules",
            "create",
            "--filters-file",
            filters_file.to_str().unwrap(),
            "--assignee-id",
            "uid-1",
        ])
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "create without --yes should fail in non-TTY"
    );
}
