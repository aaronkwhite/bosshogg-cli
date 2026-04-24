// src/commands/batch_export.rs
//! `bosshogg batch-export` — list / get / create / update / delete / pause /
//! unpause / backfills / runs.
//!
//! Batch exports are environment-scoped. Deletion is a HARD DELETE
//! (batch_exports is NOT in SOFT_DELETE_RESOURCES).

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::commands::context::CommandContext;
use crate::commands::util::{env_id_required, read_json_file};
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed structs ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BatchExport {
    pub id: String,
    pub name: String,
    pub destination: Value,
    pub interval: String,
    #[serde(default)]
    pub paused: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_updated_at: Option<String>,
    #[serde(default)]
    pub last_paused_at: Option<String>,
    #[serde(default)]
    pub start_at: Option<String>,
    #[serde(default)]
    pub end_at: Option<String>,
    #[serde(default)]
    pub schema: Option<Value>,
    #[serde(default)]
    pub model: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct BatchExportArgs {
    #[command(subcommand)]
    pub command: BatchExportCommand,
}

#[derive(Subcommand, Debug)]
pub enum BatchExportCommand {
    /// List all batch exports.
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single batch export by UUID.
    Get { id: String },
    /// Create a new batch export.
    Create {
        #[arg(long)]
        name: String,
        /// Path to a JSON file defining the destination (type, config, etc.).
        #[arg(long)]
        destination_file: PathBuf,
        /// Export interval (e.g. "hour", "day", "every 5 minutes").
        #[arg(long)]
        interval: Option<String>,
    },
    /// Update a batch export's fields.
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// Set the export to paused state.
        #[arg(long)]
        paused: bool,
        /// Path to a JSON file with updated destination config.
        #[arg(long)]
        destination_file: Option<PathBuf>,
    },
    /// Hard-delete a batch export.
    Delete { id: String },
    /// Pause a batch export.
    Pause { id: String },
    /// Unpause a batch export.
    Unpause { id: String },
    /// Manage backfills for a batch export.
    #[command(subcommand)]
    Backfills(BackfillsCommand),
    /// Manage runs for a batch export.
    #[command(subcommand)]
    Runs(RunsCommand),
}

