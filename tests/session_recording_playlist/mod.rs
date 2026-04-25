//! Integration tests for `bosshogg session-recording-playlist` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, ResponseTemplate};

// ── shared fixture ────────────────────────────────────────────────────────────

fn playlist_fixture(id: i64, short_id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "short_id": short_id,
        "name": name,
        "derived_name": null,
        "description": "A test playlist",
        "pinned": false,
        "deleted": false,
        "type": "collection",
        "filters": null,
        "created_at": "2026-01-01T00:00:00Z",
        "created_by": {"id": 1, "email": "test@example.com"},
        "last_modified_at": "2026-01-02T00:00:00Z",
        "last_modified_by": null,
        "recordings_counts": {},
        "is_synthetic": false
    })
}

// ── 1. list ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_list_returns_results() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/session_recording_playlists/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                playlist_fixture(1, "AbCd1234", "Checkout Flow"),
                playlist_fixture(2, "EfGh5678", "Signup Funnel"),
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["session-recording-playlist", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Checkout Flow"))
        .stdout(contains("Signup Funnel"));
}

// ── 2. get ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_get_by_short_id() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/projects/999999/session_recording_playlists/AbCd1234/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(playlist_fixture(
            1,
            "AbCd1234",
            "Checkout Flow",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["session-recording-playlist", "get", "AbCd1234", "--json"])
        .assert()
        .success()
        .stdout(contains("\"short_id\":\"AbCd1234\""))
        .stdout(contains("Checkout Flow"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_create() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/session_recording_playlists/"))
        .and(body_partial_json(json!({"name": "New Playlist"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(playlist_fixture(
            99,
            "NwPl0001",
            "New Playlist",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "session-recording-playlist",
            "create",
            "--name",
            "New Playlist",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("NwPl0001"));
}

// ── 4. update ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_update_name() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path(
            "/api/projects/999999/session_recording_playlists/AbCd1234/",
        ))
        .and(body_partial_json(json!({"name": "Renamed Playlist"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(playlist_fixture(
            1,
            "AbCd1234",
            "Renamed Playlist",
        )))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "session-recording-playlist",
            "update",
            "AbCd1234",
            "--name",
            "Renamed Playlist",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"short_id\":\"AbCd1234\""));
}

// ── 5. delete ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path(
            "/api/projects/999999/session_recording_playlists/AbCd1234/",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "session-recording-playlist",
            "delete",
            "AbCd1234",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("AbCd1234"));
}

// ── 6. recordings ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_recordings() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/projects/999999/session_recording_playlists/AbCd1234/recordings/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"id": "session-1"}, {"id": "session-2"}]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "session-recording-playlist",
            "recordings",
            "AbCd1234",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("session-1"));
}

// ── 7. add-recording ──────────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_add_recording() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/projects/999999/session_recording_playlists/AbCd1234/recordings/session-abc/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "session-recording-playlist",
            "add-recording",
            "AbCd1234",
            "--session-id",
            "session-abc",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"add-recording\""))
        .stdout(contains("session-abc"));
}

// ── 8. remove-recording ───────────────────────────────────────────────────────

#[tokio::test]
async fn session_recording_playlist_remove_recording() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path(
            "/api/projects/999999/session_recording_playlists/AbCd1234/recordings/session-abc/",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "session-recording-playlist",
            "remove-recording",
            "AbCd1234",
            "--session-id",
            "session-abc",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"remove-recording\""))
        .stdout(contains("session-abc"));
}
