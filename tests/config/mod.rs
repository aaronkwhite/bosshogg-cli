use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn set_context_project_token_flag_persists() {
    let tmp = TempDir::new().unwrap();
    let cfg_path = tmp.path().join("config.toml");

    // First: create a context with --project-token.
    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg_path)
        .args([
            "config",
            "set-context",
            "myctx",
            "--host",
            "https://us.posthog.com",
            "--project-token",
            "phc_testprojecttoken12345",
        ])
        .assert()
        .success();

    // Verify it round-trips via get-contexts --json.
    // (The ContextListItem doesn't expose project_token directly, so we read
    // the TOML to verify it was stored.)
    let raw = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        raw.contains("phc_testprojecttoken12345"),
        "project_token should be in saved config: {raw}"
    );

    // Second call without --project-token must preserve the existing token.
    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg_path)
        .args([
            "config",
            "set-context",
            "myctx",
            "--host",
            "https://us.posthog.com",
        ])
        .assert()
        .success();

    let raw2 = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        raw2.contains("phc_testprojecttoken12345"),
        "project_token must be preserved when not re-specified: {raw2}"
    );
}

#[test]
fn get_contexts_lists_entries_from_config() {
    let tmp = TempDir::new().unwrap();
    let cfg_path = tmp.path().join("config.toml");
    std::fs::write(
        &cfg_path,
        r#"current_context = "staging"
[contexts.staging]
host = "https://us.posthog.com"
api_key = "phx_test"
project_id = "999999"
env_id = "999999"

[contexts.prod]
host = "https://us.posthog.com"
api_key = "phx_prod"
project_id = "999"
env_id = "999"
"#,
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg_path)
        .args(["config", "get-contexts", "--json"])
        .assert()
        .success()
        .stdout(contains("\"staging\""))
        .stdout(contains("\"prod\""))
        .stdout(contains("\"current\":\"staging\""));
}
