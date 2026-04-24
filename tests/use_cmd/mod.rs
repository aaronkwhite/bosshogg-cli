use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn use_switches_current_context() {
    let tmp = TempDir::new().unwrap();
    let cfg = tmp.path().join("config.toml");
    std::fs::write(
        &cfg,
        r#"current_context = "a"
[contexts.a]
host = "https://us.posthog.com"
[contexts.b]
host = "https://eu.posthog.com"
"#,
    )
    .unwrap();

    Command::cargo_bin("bosshogg")
        .unwrap()
        .env("BOSSHOGG_CONFIG", &cfg)
        .args(["use", "b", "--json"])
        .assert()
        .success()
        .stdout(contains("\"current\":\"b\""));
}
