//! Integration tests for `bosshogg dataset-item` subcommands.

use crate::common::TestHarness;
use predicates::str::contains;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn item_fixture(id: &str, dataset_id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "dataset": dataset_id,
        "input": {"question": "What is 2+2?"},
        "output": {"answer": "4"},
        "metadata": null,
        "ref_trace_id": null,
        "created_at": "2026-04-01T00:00:00Z",
        "updated_at": null
    })
}

// ── dataset-item list ─────────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_item_list() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/dataset_items/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                item_fixture("di-1", "ds-abc"),
                item_fixture("di-2", "ds-abc")
            ]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dataset-item", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("di-1"))
        .stdout(contains("di-2"));
}

// ── dataset-item list with --dataset filter ───────────────────────────────────

#[tokio::test]
async fn dataset_item_list_with_filter() {
    let h = TestHarness::new().await;
    // wiremock path matching ignores query params, so we match on path only
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/dataset_items/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [item_fixture("di-filtered", "ds-xyz")]
        })))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dataset-item", "list", "--dataset", "ds-xyz", "--json"])
        .assert()
        .success()
        .stdout(contains("di-filtered"));
}

// ── dataset-item get ──────────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_item_get() {
    let h = TestHarness::new().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/999999/dataset_items/di-abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(item_fixture("di-abc", "ds-1")))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["dataset-item", "get", "di-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"di-abc\""))
        .stdout(contains("\"dataset\":\"ds-1\""));
}

// ── dataset-item create ───────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_item_create() {
    let h = TestHarness::new().await;
    Mock::given(method("POST"))
        .and(path("/api/projects/999999/dataset_items/"))
        .respond_with(ResponseTemplate::new(201).set_body_json(item_fixture("di-new", "ds-target")))
        .mount(&h.server)
        .await;

    let inputs_file = h.config_path.parent().unwrap().join("inputs.json");
    let outputs_file = h.config_path.parent().unwrap().join("outputs.json");
    std::fs::write(&inputs_file, r#"{"question": "2+2?"}"#).unwrap();
    std::fs::write(&outputs_file, r#"{"answer": "4"}"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "dataset-item",
            "create",
            "--dataset",
            "ds-target",
            "--inputs-file",
            inputs_file.to_str().unwrap(),
            "--outputs-file",
            outputs_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("di-new"))
        .stdout(contains("ds-target"));
}

// ── dataset-item update ───────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_item_update() {
    let h = TestHarness::new().await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/999999/dataset_items/di-upd/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(item_fixture("di-upd", "ds-1")))
        .mount(&h.server)
        .await;

    let inputs_file = h.config_path.parent().unwrap().join("upd_inputs.json");
    std::fs::write(&inputs_file, r#"{"question": "updated?"}"#).unwrap();

    h.cmd()
        .args([
            "--yes",
            "dataset-item",
            "update",
            "di-upd",
            "--inputs-file",
            inputs_file.to_str().unwrap(),
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("di-upd"));
}

// ── dataset-item delete ───────────────────────────────────────────────────────

#[tokio::test]
async fn dataset_item_delete() {
    let h = TestHarness::new().await;
    Mock::given(method("DELETE"))
        .and(path("/api/projects/999999/dataset_items/di-del/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&h.server)
        .await;

    h.cmd()
        .args(["--yes", "dataset-item", "delete", "di-del", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("di-del"));
}
