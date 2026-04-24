//! Integration tests for `bosshogg session-recording` subcommands.
//!
//! CRITICAL safety check: the `snapshot_safety` test verifies that
//! snapshot blobs NEVER appear on stdout in default mode.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn recording_fixture(id: &str, distinct_id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "distinct_id": distinct_id,
        "viewed": false,
        "recording_duration": 90,
        "active_seconds": 60,
        "inactive_seconds": 30,
        "start_time": "2026-01-01T10:00:00Z",
        "end_time": "2026-01-01T10:01:30Z",
        "click_count": 3,
        "keypress_count": 10,
        "console_log_count": 0,
        "console_warn_count": 0,
        "console_error_count": 0,
        "start_url": "https://example.com/dashboard",
        "person": {"id": "person-1"},
        "storage": "object_storage",
        "pinned_count": 0,
        "ongoing": false,
        "activity_score": 5.5,
        "snapshot_source": "realtime"
    })
}

/// Same fixture but with a large snapshots field attached.
fn recording_with_snapshots_fixture(id: &str) -> serde_json::Value {
    let mut v = recording_fixture(id, "user-snap");
    v["snapshots"] = json!([
        {"type": 2, "data": {"source": 1, "payload": "AAABBBCCC_very_large_blob"}},
        {"type": 3, "data": {"source": 2, "payload": "DDDEEEFFF_another_blob"}}
    ]);
    v
}

// ── 1. list returns results ───────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/session_recordings/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                recording_fixture("rec-1", "user-a"),
                recording_fixture("rec-2", "user-b")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["session-recording", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("rec-1"))
        .stdout(contains("rec-2"));
}

// ── 2. get by id (default: no snapshots on stdout) ───────────────────────────

#[tokio::test]
async fn session_recording_get_default_strips_snapshots() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/session_recordings/rec-snap/",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(recording_with_snapshots_fixture("rec-snap")),
        )
        .mount(&h.server)
        .await;

    let out = h
        .cmd()
        .args(["session-recording", "get", "rec-snap", "--json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "exit status: {:?}\n{stdout}",
        out.status
    );

    // SAFETY CRITICAL: snapshots must NEVER appear on stdout.
    assert!(
        !stdout.contains("AAABBBCCC_very_large_blob"),
        "snapshot blob leaked to stdout!\n{stdout}"
    );
    assert!(
        !stdout.contains("DDDEEEFFF_another_blob"),
        "snapshot blob leaked to stdout!\n{stdout}"
    );

    // ID should be present.
    assert!(
        stdout.contains("rec-snap"),
        "id missing from stdout\n{stdout}"
    );
}

// ── 3. get --with-snapshots --out writes file, not stdout ────────────────────

#[tokio::test]
async fn session_recording_get_with_snapshots_writes_to_file() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/environments/999999/session_recordings/rec-file/",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(recording_with_snapshots_fixture("rec-file")),
        )
        .mount(&h.server)
        .await;

    let out_file = h.config_path.parent().unwrap().join("recording.json");

    let out = h
        .cmd()
        .args([
            "session-recording",
            "get",
            "rec-file",
            "--with-snapshots",
            "--out",
            out_file.to_str().unwrap(),
            "--json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success(), "exit: {:?}\n{stdout}", out.status);

    // Stdout should NOT contain the blob — file should.
    assert!(
        !stdout.contains("AAABBBCCC_very_large_blob"),
        "blob leaked to stdout when --out was used!\n{stdout}"
    );

    // File should exist and contain the snapshot.
    assert!(out_file.exists(), "output file was not created");
    let file_contents = std::fs::read_to_string(&out_file).unwrap();
    assert!(
        file_contents.contains("AAABBBCCC_very_large_blob"),
        "snapshot blob missing from output file"
    );
}

// ── 4. update patches fields ──────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_update_patches_viewed() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/environments/999999/session_recordings/rec-upd/"))
        .respond_with(ResponseTemplate::new(200).set_body_json({
            let mut v = recording_fixture("rec-upd", "user-c");
            v["viewed"] = json!(true);
            v
        }))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "session-recording",
            "update",
            "rec-upd",
            "--viewed",
            "true",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("rec-upd"));
}

// ── 5. delete issues hard DELETE ─────────────────────────────────────────────

#[tokio::test]
async fn session_recording_delete_hard_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path("/api/environments/999999/session_recordings/rec-del/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "session-recording", "delete", "rec-del", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("rec-del"));
}

// ── 6. delete without --yes blocked in non-tty ───────────────────────────────

#[tokio::test]
async fn session_recording_delete_without_yes_blocked() {
    use assert_cmd::Command;
    use tempfile::TempDir;

    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19993"
api_key = "phx_testkey"
project_id = "1"
env_id = "1"
"#,
    )
    .unwrap();

    let out = Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["session-recording", "delete", "rec-x"])
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "delete without --yes should not succeed in non-TTY"
    );
}

// ── 7. list with --person-id filter ──────────────────────────────────────────

#[tokio::test]
async fn session_recording_list_with_person_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/environments/999999/session_recordings/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [recording_fixture("rec-3", "user-a")]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "session-recording",
            "list",
            "--person-id",
            "some-uuid-123",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("rec-3"));
}
