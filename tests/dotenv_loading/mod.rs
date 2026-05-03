//! Tests for automatic .env / .env.local loading from the current working directory.
//!
//! These tests verify that running `bosshogg` from a directory that contains a
//! `.env` or `.env.local` file causes those values to be picked up as if they
//! were real environment variables — with the standard priority:
//!   process env > .env.local > .env > config.toml

use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

fn bare_config(tmp: &TempDir) -> std::path::PathBuf {
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "https://us.posthog.com"
"#,
    )
    .unwrap();
    cfg
}

#[test]
fn dotenv_provides_token_when_no_other_auth() {
    let tmp = TempDir::new().unwrap();
    let cfg = bare_config(&tmp);

    std::fs::write(
        tmp.path().join(".env"),
        "POSTHOG_CLI_TOKEN=phx_from_dotenv\n",
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .current_dir(tmp.path())
        .env("BOSSHOGG_CONFIG", &cfg)
        .env_remove("POSTHOG_CLI_TOKEN")
        .env_remove("POSTHOG_CLI_API_KEY")
        .env_remove("POSTHOG_API_KEY")
        .args(["auth", "token"])
        .assert()
        .success()
        .stdout(contains("phx_from_dotenv"));
}

#[test]
fn dotenv_local_overrides_dotenv() {
    let tmp = TempDir::new().unwrap();
    let cfg = bare_config(&tmp);

    std::fs::write(
        tmp.path().join(".env"),
        "POSTHOG_CLI_TOKEN=phx_from_dotenv\n",
    )
    .unwrap();
    std::fs::write(
        tmp.path().join(".env.local"),
        "POSTHOG_CLI_TOKEN=phx_from_dotenv_local\n",
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .current_dir(tmp.path())
        .env("BOSSHOGG_CONFIG", &cfg)
        .env_remove("POSTHOG_CLI_TOKEN")
        .env_remove("POSTHOG_CLI_API_KEY")
        .env_remove("POSTHOG_API_KEY")
        .args(["auth", "token"])
        .assert()
        .success()
        .stdout(contains("phx_from_dotenv_local"));
}

#[test]
fn process_env_beats_dotenv() {
    let tmp = TempDir::new().unwrap();
    let cfg = bare_config(&tmp);

    std::fs::write(
        tmp.path().join(".env"),
        "POSTHOG_CLI_TOKEN=phx_from_dotenv\n",
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .current_dir(tmp.path())
        .env("BOSSHOGG_CONFIG", &cfg)
        .env("POSTHOG_CLI_TOKEN", "phx_from_process")
        .args(["auth", "token"])
        .assert()
        .success()
        .stdout(contains("phx_from_process"));
}

#[test]
fn missing_dotenv_is_silent() {
    // No .env file at all — command should still work if config has a key.
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "t"
[contexts.t]
host = "https://us.posthog.com"
api_key = "phx_from_config"
"#,
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .current_dir(tmp.path())
        .env("BOSSHOGG_CONFIG", &cfg)
        .env_remove("POSTHOG_CLI_TOKEN")
        .env_remove("POSTHOG_CLI_API_KEY")
        .env_remove("POSTHOG_API_KEY")
        .args(["auth", "token"])
        .assert()
        .success()
        .stdout(contains("phx_from_config"));
}
