//! Integration tests for `bosshogg completion`.

use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn completion_bash_outputs_complete_function() {
    Command::cargo_bin("bosshogg")
        .unwrap()
        .args(["completion", "bash"])
        .assert()
        .success()
        .stdout(contains("complete -F"));
}

#[test]
fn completion_zsh_outputs_compdef() {
    Command::cargo_bin("bosshogg")
        .unwrap()
        .args(["completion", "zsh"])
        .assert()
        .success()
        .stdout(contains("#compdef"));
}

#[test]
fn completion_fish_outputs_complete_command() {
    Command::cargo_bin("bosshogg")
        .unwrap()
        .args(["completion", "fish"])
        .assert()
        .success()
        .stdout(contains("complete"));
}

#[test]
fn completion_invalid_shell_exits_nonzero() {
    Command::cargo_bin("bosshogg")
        .unwrap()
        .args(["completion", "bogusshell"])
        .assert()
        .failure();
}
