use assert_cmd::Command;
use predicates::str::contains;
use tempfile::TempDir;

#[test]
fn configure_non_interactive_errors() {
    let tmp = TempDir::new().unwrap();
    Command::cargo_bin("bosshogg")
        .unwrap()
        .current_dir(tmp.path())
        .args(["configure", "--non-interactive", "--json"])
        .assert()
        .failure()
        .stderr(contains("\"code\""));
}
