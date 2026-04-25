//! Anonymous self-tracking telemetry.
//!
//! Each command invocation appends one event to a local JSONL queue
//! (`<data_dir>/analytics_queue.jsonl`); a fire-and-forget background task
//! flushes the queue to PostHog `/batch/` on exit with a 3-second timeout.
//! Failures keep the queue intact for the next run.
//!
//! Captured properties are coarse and non-identifying: the command name
//! (e.g. `flag.list`), which top-level flags were set, success bool,
//! `duration_ms`, `version`, `os`, `arch`, the active context's `region`
//! (`us` / `eu` / `self-hosted`), and on failure the error's
//! `SCREAMING_SNAKE` `error_code` + numeric `exit_code`. No identifiers,
//! no flag values, no stdout/stderr, no auth material.
//!
//! Opt-out: `DO_NOT_TRACK=1`, `bosshogg config analytics off`, or build
//! with `--features test-harness` (auto-disables for the test suite).
//!
//! The PostHog token below is a write-only public project key
//! (`phc_*`) — same project as the user's other CLIs. It cannot read
//! data; embedding it in source is the standard PostHog pattern.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

const POSTHOG_TOKEN: &str = "phc_3DIgL4ES4ukoFmH4hgg3jR0e6O52PiQIfzfsVEjJu9u";
const POSTHOG_BATCH_URL: &str = "https://app.posthog.com/batch/";

/// Whether telemetry should fire for this invocation. Order:
/// 1. `cfg!(feature = "test-harness")` → disabled (test suite never emits).
/// 2. `DO_NOT_TRACK=1` → disabled.
/// 3. Config `analytics_enabled = false` → disabled.
/// 4. Otherwise → enabled.
pub fn is_enabled() -> bool {
    if cfg!(feature = "test-harness") {
        return false;
    }
    crate::config::is_analytics_enabled()
}

/// Captured per-invocation. Built in `main.rs` after dispatch returns.
pub struct Event {
    pub command: String,
    pub flags: Vec<String>,
    pub success: bool,
    pub duration_ms: u64,
    pub region: Option<String>,
    /// `SCREAMING_SNAKE` from `BosshoggError::error_code()` on failure.
    pub error_code: Option<String>,
    /// Numeric exit code from `BosshoggError::exit_code()` on failure.
    pub exit_code: Option<i32>,
}

/// Append the event to the queue file. First-run prints a one-time stderr
/// notice. Returns silently on any failure (telemetry never blocks the user).
pub fn track(event: &Event) {
    if !is_enabled() {
        return;
    }
    let Some(dir) = crate::config::data_dir() else {
        return;
    };
    if track_to_dir(&dir, event) {
        eprintln!(
            "bosshogg: anonymous usage stats enabled. Disable: bosshogg config analytics off (or set DO_NOT_TRACK=1)"
        );
    }
}

/// Testable inner: write to `<dir>/analytics_queue.jsonl`. Returns `true`
/// when this invocation also created the install_id file (first run).
fn track_to_dir(dir: &Path, event: &Event) -> bool {
    let Some((install_id, first_run)) = get_or_create_install_id_in(dir) else {
        return false;
    };

    let mut properties = serde_json::json!({
        "command": event.command,
        "flags": event.flags,
        "success": event.success,
        "duration_ms": event.duration_ms,
        "version": env!("CARGO_PKG_VERSION"),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
    });
    if let Some(map) = properties.as_object_mut() {
        if let Some(region) = &event.region {
            map.insert("region".into(), serde_json::json!(region));
        }
        if let Some(code) = &event.error_code {
            map.insert("error_code".into(), serde_json::json!(code));
        }
        if let Some(code) = event.exit_code {
            map.insert("exit_code".into(), serde_json::json!(code));
        }
    }

    let payload = serde_json::json!({
        "event": "command_executed",
        "distinct_id": install_id,
        "properties": properties,
    });

    let queue_path = dir.join("analytics_queue.jsonl");
    let Ok(line) = serde_json::to_string(&payload) else {
        return false;
    };

    let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&queue_path)
    else {
        return false;
    };
    let _ = writeln!(file, "{line}");
    first_run
}

