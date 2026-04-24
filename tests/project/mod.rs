//! Integration tests for `bosshogg project` subcommands.

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_cfg(tmp: &TempDir, server_uri: &str) -> std::path::PathBuf {
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            r#"current_context = "t"
[contexts.t]
host = "{}"
api_key = "phx_testkey"
project_id = "101"
org_id = "org-uuid-1"
"#,
            server_uri
        ),
    )
    .unwrap();
    cfg
}

fn project_json(id: i64, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "organization": "org-uuid-1",
        "api_token": "phc_testtoken",
        "timezone": "UTC",
        "created_at": "2026-01-01T00:00:00Z",
        "session_recording_opt_in": false
    })
}

#[tokio::test]
async fn project_list_returns_projects() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/organizations/org-uuid-1/projects/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "results": [
                project_json(101, "Production"),
                project_json(102, "Staging")
            ]
        })))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["project", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Production"))
        .stdout(contains("Staging"));
}

#[tokio::test]
async fn project_get_by_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/101/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_json(101, "Production")))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["project", "get", "101", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":101"))
        .stdout(contains("\"name\":\"Production\""));
}

#[tokio::test]
async fn project_get_by_name_uses_list_filter() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/organizations/org-uuid-1/projects/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "results": [
                project_json(101, "Production"),
                project_json(102, "Staging")
            ]
        })))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["project", "get", "Staging", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":102"))
        .stdout(contains("\"name\":\"Staging\""));
}

#[tokio::test]
async fn project_current_reads_from_config_and_fetches() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/projects/101/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(project_json(101, "Production")))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["project", "current", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":101"))
        .stdout(contains("\"name\":\"Production\""));
}

#[tokio::test]
async fn project_switch_updates_config() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "https://us.posthog.com"
api_key = "phx_testkey"
project_id = "101"
"#,
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .args(["project", "switch", "999", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"project_id\":\"999\""));

    let contents = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        contents.contains("999"),
        "config should contain new project_id: {contents}"
    );
}

#[tokio::test]
async fn project_reset_token_with_yes_flag() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/projects/101/reset_token/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"api_token": "phc_newtoken123"})),
        )
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["--yes", "project", "reset-token", "101", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("phc_newtoken123"));
}
