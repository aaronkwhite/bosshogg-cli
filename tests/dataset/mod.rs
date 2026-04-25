//! Integration tests for `bosshogg dataset` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn dataset_fixture(id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "description": "Test dataset",
        "metadata": null,
        "created_at": "2026-04-01T00:00:00Z",
        "updated_at": "2026-04-02T00:00:00Z",
        "deleted": false,
        "team": 999999,
        "created_by": {"id": 1, "email": "admin@example.com"}
    })
}

// ── dataset list ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/datasets/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                dataset_fixture("ds-1", "Training Set"),
                dataset_fixture("ds-2", "Validation Set")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dataset", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("ds-1"))
        .stdout(contains("ds-2"));
}

// ── dataset get ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/datasets/ds-abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(dataset_fixture("ds-abc", "My DS")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dataset", "get", "ds-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"ds-abc\""))
        .stdout(contains("My DS"));
}

// ── dataset create ────────────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_create() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/datasets/"))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(dataset_fixture("ds-new", "New Dataset")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "dataset",
            "create",
            "--name",
            "New Dataset",
            "--description",
            "A fresh dataset",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("ds-new"))
        .stdout(contains("New Dataset"));
}

// ── dataset update ────────────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_update() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/datasets/ds-upd/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(dataset_fixture("ds-upd", "Updated Name")),
        )
        .mount(&h.server)
        .await;

    h.cmd()
        .args([
            "--yes",
            "dataset",
            "update",
            "ds-upd",
            "--name",
            "Updated Name",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("ds-upd"));
}

// ── dataset delete ────────────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/datasets/ds-del/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "dataset", "delete", "ds-del", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("ds-del"));
}

// ── update without flags is rejected ─────────────────────────────────────────

#[tokio::test]
async fn dataset_update_no_flags_rejected() {
    let h = TestHarness::new().await;

    h.cmd()
        .args(["--yes", "dataset", "update", "ds-x"])
        .assert()
        .failure();
}
