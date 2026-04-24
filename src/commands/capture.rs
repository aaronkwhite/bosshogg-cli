// src/commands/capture.rs
//! `bosshogg capture` — event / batch / identify.
//!
//! IMPORTANT: Capture uses the public PostHog ingest API, NOT the personal API.
//! Authentication is `api_key` in the request body (phc_ project token), not
//! an Authorization header.
//!
//! The project_token (phc_) is read from the active context.
//! All subcommands are gated on `yes || confirm` because they post real events.

use std::path::PathBuf;
use std::time::Duration;

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use reqwest::header::{HeaderMap, HeaderValue, USER_AGENT};

use crate::commands::util::{gate_destructive, read_json_file};
use crate::config;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed response ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct CaptureResponse {
    pub status: i32,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct CaptureArgs {
    #[command(subcommand)]
    pub command: CaptureCommand,
}

#[derive(Subcommand, Debug)]
pub enum CaptureCommand {
    /// Send a single event to PostHog (uses project token, not personal key).
    ///
    /// WARNING: this posts real events to your PostHog project.
    Event {
        /// Event name.
        #[arg(long)]
        event: String,
        /// Distinct ID of the user/actor.
        #[arg(long)]
        distinct_id: String,
        /// Optional path to a JSON file with event properties.
        #[arg(long)]
        properties_file: Option<PathBuf>,
    },
    /// Send a batch of events from a JSONL file (one JSON object per line).
    ///
    /// WARNING: this posts real events to your PostHog project.
    Batch {
        /// Path to a JSONL file where each line is an event object.
        #[arg(long)]
        file: PathBuf,
    },
    /// Send an identify call to set person properties.
    ///
    /// WARNING: this posts real events to your PostHog project.
    Identify {
        /// Distinct ID to identify.
        #[arg(long)]
        distinct_id: String,
        /// Path to a JSON file with person properties.
        #[arg(long)]
        properties_file: PathBuf,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(
    args: CaptureArgs,
    json_mode: bool,
    debug: bool,
    context: Option<&str>,
    yes: bool,
) -> Result<()> {
    // Load config to get project_token + host from the active context.
    let cfg = config::load()?;
    let ctx_name: &str = if let Some(c) = context {
        c
    } else {
        cfg.current_context
            .as_deref()
            .ok_or_else(|| BosshoggError::Config("no active context".into()))?
    };

    let ctx = cfg
        .contexts
        .get(ctx_name)
        .ok_or_else(|| BosshoggError::Config(format!("context `{ctx_name}` not found")))?;

    let project_token = ctx
        .project_token
        .clone()
        .ok_or_else(|| BosshoggError::Config(
            "no project_token (phc_) on current context — set via `bosshogg config set-context --project-token phc_...`".into()
        ))?;

    let host = ctx.host.trim_end_matches('/').to_string();

    // Build a minimal reqwest client — no Authorization header (capture API
    // takes project token in the body). HTTPS-only is enforced to match the
    // main Client's posture; the test-harness feature permits a BOSSHOGG_ALLOW_HTTP
    // bypass exactly as in src/client/mod.rs so wiremock integration tests work.
    let http_allowed_in_test_harness =
        cfg!(feature = "test-harness") && std::env::var("BOSSHOGG_ALLOW_HTTP").is_ok();
    if http_allowed_in_test_harness {
        tracing::warn!(
            "TLS downgraded via BOSSHOGG_ALLOW_HTTP (test-harness feature); never use in production"
        );
    }
    let mut capture_headers = HeaderMap::new();
    capture_headers.insert(
        USER_AGENT,
        HeaderValue::from_static(concat!("bosshogg/", env!("CARGO_PKG_VERSION"))),
    );
    let http = reqwest::Client::builder()
        .https_only(!http_allowed_in_test_harness)
        .gzip(true)
        .timeout(Duration::from_secs(30))
        .default_headers(capture_headers)
        .build()
        .map_err(BosshoggError::Http)?;

    match args.command {
        CaptureCommand::Event {
            event,
            distinct_id,
            properties_file,
        } => {
            capture_event(
                &http,
                &host,
                &project_token,
                event,
                distinct_id,
                properties_file,
                json_mode,
                yes,
                debug,
            )
            .await
        }
        CaptureCommand::Batch { file } => {
            capture_batch(&http, &host, &project_token, file, json_mode, yes, debug).await
        }
        CaptureCommand::Identify {
            distinct_id,
            properties_file,
        } => {
            capture_identify(
                &http,
                &host,
                &project_token,
                distinct_id,
                properties_file,
                json_mode,
                yes,
                debug,
            )
            .await
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn send_capture(
    http: &reqwest::Client,
    url: &str,
    body: &Value,
    debug: bool,
) -> Result<CaptureResponse> {
    if debug {
        // Redact the api_key (project token) before logging.
        let mut redacted_body = body.clone();
        if let Some(key) = redacted_body.get("api_key").and_then(Value::as_str) {
            redacted_body["api_key"] = Value::String(crate::util::redact_key(key));
        }
        tracing::debug!(url, body = %redacted_body, "capture request");
    }

    let resp = http
        .post(url)
        .json(body)
        .send()
        .await
        .map_err(BosshoggError::Http)?;

    let status = resp.status();
    if debug {
        tracing::debug!(status = status.as_u16(), "capture response");
    }

    if !status.is_success() {
        let body_text = resp.text().await.unwrap_or_default();
        return Err(BosshoggError::ServerError {
            status: status.as_u16(),
            message: body_text
                .lines()
                .next()
                .unwrap_or("capture failed")
                .to_string(),
        });
    }

    let bytes = resp.bytes().await.map_err(BosshoggError::Http)?;
    if bytes.is_empty() {
        return Ok(CaptureResponse { status: 1 });
    }
    serde_json::from_slice::<CaptureResponse>(&bytes).map_err(BosshoggError::Json)
}

fn print_capture_result(resp: &CaptureResponse, json_mode: bool) {
    if json_mode {
        output::print_json(resp);
    } else {
        println!("Event captured (status: {})", resp.status);
    }
}

// ── event ─────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn capture_event(
    http: &reqwest::Client,
    host: &str,
    project_token: &str,
    event: String,
    distinct_id: String,
    properties_file: Option<PathBuf>,
    json_mode: bool,
    yes: bool,
    debug: bool,
) -> Result<()> {
    gate_destructive(
        yes,
        &format!("send event `{event}` to PostHog (real production data); continue?"),
    )?;

    let properties: Value = if let Some(p) = properties_file.as_deref() {
        read_json_file(p).await?
    } else {
        json!({})
    };

    let body = json!({
        "api_key": project_token,
        "event": event,
        "distinct_id": distinct_id,
        "properties": properties,
    });

    let url = format!("{host}/i/v0/e");
    let resp = send_capture(http, &url, &body, debug).await?;
    print_capture_result(&resp, json_mode);
    Ok(())
}

// ── batch ─────────────────────────────────────────────────────────────────────

async fn capture_batch(
    http: &reqwest::Client,
    host: &str,
    project_token: &str,
    file: PathBuf,
    json_mode: bool,
    yes: bool,
    debug: bool,
) -> Result<()> {
    gate_destructive(
        yes,
        "send batch events to PostHog (real production data); continue?",
    )?;

    let raw = tokio::fs::read_to_string(&file)
        .await
        .map_err(|e| BosshoggError::Config(format!("read {}: {e}", file.display())))?;

    let mut events: Vec<Value> = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(line)
            .map_err(|e| BosshoggError::BadRequest(format!("line {}: {e}", i + 1)))?;
        events.push(v);
    }

    if events.is_empty() {
        return Err(BosshoggError::BadRequest(
            "batch file contained no events".into(),
        ));
    }

    let body = json!({
        "api_key": project_token,
        "batch": events,
    });

    let url = format!("{host}/batch");
    let resp = send_capture(http, &url, &body, debug).await?;
    print_capture_result(&resp, json_mode);
    Ok(())
}

// ── identify ──────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn capture_identify(
    http: &reqwest::Client,
    host: &str,
    project_token: &str,
    distinct_id: String,
    properties_file: PathBuf,
    json_mode: bool,
    yes: bool,
    debug: bool,
) -> Result<()> {
    gate_destructive(
        yes,
        &format!("send identify for `{distinct_id}` to PostHog (real production data); continue?"),
    )?;

    let properties = read_json_file(&properties_file).await?;

    let body = json!({
        "api_key": project_token,
        "event": "$identify",
        "distinct_id": distinct_id,
        "properties": {
            "$set": properties,
        },
    });

    let url = format!("{host}/i/v0/e");
    let resp = send_capture(http, &url, &body, debug).await?;
    print_capture_result(&resp, json_mode);
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_response_roundtrip() {
        let raw = r#"{"status": 1}"#;
        let r: CaptureResponse = serde_json::from_str(raw).unwrap();
        assert_eq!(r.status, 1);
    }

    #[test]
    fn capture_response_serialize() {
        let r = CaptureResponse { status: 1 };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"status\":1"));
    }

    #[test]
    fn event_body_shape() {
        let body = json!({
            "api_key": "phc_testtoken",
            "event": "page_view",
            "distinct_id": "user123",
            "properties": {"url": "https://example.com"},
        });
        assert_eq!(body["api_key"], "phc_testtoken");
        assert_eq!(body["event"], "page_view");
        assert_eq!(body["distinct_id"], "user123");
    }

    #[test]
    fn batch_body_shape() {
        let events = vec![
            json!({"event": "click", "distinct_id": "u1"}),
            json!({"event": "view", "distinct_id": "u2"}),
        ];
        let body = json!({
            "api_key": "phc_test",
            "batch": events,
        });
        assert!(body["batch"].as_array().unwrap().len() == 2);
    }

    #[test]
    fn identify_body_shape() {
        let props = json!({"name": "Alice", "plan": "pro"});
        let body = json!({
            "api_key": "phc_test",
            "event": "$identify",
            "distinct_id": "alice@example.com",
            "properties": { "$set": props },
        });
        assert_eq!(body["event"], "$identify");
        assert_eq!(body["properties"]["$set"]["name"], "Alice");
    }
}
