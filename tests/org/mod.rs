//! Integration tests for `bosshogg org` subcommands.

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
project_id = "999999"
org_id = "org-uuid-1"
"#,
            server_uri
        ),
    )
    .unwrap();
    cfg
}

#[tokio::test]
async fn org_list_returns_orgs() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/organizations/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [
                {"id": "org-uuid-1", "name": "Acme Corp", "slug": "acme"},
                {"id": "org-uuid-2", "name": "Beta Inc", "slug": "beta"}
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
        .args(["org", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Acme Corp"))
        .stdout(contains("Beta Inc"));
}

#[tokio::test]
async fn org_get_by_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/organizations/org-uuid-1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "org-uuid-1",
            "name": "Acme Corp",
            "slug": "acme",
            "membership_level": 15,
            "created_at": "2026-01-01T00:00:00Z"
        })))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["org", "get", "org-uuid-1", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"org-uuid-1\""))
        .stdout(contains("\"name\":\"Acme Corp\""));
}

#[tokio::test]
async fn org_current_reads_from_config_and_fetches() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/organizations/org-uuid-1/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "org-uuid-1",
            "name": "Acme Corp",
            "slug": "acme"
        })))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["org", "current", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"org-uuid-1\""))
        .stdout(contains("\"name\":\"Acme Corp\""));
}

#[tokio::test]
async fn org_switch_updates_config() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "https://us.posthog.com"
api_key = "phx_testkey"
org_id = "old-org-id"
"#,
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .args(["org", "switch", "new-org-uuid", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"org_id\":\"new-org-uuid\""));

    // Verify the config file was actually updated
    let contents = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        contents.contains("new-org-uuid"),
        "config should contain new org_id: {contents}"
    );
}

#[tokio::test]
async fn org_current_errors_when_no_org_id_configured() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    // No org_id in config
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "https://us.posthog.com"
api_key = "phx_testkey"
"#,
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .args(["org", "current"])
        .assert()
        .failure();
}
