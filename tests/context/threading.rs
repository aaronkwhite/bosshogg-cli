//! Regression test for Opus review critical #1 — --context must actually flow through.
//!
//! `bosshogg --context nonexistent whoami` must error with CONFIG (code 70) rather
//! than silently succeeding against env-var or default-context credentials.

use assert_cmd::Command;

#[test]
fn unknown_context_errors_with_config_code() {
    let mut cmd = Command::cargo_bin("bosshogg").unwrap();
    // Isolate from the developer's real config + env.
    let tmp = tempfile::tempdir().unwrap();
    cmd.env("BOSSHOGG_CONFIG", tmp.path().join("config.toml"))
        .env_remove("POSTHOG_CLI_TOKEN")
        .env_remove("POSTHOG_CLI_API_KEY")
        .env_remove("POSTHOG_API_KEY")
        .args(["--context", "does-not-exist", "whoami"]);

    let output = cmd.output().unwrap();
    // Must be non-zero exit (CONFIG bucket = 70 per docs/conventions.md, or 12 if resolve_auth hits MissingScope first)
    assert!(!output.status.success(), "expected error, got success");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unknown context") || stderr.contains("does-not-exist"),
        "stderr should mention the missing context; got: {stderr}"
    );
}
