use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn auth_token_emits_current_key() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "https://us.posthog.com"
api_key = "phx_abc"
"#,
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .env_remove("POSTHOG_CLI_TOKEN")
        .env_remove("POSTHOG_CLI_API_KEY")
        .env_remove("POSTHOG_API_KEY")
        .args(["auth", "token"])
        .assert()
        .success()
        .stdout(contains("phx_abc"));
}
