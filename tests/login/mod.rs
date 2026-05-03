//! Integration tests for `bosshogg login` — browser device-flow auth.
//!
//! These tests stub the PostHog CLI-auth endpoints so the binary never touches
//! a real PostHog instance. The `--no-browser` flag skips opening a browser
//! and instead prints the verification URL, making the flow testable.

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn mount_happy_stubs(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/api/cli-auth/device-code/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "test-device-code-abc",
            "user_code": "BOSS-12345",
            "verification_uri": format!("{}/device", server.uri()),
            "verification_uri_complete": format!("{}/device?code=BOSS-12345", server.uri()),
            "expires_in": 300,
            "interval": 0
        })))
        .mount(server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/cli-auth/poll/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "status": "authorized",
            "personal_api_key": "phx_login_authorized_key",
            "label": "bosshogg-login",
            "project_id": "999999"
        })))
        .mount(server)
        .await;

    let users_me = std::fs::read_to_string("tests/fixtures/users_me.json").unwrap();
    Mock::given(method("GET"))
        .and(path("/api/users/@me/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(users_me))
        .mount(server)
        .await;
}

fn bh_cmd(cfg: &std::path::Path, server_uri: &str) -> Command {
    let mut c = Command::cargo_bin("bosshogg").unwrap();
    c.current_dir(cfg.parent().expect("config parent dir"))
        .env("BOSSHOGG_CONFIG", cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .env_remove("POSTHOG_CLI_TOKEN")
        .env_remove("POSTHOG_CLI_API_KEY")
        .env_remove("POSTHOG_API_KEY")
        .arg("login")
        .arg("--host")
        .arg(server_uri)
        .arg("--no-browser");
    c
}

#[tokio::test]
async fn login_happy_path_json_contains_context() {
    let server = MockServer::start().await;
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "").unwrap();
    mount_happy_stubs(&server).await;

    bh_cmd(&cfg, &server.uri())
        .args(["--context", "test-ctx", "--json"])
        .assert()
        .success()
        .stdout(contains("test-ctx"));

    let saved = std::fs::read_to_string(&cfg).unwrap();
    assert!(
        saved.contains("phx_login_authorized_key"),
        "config should contain the authorized key; got:\n{saved}"
    );
}

#[tokio::test]
async fn login_no_browser_prints_user_code() {
    let server = MockServer::start().await;
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "").unwrap();
    mount_happy_stubs(&server).await;

    // In non-JSON mode the user code must appear somewhere in the output so
    // the user knows what to type on the authorization page.
    bh_cmd(&cfg, &server.uri())
        .assert()
        .success()
        .stdout(contains("BOSS-12345"));
}

#[tokio::test]
async fn login_expired_exits_nonzero() {
    let server = MockServer::start().await;
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "").unwrap();

    Mock::given(method("POST"))
        .and(path("/api/cli-auth/device-code/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "test-device-code-abc",
            "user_code": "BOSS-12345",
            "verification_uri": format!("{}/device", server.uri()),
            "verification_uri_complete": format!("{}/device?code=BOSS-12345", server.uri()),
            "expires_in": 300,
            "interval": 0
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/api/cli-auth/poll/"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "expired_token"
        })))
        .mount(&server)
        .await;

    bh_cmd(&cfg, &server.uri())
        .assert()
        .failure()
        .stderr(contains("expire"));
}

#[tokio::test]
async fn login_unsupported_host_suggests_configure() {
    let server = MockServer::start().await;
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(&cfg, "").unwrap();

    Mock::given(method("POST"))
        .and(path("/api/cli-auth/device-code/"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    bh_cmd(&cfg, &server.uri())
        .assert()
        .failure()
        .stderr(contains("configure"));
}