/// Flush the on-disk queue to PostHog with a 5-second per-request timeout.
pub async fn flush() {
    let Some(dir) = crate::config::data_dir() else {
        return;
    };
    flush_dir(&dir, POSTHOG_BATCH_URL).await;
}

async fn flush_dir(dir: &Path, url: &str) {
    let queue_path = dir.join("analytics_queue.jsonl");

    let contents = match fs::read_to_string(&queue_path) {
        Ok(c) if !c.trim().is_empty() => c,
        _ => return,
    };

    let events: Vec<serde_json::Value> = contents
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();

    if events.is_empty() {
        return;
    }

    let batch_payload = serde_json::json!({
        "api_key": POSTHOG_TOKEN,
        "batch": events,
    });

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => return,
    };

    let response = client.post(url).json(&batch_payload).send().await;

    if let Ok(resp) = response
        && resp.status().is_success()
    {
        let _ = fs::write(&queue_path, "");
    }
}

fn get_or_create_install_id_in(dir: &Path) -> Option<(String, bool)> {
    let path = dir.join("analytics_id");
    if let Ok(id) = fs::read_to_string(&path) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return Some((id, false));
        }
    }
    let id = uuid::Uuid::new_v4().to_string();
    fs::create_dir_all(dir).ok()?;
    fs::write(&path, &id).ok()?;
    Some((id, true))
}