#[derive(Subcommand, Debug)]
pub enum BackfillsCommand {
    /// List backfills for a batch export.
    List { export_id: String },
    /// Create a new backfill for a batch export.
    Create {
        export_id: String,
        /// Start timestamp for the backfill (ISO 8601).
        #[arg(long)]
        start_at: String,
        /// End timestamp for the backfill (ISO 8601).
        #[arg(long)]
        end_at: Option<String>,
    },
    /// Cancel a backfill.
    Cancel {
        export_id: String,
        backfill_id: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum RunsCommand {
    /// List runs for a batch export.
    List { export_id: String },
    /// Get a single run by ID.
    Get { export_id: String, run_id: String },
    /// Fetch logs for a run.
    Logs { export_id: String, run_id: String },
    /// Cancel a run.
    Cancel { export_id: String, run_id: String },
    /// Retry a run.
    Retry { export_id: String, run_id: String },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: BatchExportArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        BatchExportCommand::List { limit } => list_batch_exports(cx, limit).await,
        BatchExportCommand::Get { id } => get_batch_export(cx, id).await,
        BatchExportCommand::Create {
            name,
            destination_file,
            interval,
        } => create_batch_export(cx, name, destination_file, interval).await,
        BatchExportCommand::Update {
            id,
            name,
            paused,
            destination_file,
        } => update_batch_export(cx, id, name, paused, destination_file).await,
        BatchExportCommand::Delete { id } => delete_batch_export(cx, id).await,
        BatchExportCommand::Pause { id } => pause_batch_export(cx, id).await,
        BatchExportCommand::Unpause { id } => unpause_batch_export(cx, id).await,
        BatchExportCommand::Backfills(cmd) => dispatch_backfills(cx, cmd).await,
        BatchExportCommand::Runs(cmd) => dispatch_runs(cx, cmd).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<BatchExport>,
}

async fn list_batch_exports(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/batch_exports/");
    let results: Vec<BatchExport> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "INTERVAL", "PAUSED", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|e| {
                vec![
                    e.id.clone(),
                    e.name.clone(),
                    e.interval.clone(),
                    e.paused
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".into()),
                    e.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_batch_export(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let e: BatchExport = client
        .get(&format!("/api/environments/{env_id}/batch_exports/{id}/"))
        .await?;
    if cx.json_mode {
        output::print_json(&e);
    } else {
        println!("ID:          {}", e.id);
        println!("Name:        {}", e.name);
        println!("Interval:    {}", e.interval);
        println!(
            "Paused:      {}",
            e.paused
                .map(|p| p.to_string())
                .unwrap_or_else(|| "-".into())
        );
        if let Some(ca) = e.created_at.as_deref() {
            println!("Created:     {ca}");
        }
    }
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_batch_export(
    cx: &CommandContext,
    name: String,
    destination_file: PathBuf,
    interval: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let destination = read_json_file(&destination_file).await?;
    let mut body = json!({
        "name": name,
        "destination": destination,
    });
    if let Some(iv) = interval {
        body["interval"] = Value::String(iv);
    }

    let created: BatchExport = client
        .post(&format!("/api/environments/{env_id}/batch_exports/"), &body)
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: String,
            name: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            name: created.name,
        });
    } else {
        println!(
            "Created batch export '{}' (id {})",
            created.name, created.id
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_batch_export(
    cx: &CommandContext,
    id: String,
    name: Option<String>,
    paused: bool,
    destination_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if paused {
        body.insert("paused".into(), Value::Bool(true));
    }
    if let Some(p) = destination_file.as_deref() {
        body.insert("destination".into(), read_json_file(p).await?);
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --paused, --destination-file)".into(),
        ));
    }

    cx.confirm(&format!("update batch export `{id}`; continue?"))?;

    let updated: BatchExport = client
        .patch(
            &format!("/api/environments/{env_id}/batch_exports/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated batch export '{}' (id {})",
            updated.name, updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_batch_export(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("hard-delete batch export `{id}`; continue?"))?;

    // batch_exports is NOT in SOFT_DELETE_RESOURCES — use raw HTTP DELETE.
    // client.delete() will issue a true DELETE (not PATCH) for this path.
    client
        .delete(&format!("/api/environments/{env_id}/batch_exports/{id}/"))
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
        println!("Deleted batch export {id}");
    }
    Ok(())
}

// ── pause ─────────────────────────────────────────────────────────────────────

async fn pause_batch_export(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("pause batch export `{id}`; continue?"))?;

    let updated: BatchExport = client
        .patch(
            &format!("/api/environments/{env_id}/batch_exports/{id}/"),
            &json!({ "paused": true }),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Paused batch export '{}'", updated.name);
    }
    Ok(())
}

// ── unpause ───────────────────────────────────────────────────────────────────

async fn unpause_batch_export(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("unpause batch export `{id}`; continue?"))?;

    let updated: BatchExport = client
        .patch(
            &format!("/api/environments/{env_id}/batch_exports/{id}/"),
            &json!({ "paused": false }),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Unpaused batch export '{}'", updated.name);
    }
    Ok(())
}

// ── backfills dispatch ────────────────────────────────────────────────────────

async fn dispatch_backfills(cx: &CommandContext, cmd: BackfillsCommand) -> Result<()> {
    match cmd {
        BackfillsCommand::List { export_id } => backfills_list(cx, export_id).await,
        BackfillsCommand::Create {
            export_id,
            start_at,
            end_at,
        } => backfills_create(cx, export_id, start_at, end_at).await,
        BackfillsCommand::Cancel {
            export_id,
            backfill_id,
        } => backfills_cancel(cx, export_id, backfill_id).await,
    }
}

async fn backfills_list(cx: &CommandContext, export_id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/batch_exports/{export_id}/backfills/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{v}");
    }
    Ok(())
}

async fn backfills_create(
    cx: &CommandContext,
    export_id: String,
    start_at: String,
    end_at: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "create backfill for batch export `{export_id}`; continue?"
    ))?;

    let mut body = json!({ "start_at": start_at });
    if let Some(ea) = end_at {
        body["end_at"] = Value::String(ea);
    }

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/batch_exports/{export_id}/backfills/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Created backfill for batch export {export_id}");
    }
    Ok(())
}

