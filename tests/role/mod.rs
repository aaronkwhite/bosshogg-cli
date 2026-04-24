//! Integration tests for `bosshogg role` subcommands.

use assert_cmd::Command;
use predicates::str::contains;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{body_partial_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

// ── fixture helpers ───────────────────────────────────────────────────────────

fn role_fixture(id: &str, name: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": name,
        "feature_flags_access_level": 37,
        "created_at": "2026-01-01T00:00:00Z",
        "created_by": {"email": "admin@example.com"},
        "members": [],
        "associated_flags": []
    })
}

fn membership_fixture(mid: &str, user_email: &str) -> serde_json::Value {
    json!({
        "id": mid,
        "user": {"id": "user-1", "email": user_email},
        "joined_at": "2026-04-01T00:00:00Z",
        "updated_at": "2026-04-01T00:00:00Z"
    })
}

fn write_cfg(tmp: &TempDir, server_uri: &str) -> std::path::PathBuf {
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        format!(
            r#"current_context = "t"
[contexts.t]
host = "{server_uri}"
api_key = "phx_testkey"
project_id = "999999"
env_id = "999999"
org_id = "my-org-uuid"
"#
        ),
    )
    .unwrap();
    cfg
}

// ── 1. list ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn role_list_returns_results() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/organizations/my-org-uuid/roles/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 2,
            "next": null,
            "previous": null,
            "results": [
                role_fixture("role-1", "Engineers"),
                role_fixture("role-2", "Admins")
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
        .args(["role", "list", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":2"))
        .stdout(contains("Engineers"))
        .stdout(contains("Admins"));
}

// ── 2. get ────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn role_get_by_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/organizations/my-org-uuid/roles/role-abc/"))
        .respond_with(ResponseTemplate::new(200).set_body_json(role_fixture("role-abc", "Devs")))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["role", "get", "role-abc", "--json"])
        .assert()
        .success()
        .stdout(contains("\"id\":\"role-abc\""))
        .stdout(contains("Devs"));
}

// ── 3. create ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn role_create_posts_name() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/organizations/my-org-uuid/roles/"))
        .and(body_partial_json(json!({"name": "Viewers"})))
        .respond_with(ResponseTemplate::new(201).set_body_json(role_fixture("role-new", "Viewers")))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["role", "create", "--name", "Viewers", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"create\""))
        .stdout(contains("Viewers"));
}

// ── 4. update patches name ────────────────────────────────────────────────────

#[tokio::test]
async fn role_update_patches_name() {
    let server = MockServer::start().await;
    Mock::given(method("PATCH"))
        .and(path("/api/organizations/my-org-uuid/roles/role-upd/"))
        .and(body_partial_json(json!({"name": "Renamed"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(role_fixture("role-upd", "Renamed")))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes", "role", "update", "role-upd", "--name", "Renamed", "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"name\":\"Renamed\""));
}

// ── 5. delete ─────────────────────────────────────────────────────────────────

#[tokio::test]
async fn role_delete_hard_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/api/organizations/my-org-uuid/roles/role-del/"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["--yes", "role", "delete", "role-del", "--json"])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"delete\""))
        .stdout(contains("role-del"));
}

// ── 6. members ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn role_members_returns_list() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/api/organizations/my-org-uuid/roles/role-m/role_memberships/",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "count": 1,
            "next": null,
            "previous": null,
            "results": [membership_fixture("mem-1", "alice@example.com")]
        })))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["role", "members", "role-m", "--json"])
        .assert()
        .success()
        .stdout(contains("\"count\":1"))
        .stdout(contains("mem-1"));
}

// ── 7. add-member ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn role_add_member_posts_user_uuid() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(
            "/api/organizations/my-org-uuid/roles/role-m/role_memberships/",
        ))
        .and(body_partial_json(json!({"user_uuid": "user-uuid-1"})))
        .respond_with(
            ResponseTemplate::new(201)
                .set_body_json(membership_fixture("mem-new", "bob@example.com")),
        )
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "role",
            "add-member",
            "role-m",
            "--user-id",
            "user-uuid-1",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("mem-new"));
}

// ── 8. remove-member ──────────────────────────────────────────────────────────

#[tokio::test]
async fn role_remove_member_deletes_membership() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path(
            "/api/organizations/my-org-uuid/roles/role-r/role_memberships/mem-del/",
        ))
        .respond_with(ResponseTemplate::new(204))
        .mount(&server)
        .await;

    let tmp = TempDir::new().unwrap();
    let cfg = write_cfg(&tmp, &server.uri());

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args([
            "--yes",
            "role",
            "remove-member",
            "role-r",
            "--membership-id",
            "mem-del",
            "--json",
        ])
        .assert()
        .success()
        .stdout(contains("\"ok\":true"))
        .stdout(contains("\"action\":\"remove-member\""))
        .stdout(contains("mem-del"));
}

// ── 9. delete without --yes blocked ──────────────────────────────────────────

#[tokio::test]
async fn role_delete_without_yes_blocked() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19995"
api_key = "phx_testkey"
project_id = "1"
env_id = "1"
org_id = "my-org"
"#,
    )
    .unwrap();

    let out = Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("BOSSHOGG_ALLOW_HTTP", "1")
        .args(["role", "delete", "role-x"])
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "delete without --yes should fail in non-TTY"
    );
}

// ── 10. missing org_id returns error ─────────────────────────────────────────

#[tokio::test]
async fn role_list_without_org_id_fails() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    // No org_id in context
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "http://127.0.0.1:19996"
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
        .args(["role", "list", "--json"])
        .output()
        .unwrap();

    assert!(
        !out.status.success(),
        "role list without org_id should fail"
    );
}