/// Stable telemetry name for a top-level command. Uses dots for nested
/// resources so dashboards can split by top-level (`flag`, `experiment`,
/// …) without parsing flag strings.
pub fn command_name(cmd: &crate::cli::Commands) -> &'static str {
    use crate::cli::Commands;
    match cmd {
        Commands::Configure(_) => "configure",
        Commands::Whoami => "whoami",
        Commands::Doctor(_) => "doctor",
        Commands::Schema(_) => "schema",
        Commands::Auth(_) => "auth",
        Commands::Config(_) => "config",
        Commands::Query(_) => "query",
        Commands::Flag(_) => "flag",
        Commands::Insight(_) => "insight",
        Commands::Dashboard(_) => "dashboard",
        Commands::Cohort(_) => "cohort",
        Commands::Org(_) => "org",
        Commands::Project(_) => "project",
        Commands::Person(_) => "person",
        Commands::Group(_) => "group",
        Commands::Event(_) => "event",
        Commands::Action(_) => "action",
        Commands::Annotation(_) => "annotation",
        Commands::EventDefinition(_) => "event-definition",
        Commands::PropertyDefinition(_) => "property-definition",
        Commands::Endpoint(_) => "endpoint",
        Commands::Experiment(_) => "experiment",
        Commands::Survey(_) => "survey",
        Commands::EarlyAccess(_) => "early-access",
        Commands::HogFunction(_) => "hog-function",
        Commands::BatchExport(_) => "batch-export",
        Commands::SessionRecording(_) => "session-recording",
        Commands::ErrorTracking(_) => "error-tracking",
        Commands::Role(_) => "role",
        Commands::Capture(_) => "capture",
        Commands::Alert(_) => "alert",
        Commands::DashboardTemplate(_) => "dashboard-template",
        Commands::SessionRecordingPlaylist(_) => "session-recording-playlist",
        Commands::InsightVariable(_) => "insight-variable",
        Commands::Dataset(_) => "dataset",
        Commands::DatasetItem(_) => "dataset-item",
        Commands::Evaluation(_) => "evaluation",
        Commands::LlmAnalytics(_) => "llm-analytics",
        Commands::Use(_) => "use",
        Commands::Completion(_) => "completion",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "bosshogg-analytics-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_event() -> Event {
        Event {
            command: "flag.list".into(),
            flags: vec!["--json".into()],
            success: true,
            duration_ms: 42,
            region: Some("us".into()),
            error_code: None,
            exit_code: None,
        }
    }

    #[test]
    fn install_id_created_on_first_run() {
        let dir = fresh_dir("id1");
        let (id, first) = get_or_create_install_id_in(&dir).unwrap();
        assert!(first);
        assert_eq!(id.len(), 36);
        let stored = fs::read_to_string(dir.join("analytics_id")).unwrap();
        assert_eq!(stored, id);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn install_id_stable_across_runs() {
        let dir = fresh_dir("id2");
        let (id1, first) = get_or_create_install_id_in(&dir).unwrap();
        assert!(first);
        let (id2, second) = get_or_create_install_id_in(&dir).unwrap();
        assert!(!second);
        assert_eq!(id1, id2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn track_writes_queue_with_all_properties() {
        let dir = fresh_dir("track");
        let event = Event {
            command: "experiment.archive".into(),
            flags: vec!["--json".into(), "--yes".into()],
            success: false,
            duration_ms: 250,
            region: Some("eu".into()),
            error_code: Some("NOT_FOUND".into()),
            exit_code: Some(20),
        };
        let first = track_to_dir(&dir, &event);
        assert!(first);

        let queue = fs::read_to_string(dir.join("analytics_queue.jsonl")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(queue.trim()).unwrap();
        assert_eq!(parsed["event"], "command_executed");
        let p = &parsed["properties"];
        assert_eq!(p["command"], "experiment.archive");
        assert_eq!(p["flags"][0], "--json");
        assert_eq!(p["flags"][1], "--yes");
        assert_eq!(p["success"], false);
        assert_eq!(p["duration_ms"], 250);
        assert_eq!(p["region"], "eu");
        assert_eq!(p["error_code"], "NOT_FOUND");
        assert_eq!(p["exit_code"], 20);
        assert!(p["version"].is_string());
        assert!(p["os"].is_string());
        assert!(p["arch"].is_string());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn track_omits_optional_properties_when_unset() {
        let dir = fresh_dir("omit");
        let mut event = sample_event();
        event.region = None;
        track_to_dir(&dir, &event);
        let queue = fs::read_to_string(dir.join("analytics_queue.jsonl")).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(queue.trim()).unwrap();
        assert!(parsed["properties"].get("region").is_none());
        assert!(parsed["properties"].get("error_code").is_none());
        assert!(parsed["properties"].get("exit_code").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn track_appends_with_stable_distinct_id() {
        let dir = fresh_dir("append");
        for _ in 0..3 {
            track_to_dir(&dir, &sample_event());
        }
        let queue = fs::read_to_string(dir.join("analytics_queue.jsonl")).unwrap();
        let lines: Vec<&str> = queue.trim().lines().collect();
        assert_eq!(lines.len(), 3);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let third: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(first["distinct_id"], third["distinct_id"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn flush_clears_queue_on_success() {
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(matchers::path("/batch/"))
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        let dir = fresh_dir("flush-ok");
        track_to_dir(&dir, &sample_event());
        track_to_dir(&dir, &sample_event());
        let queue_path = dir.join("analytics_queue.jsonl");
        assert_eq!(fs::read_to_string(&queue_path).unwrap().lines().count(), 2);

        let url = format!("{}/batch/", server.uri());
        flush_dir(&dir, &url).await;

        let after = fs::read_to_string(&queue_path).unwrap();
        assert!(after.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn flush_retains_queue_on_5xx() {
        use wiremock::{Mock, MockServer, ResponseTemplate, matchers};

        let server = MockServer::start().await;
        Mock::given(matchers::method("POST"))
            .and(matchers::path("/batch/"))
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&server)
            .await;

        let dir = fresh_dir("flush-5xx");
        track_to_dir(&dir, &sample_event());
        let queue_path = dir.join("analytics_queue.jsonl");
        let before = fs::read_to_string(&queue_path).unwrap();

        let url = format!("{}/batch/", server.uri());
        flush_dir(&dir, &url).await;

        let after = fs::read_to_string(&queue_path).unwrap();
        assert_eq!(before, after);
        let _ = fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn flush_noop_on_empty_queue() {
        let dir = fresh_dir("flush-empty");
        flush_dir(&dir, "http://127.0.0.1:1/batch/").await;
        assert!(!dir.join("analytics_queue.jsonl").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_enabled_blocked_under_test_harness_feature() {
        // The test suite is built with `--features test-harness`, so
        // is_enabled() must always return false here regardless of env.
        assert!(!is_enabled());
    }
}