async fn backfills_cancel(
    cx: &CommandContext,
    export_id: String,
    backfill_id: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "cancel backfill `{backfill_id}` for batch export `{export_id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!(
                "/api/environments/{env_id}/batch_exports/{export_id}/backfills/{backfill_id}/cancel/"
            ),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Cancelled backfill {backfill_id} for batch export {export_id}");
    }
    Ok(())
}

// ── runs dispatch ─────────────────────────────────────────────────────────────

async fn dispatch_runs(cx: &CommandContext, cmd: RunsCommand) -> Result<()> {
    match cmd {
        RunsCommand::List { export_id } => runs_list(cx, export_id).await,
        RunsCommand::Get { export_id, run_id } => runs_get(cx, export_id, run_id).await,
        RunsCommand::Logs { export_id, run_id } => runs_logs(cx, export_id, run_id).await,
        RunsCommand::Cancel { export_id, run_id } => runs_cancel(cx, export_id, run_id).await,
        RunsCommand::Retry { export_id, run_id } => runs_retry(cx, export_id, run_id).await,
    }
}

async fn runs_list(cx: &CommandContext, export_id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/batch_exports/{export_id}/runs/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{v}");
    }
    Ok(())
}

async fn runs_get(cx: &CommandContext, export_id: String, run_id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/batch_exports/{export_id}/runs/{run_id}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{v}");
    }
    Ok(())
}

async fn runs_logs(cx: &CommandContext, export_id: String, run_id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/batch_exports/{export_id}/runs/{run_id}/logs/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        if let Some(results) = v.get("results").and_then(Value::as_array) {
            for entry in results {
                let ts = entry
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .unwrap_or("-");
                let lvl = entry.get("level").and_then(Value::as_str).unwrap_or("INFO");
                let msg = entry.get("message").and_then(Value::as_str).unwrap_or("");
                println!("{ts}  [{lvl}]  {msg}");
            }
        } else {
            output::print_json(&v);
        }
    }
    Ok(())
}

async fn runs_cancel(cx: &CommandContext, export_id: String, run_id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "cancel run `{run_id}` for batch export `{export_id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/batch_exports/{export_id}/runs/{run_id}/cancel/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Cancelled run {run_id} for batch export {export_id}");
    }
    Ok(())
}

async fn runs_retry(cx: &CommandContext, export_id: String, run_id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "retry run `{run_id}` for batch export `{export_id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/batch_exports/{export_id}/runs/{run_id}/retry/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Retried run {run_id} for batch export {export_id}");
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_export_roundtrip_minimal() {
        let raw = r#"{
            "id": "exp-abc",
            "name": "S3 Export",
            "destination": {"type": "S3"},
            "interval": "hour"
        }"#;
        let e: BatchExport = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, "exp-abc");
        assert_eq!(e.name, "S3 Export");
        assert_eq!(e.interval, "hour");
        assert_eq!(e.paused, None);
    }

    #[test]
    fn batch_export_roundtrip_full() {
        let raw = r#"{
            "id": "exp-def",
            "name": "BigQuery Daily",
            "destination": {"type": "BigQuery", "config": {"project_id": "my-project"}},
            "interval": "day",
            "paused": false,
            "created_at": "2026-01-01T00:00:00Z",
            "last_updated_at": "2026-04-01T00:00:00Z",
            "last_paused_at": null,
            "start_at": "2026-01-01T00:00:00Z",
            "end_at": null,
            "schema": null,
            "model": "events"
        }"#;
        let e: BatchExport = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, "exp-def");
        assert_eq!(e.paused, Some(false));
        assert_eq!(e.model, Some("events".to_string()));
    }
}
