use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn configure_non_interactive_errors() {
    Command::cargo_bin("bosshogg")
        .unwrap()
        .args(["configure", "--non-interactive", "--json"])
        .assert()
        .failure()
        .stderr(contains("\"code\""));
}
