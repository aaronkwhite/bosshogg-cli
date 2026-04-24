// src/commands/session_recording.rs
//! `bosshogg session-recording` — list / get / update / delete.
//!
//! Session recordings are environment-scoped.
//! SAFETY: responses can include a `snapshots` field containing large
//! compressed rrweb JSON. We NEVER let that hit stdout unguarded.
//! `get` strips `snapshots` by default; opt-in via `--with-snapshots`
//! combined with `--out <file>`.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::commands::context::CommandContext;
use crate::commands::util::env_id_required;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────
// `snapshots` intentionally excluded — callers opt in via --with-snapshots.

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SessionRecording {
    pub id: String,
    #[serde(default)]
    pub distinct_id: Option<String>,
    #[serde(default)]
    pub viewed: Option<bool>,
    #[serde(default)]
    pub recording_duration: Option<i64>,
    #[serde(default)]
    pub active_seconds: Option<i64>,
    #[serde(default)]
    pub inactive_seconds: Option<i64>,
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub end_time: Option<String>,
    #[serde(default)]
    pub click_count: Option<i64>,
    #[serde(default)]
    pub keypress_count: Option<i64>,
    #[serde(default)]
    pub console_log_count: Option<i64>,
    #[serde(default)]
    pub console_warn_count: Option<i64>,
    #[serde(default)]
    pub console_error_count: Option<i64>,
    #[serde(default)]
    pub start_url: Option<String>,
    #[serde(default)]
    pub person: Option<Value>,
    #[serde(default)]
    pub storage: Option<String>,
    #[serde(default)]
    pub pinned_count: Option<i64>,
    #[serde(default)]
    pub ongoing: Option<bool>,
    #[serde(default)]
    pub activity_score: Option<f64>,
    #[serde(default)]
    pub snapshot_source: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct SessionRecordingArgs {
    #[command(subcommand)]
    pub command: SessionRecordingCommand,
}

#[derive(Subcommand, Debug)]
pub enum SessionRecordingCommand {
    /// List session recordings with optional filters.
    List {
        /// Filter by person UUID.
        #[arg(long)]
        person_id: Option<String>,
        /// Filter by distinct ID.
        #[arg(long)]
        distinct_id: Option<String>,
        /// Only recordings before this timestamp (ISO 8601).
        #[arg(long)]
        before: Option<String>,
        /// Only recordings after this timestamp (ISO 8601).
        #[arg(long)]
        after: Option<String>,
        /// Maximum number of results.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single session recording by ID.
    ///
    /// SAFETY: snapshots (large rrweb JSON) are excluded by default.
    /// Use --with-snapshots --out <file> to write the full payload to disk.
    Get {
        id: String,
        /// Include raw snapshot blobs in output (can be very large).
        #[arg(long)]
        with_snapshots: bool,
        /// Write full JSON (including snapshots if --with-snapshots) to this file.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Update a session recording (e.g. mark as viewed or un-delete).
    Update {
        id: String,
        /// Set deleted=false to un-delete (restore) a soft-deleted recording.
        #[arg(long)]
        deleted: Option<bool>,
        /// Mark recording as viewed.
        #[arg(long)]
        viewed: Option<bool>,
    },
    /// Soft-delete a session recording (PATCH {"deleted": true}).
    Delete { id: String },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: SessionRecordingArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        SessionRecordingCommand::List {
            person_id,
            distinct_id,
            before,
            after,
            limit,
        } => list_recordings(cx, person_id, distinct_id, before, after, limit).await,
        SessionRecordingCommand::Get {
            id,
            with_snapshots,
            out,
        } => get_recording(cx, id, with_snapshots, out).await,
        SessionRecordingCommand::Update {
            id,
            deleted,
            viewed,
        } => update_recording(cx, id, deleted, viewed).await,
        SessionRecordingCommand::Delete { id } => delete_recording(cx, id).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<SessionRecording>,
}

async fn list_recordings(
    cx: &CommandContext,
    person_id: Option<String>,
    distinct_id: Option<String>,
    before: Option<String>,
    after: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut params: Vec<String> = Vec::new();
    if let Some(p) = person_id {
        params.push(format!("person_uuid={}", urlencoding::encode(&p)));
    }
    if let Some(d) = distinct_id {
        params.push(format!("distinct_id={}", urlencoding::encode(&d)));
    }
    if let Some(b) = before {
        params.push(format!("before={}", urlencoding::encode(&b)));
    }
    if let Some(a) = after {
        params.push(format!("after={}", urlencoding::encode(&a)));
    }
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };

    let path = format!("/api/environments/{env_id}/session_recordings/{query}");
    let results: Vec<SessionRecording> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "DISTINCT_ID", "DURATION_S", "VIEWED", "START_TIME"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.distinct_id.clone().unwrap_or_else(|| "-".into()),
                    r.recording_duration
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "-".into()),
                    r.viewed
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    r.start_time.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_recording(
    cx: &CommandContext,
    id: String,
    with_snapshots: bool,
    out: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    // Always fetch the full raw Value so we can handle snapshots safely.
    let raw: Value = client
        .get(&format!(
            "/api/environments/{env_id}/session_recordings/{id}/"
        ))
        .await?;

    if with_snapshots {
        // User explicitly opted in to snapshots.
        if let Some(out_path) = out {
            // Write full JSON to disk.
            let content = serde_json::to_string(&raw).map_err(BosshoggError::Json)?;
            tokio::fs::write(&out_path, content)
                .await
                .map_err(BosshoggError::Io)?;
            if cx.json_mode {
                output::print_json(&json!({
                    "ok": true,
                    "written_to": out_path.display().to_string(),
                    "id": id,
                }));
            } else {
                println!(
                    "Full recording (with snapshots) written to {}",
                    out_path.display()
                );
            }
        } else {
            // TTY or piped: warn that snapshots can be huge; show terse summary.
            let snapshot_count = raw
                .get("snapshots")
                .and_then(Value::as_array)
                .map(|a| a.len())
                .unwrap_or(0);
            let snapshot_bytes = raw
                .get("snapshots")
                .map(|s| serde_json::to_string(s).unwrap_or_default().len())
                .unwrap_or(0);

            if cx.json_mode {
                // Strip snapshots before printing to stdout — safety rule.
                let mut stripped = raw.clone();
                if let Some(obj) = stripped.as_object_mut() {
                    obj.remove("snapshots");
                }
                output::print_json(&stripped);
                eprintln!(
                    "note: {} snapshot blob(s), ~{} bytes — use --out <file> to write full payload",
                    snapshot_count, snapshot_bytes
                );
            } else {
                // Print metadata summary; snapshots suppressed.
                let meta: SessionRecording =
                    serde_json::from_value(strip_snapshots(raw)).map_err(BosshoggError::Json)?;
                print_recording(&meta, false);
                eprintln!(
                    "note: {} snapshot blob(s), ~{} bytes — redirect to --out <file> to capture",
                    snapshot_count, snapshot_bytes
                );
            }
        }
    } else {
        // Default mode: strip snapshots entirely.
        let stripped = strip_snapshots(raw);
        if cx.json_mode {
            output::print_json(&stripped);
        } else {
            let meta: SessionRecording =
                serde_json::from_value(stripped).map_err(BosshoggError::Json)?;
            print_recording(&meta, false);
        }
    }

    Ok(())
}

/// Remove the `snapshots` key from a JSON Value (non-destructive clone if needed).
fn strip_snapshots(mut v: Value) -> Value {
    if let Some(obj) = v.as_object_mut() {
        obj.remove("snapshots");
    }
    v
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_recording(
    cx: &CommandContext,
    id: String,
    deleted: Option<bool>,
    viewed: Option<bool>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(d) = deleted {
        body.insert("deleted".into(), Value::Bool(d));
    }
    if let Some(v) = viewed {
        body.insert("viewed".into(), Value::Bool(v));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --deleted, --viewed)".into(),
        ));
    }

    cx.confirm(&format!("update session recording `{id}`; continue?"))?;

    let updated: Value = client
        .patch(
            &format!("/api/environments/{env_id}/session_recordings/{id}/"),
            &Value::Object(body),
        )
        .await?;

    // Strip snapshots from the response before printing.
    let stripped = strip_snapshots(updated);

    if cx.json_mode {
        output::print_json(&stripped);
    } else {
        let meta: SessionRecording =
            serde_json::from_value(stripped).map_err(BosshoggError::Json)?;
        println!("Updated session recording {}", meta.id);
        print_recording(&meta, false);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_recording(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("delete session recording `{id}`; continue?"))?;

    // session_recordings is NOT in SOFT_DELETE_RESOURCES — use hard delete.
    client
        .delete(&format!(
            "/api/environments/{env_id}/session_recordings/{id}/"
        ))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "delete",
            id,
        });
    } else {
        println!("Deleted session recording {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_recording(r: &SessionRecording, _json_mode: bool) {
    println!("ID:             {}", r.id);
    if let Some(d) = r.distinct_id.as_deref() {
        println!("Distinct ID:    {d}");
    }
    if let Some(v) = r.viewed {
        println!("Viewed:         {v}");
    }
    if let Some(dur) = r.recording_duration {
        println!("Duration (s):   {dur}");
    }
    if let Some(st) = r.start_time.as_deref() {
        println!("Start:          {st}");
    }
    if let Some(et) = r.end_time.as_deref() {
        println!("End:            {et}");
    }
    if let Some(url) = r.start_url.as_deref() {
        println!("Start URL:      {url}");
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_recording_roundtrip_minimal() {
        let raw = r#"{"id": "rec-1"}"#;
        let r: SessionRecording = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "rec-1");
        assert!(r.distinct_id.is_none());
    }

    #[test]
    fn session_recording_roundtrip_full() {
        let raw = r#"{
            "id": "rec-full",
            "distinct_id": "user123",
            "viewed": true,
            "recording_duration": 120,
            "active_seconds": 80,
            "inactive_seconds": 40,
            "start_time": "2026-01-01T10:00:00Z",
            "end_time": "2026-01-01T10:02:00Z",
            "click_count": 5,
            "keypress_count": 20,
            "console_log_count": 3,
            "console_warn_count": 1,
            "console_error_count": 0,
            "start_url": "https://example.com",
            "person": {"id": "person-1"},
            "storage": "object_storage",
            "pinned_count": 0,
            "ongoing": false,
            "activity_score": 7.5,
            "snapshot_source": "realtime"
        }"#;
        let r: SessionRecording = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "rec-full");
        assert_eq!(r.distinct_id.as_deref(), Some("user123"));
        assert_eq!(r.viewed, Some(true));
        assert_eq!(r.recording_duration, Some(120));
        assert_eq!(r.activity_score, Some(7.5));
    }

    #[test]
    fn session_recording_snapshots_not_in_struct() {
        // Even if the API returns `snapshots`, our struct does not capture it.
        let raw = r#"{
            "id": "rec-with-snaps",
            "snapshots": [{"type": 2, "data": {"source": 1}}]
        }"#;
        let r: SessionRecording = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "rec-with-snaps");
        // No panic — snapshots field silently ignored.
        let out = serde_json::to_string(&r).unwrap();
        assert!(!out.contains("snapshots"));
    }

    #[test]
    fn strip_snapshots_removes_key() {
        let v = serde_json::json!({
            "id": "rec-1",
            "distinct_id": "u1",
            "snapshots": [{"data": "huge"}]
        });
        let stripped = strip_snapshots(v);
        assert!(stripped.get("snapshots").is_none());
        assert_eq!(stripped["id"], "rec-1");
    }

    #[test]
    fn strip_snapshots_noop_when_absent() {
        let v = serde_json::json!({"id": "rec-2"});
        let stripped = strip_snapshots(v);
        assert_eq!(stripped["id"], "rec-2");
    }
}
