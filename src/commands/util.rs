//! Shared helpers for command modules.
//!
//! Extraction rule: a helper belongs here only when it's identical
//! (or identical modulo resource name) across multiple command files.
//! "Similar-looking" does not qualify.

use std::path::Path;

use serde_json::Value;

use crate::client::Client;
use crate::{BosshoggError, Result};

/// Read a JSON file from disk and parse it into `serde_json::Value`.
/// File I/O errors map to `BosshoggError::Config`; parse errors to
/// `BosshoggError::BadRequest`.
pub async fn read_json_file(path: &Path) -> Result<Value> {
    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| BosshoggError::Config(format!("read {}: {e}", path.display())))?;
    serde_json::from_str(&raw).map_err(|e| BosshoggError::BadRequest(format!("json: {e}")))
}

/// Read a UTF-8 text file from disk. File I/O errors map to
/// `BosshoggError::Config`.
pub async fn read_text_file(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path)
        .await
        .map_err(|e| BosshoggError::Config(format!("read {}: {e}", path.display())))
}

/// Return the configured `env_id`, or a `BosshoggError::Config` with a
/// fix-it-yourself hint. Every HogQL / env-scoped endpoint needs this.
pub fn env_id_required(client: &Client) -> Result<&str> {
    client.env_id().ok_or_else(|| {
        BosshoggError::Config(
            "no env_id configured; run `bosshogg configure` or set POSTHOG_CLI_ENV_ID".into(),
        )
    })
}

/// Destructive-action gate. Passes through immediately when `yes` is
/// true. Otherwise prompts interactively via `output::interactive::confirm`
/// and returns a `BadRequest("aborted by user")` on decline.
pub fn gate_destructive(yes: bool, prompt: &str) -> Result<()> {
    if yes {
        return Ok(());
    }
    let proceed = crate::output::interactive::confirm(prompt, false)?;
    if !proceed {
        return Err(BosshoggError::BadRequest("aborted by user".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn read_json_file_parses_valid_json() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("x.json");
        std::fs::write(&p, r#"{"a":1,"b":["c"]}"#).unwrap();
        let v = read_json_file(&p).await.unwrap();
        assert_eq!(v["a"], 1);
        assert_eq!(v["b"][0], "c");
    }

    #[tokio::test]
    async fn read_json_file_missing_path_is_config_error() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("does-not-exist.json");
        let err = read_json_file(&p).await.unwrap_err();
        assert!(matches!(err, BosshoggError::Config(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn read_json_file_invalid_json_is_bad_request() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("bad.json");
        std::fs::write(&p, "{not json").unwrap();
        let err = read_json_file(&p).await.unwrap_err();
        assert!(matches!(err, BosshoggError::BadRequest(_)), "got {err:?}");
    }

    #[tokio::test]
    async fn read_text_file_reads_utf8_contents() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("hello.txt");
        std::fs::write(&p, "hello\nworld\n").unwrap();
        let s = read_text_file(&p).await.unwrap();
        assert_eq!(s, "hello\nworld\n");
    }

    #[tokio::test]
    async fn read_text_file_missing_path_is_config_error() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("nope.txt");
        let err = read_text_file(&p).await.unwrap_err();
        assert!(matches!(err, BosshoggError::Config(_)), "got {err:?}");
    }

    #[test]
    fn gate_destructive_passes_through_when_yes() {
        // With yes=true, gate_destructive returns Ok immediately without
        // invoking the interactive confirm. This is the only case we can
        // unit-test cleanly; interactive paths are covered by integration
        // tests that set BOSSHOGG_NON_INTERACTIVE or pass --yes.
        assert!(gate_destructive(true, "delete everything?").is_ok());
    }
}
