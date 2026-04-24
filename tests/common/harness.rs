//! Test harness for BossHogg integration tests.
//!
//! Gives every test a pre-wired MockServer + config.toml + Command::cargo_bin
//! without repeating ~20 LOC of setup. Each harness instance owns its own
//! TempDir and MockServer, so tests in the same binary run in parallel safely.

#![allow(dead_code)] // different tests use different subsets

use std::path::PathBuf;

use assert_cmd::Command;
use tempfile::TempDir;
use wiremock::MockServer;

/// Default fake project/env IDs used across the suite. Chosen so that
/// accidental escape to a real PostHog host produces a 404 rather than
/// mutating anything real.
pub const TEST_PROJECT_ID: &str = "999999";
pub const TEST_ENV_ID: &str = "999999";

/// Pre-wired fixture for integration tests.
///
/// Owns a MockServer (HTTP, random port), a TempDir holding a config.toml
/// pointing at the server, and yields a `Command` pre-configured with
/// `BOSSHOGG_CONFIG` and `BOSSHOGG_ALLOW_HTTP` env vars.
pub struct TestHarness {
    pub server: MockServer,
    pub config_path: PathBuf,
    _tmp: TempDir, // kept alive for config lifetime
}

impl TestHarness {
    /// Harness with default project/env IDs.
    pub async fn new() -> Self {
        Self::with_project(TEST_PROJECT_ID, TEST_ENV_ID).await
    }

    /// Harness with custom project/env IDs (for tests that exercise id
    /// resolution or multi-context behaviour).
    pub async fn with_project(project_id: &str, env_id: &str) -> Self {
        let server = MockServer::start().await;
        let tmp = TempDir::new().expect("create TempDir");
        let config_path = tmp.path().join("config.toml");

        let config = format!(
            r#"current_context = "t"
[contexts.t]
host = "{host}"
api_key = "phx_testkey"
project_id = "{project_id}"
env_id = "{env_id}"
"#,
            host = server.uri(),
        );
        std::fs::write(&config_path, config).expect("write config.toml");

        Self {
            server,
            config_path,
            _tmp: tmp,
        }
    }

    /// Build a `Command` invocation of `bosshogg` with env vars already set.
    /// Callers add their own `.args([...])`.
    pub fn cmd(&self) -> Command {
        let mut c = Command::cargo_bin("bosshogg").expect("cargo_bin bosshogg");
        c.env("BOSSHOGG_CONFIG", &self.config_path);
        c.env("BOSSHOGG_ALLOW_HTTP", "1");
        c
    }
}
