// src/commands/llm_analytics.rs
//! `bosshogg llm-analytics` — models / evaluation-summary /
//! evaluation-reports / provider-keys / review-queue.
//!
//! All endpoints are environment-scoped:
//!   `/api/environments/{project_id}/llm_analytics/...`
//! where `{project_id}` resolves from the configured `env_id` (same as
//! other `/api/environments/` endpoints).
//!
//! Deferred (admin/setup flows, not shipped in this release):
//!   - clustering_config / clustering_config/set_event_filters
//!   - clustering_jobs (job orchestration)
//!   - evaluation_config / evaluation_config/set_active_key
//!   - provider_keys write paths (assign / dependent_configs /
//!     trial_evaluations / create / update / delete)

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
pub struct EvaluationReport {
    pub id: String,
    #[serde(default)]
    pub evaluation: Option<String>,
    #[serde(default)]
    pub frequency: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub next_delivery_date: Option<String>,
    #[serde(default)]
    pub last_delivered_at: Option<String>,
    #[serde(default)]
    pub rrule: Option<String>,
    #[serde(default)]
    pub timezone_name: Option<String>,
    #[serde(default)]
    pub delivery_targets: Option<Value>,
    #[serde(default)]
    pub report_prompt_guidance: Option<String>,
    #[serde(default)]
    pub trigger_threshold: Option<Value>,
    #[serde(default)]
    pub max_sample_size: Option<i64>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct LlmProviderKey {
    pub id: String,
    #[serde(default)]
    pub provider: Option<String>,
    pub name: String,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub api_key_masked: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_used_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ReviewQueueItem {
    pub id: String,
    #[serde(default)]
    pub trace_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub queue: Option<Value>,
    #[serde(default)]
    pub status: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct LlmAnalyticsArgs {
    #[command(subcommand)]
    pub command: LlmAnalyticsCommand,
}

#[derive(Subcommand, Debug)]
pub enum LlmAnalyticsCommand {
    /// List available LLM models for the configured provider.
    #[command(subcommand)]
    Models(ModelsCommand),
    /// Get an AI-powered summary of evaluation results.
    ///
    /// Analyzes evaluation runs and identifies pass/fail patterns with
    /// actionable recommendations. This is synchronous — returns immediately.
    #[command(name = "evaluation-summary")]
    EvaluationSummary {
        /// UUID of the evaluation to summarize.
        #[arg(long)]
        evaluation_id: String,
        /// Filter to include: `all`, `pass`, `fail`, or `na`.
        #[arg(long, default_value = "all")]
        filter: String,
        /// Force a fresh summary even if a cached one exists.
        #[arg(long)]
        force_refresh: bool,
    },
    /// Manage evaluation report configurations (CRUD + generate + runs).
    #[command(name = "evaluation-reports", subcommand)]
    EvaluationReports(EvaluationReportsCommand),
    /// Read and validate LLM provider keys (read-only; write paths deferred).
    #[command(name = "provider-keys", subcommand)]
    ProviderKeys(ProviderKeysCommand),
    /// List LLM analytics review queue items.
    #[command(name = "review-queue", subcommand)]
    ReviewQueue(ReviewQueueCommand),
}

// ── models sub-enum ───────────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ModelsCommand {
    /// List available LLM models.
    List,
}

// ── evaluation-reports sub-enum ───────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum EvaluationReportsCommand {
    /// List evaluation report configurations (paginated).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single evaluation report configuration by ID.
    Get { id: String },
    /// Create a new evaluation report configuration.
    Create {
        /// Path to a JSON file with the report configuration body.
        #[arg(long)]
        config_file: PathBuf,
    },
    /// Update an evaluation report configuration.
    Update {
        /// Report configuration UUID.
        id: String,
        /// Path to a JSON file with the updated configuration fields (PATCH).
        #[arg(long)]
        config_file: PathBuf,
    },
    /// Soft-delete an evaluation report configuration (sets deleted=true).
    ///
    /// Note: the PostHog API returns 405 on hard DELETE for this resource.
    /// This command sends PATCH {"deleted": true} instead.
    Delete {
        /// Report configuration UUID.
        id: String,
    },
    /// Trigger immediate report generation for a report configuration.
    ///
    /// Returns 202 (accepted) — generation runs asynchronously server-side.
    Generate {
        /// Report configuration UUID.
        id: String,
    },
    /// List report run history for a report configuration.
    Runs {
        /// Report configuration UUID.
        id: String,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
}

// ── provider-keys sub-enum ────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ProviderKeysCommand {
    /// List LLM provider keys (read-only; write paths deferred).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single provider key by ID.
    Get { id: String },
    /// Validate a provider key (POST to /{id}/validate/).
    Validate { id: String },
}

// ── review-queue sub-enum ─────────────────────────────────────────────────────

#[derive(Subcommand, Debug)]
pub enum ReviewQueueCommand {
    /// List review queue items (paginated).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: LlmAnalyticsArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        LlmAnalyticsCommand::Models(cmd) => dispatch_models(cx, cmd).await,
        LlmAnalyticsCommand::EvaluationSummary {
            evaluation_id,
            filter,
            force_refresh,
        } => evaluation_summary(cx, evaluation_id, filter, force_refresh).await,
        LlmAnalyticsCommand::EvaluationReports(cmd) => dispatch_evaluation_reports(cx, cmd).await,
        LlmAnalyticsCommand::ProviderKeys(cmd) => dispatch_provider_keys(cx, cmd).await,
        LlmAnalyticsCommand::ReviewQueue(cmd) => dispatch_review_queue(cx, cmd).await,
    }
}

// ── models ────────────────────────────────────────────────────────────────────

async fn dispatch_models(cx: &CommandContext, cmd: ModelsCommand) -> Result<()> {
    match cmd {
        ModelsCommand::List => list_models(cx).await,
    }
}

async fn list_models(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    // The models endpoint is a single GET — returns object, not a paginated list.
    let v: Value = client
        .get(&format!("/api/environments/{env_id}/llm_analytics/models/"))
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── evaluation-summary ────────────────────────────────────────────────────────

async fn evaluation_summary(
    cx: &CommandContext,
    evaluation_id: String,
    filter: String,
    force_refresh: bool,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let body = json!({
        "evaluation_id": evaluation_id,
        "filter": filter,
        "force_refresh": force_refresh,
    });

    let resp: Value = client
        .post(
            &format!("/api/environments/{env_id}/llm_analytics/evaluation_summary/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&resp);
    } else {
        if let Some(assessment) = resp.get("overall_assessment").and_then(Value::as_str) {
            println!("Overall: {assessment}");
        }
        if let Some(stats) = resp.get("statistics") {
            if let (Some(total), Some(pass), Some(fail)) = (
                stats.get("total_analyzed").and_then(Value::as_i64),
                stats.get("pass_count").and_then(Value::as_i64),
                stats.get("fail_count").and_then(Value::as_i64),
            ) {
                println!("Stats: {total} analyzed — {pass} pass / {fail} fail");
            }
        }
        if let Some(recs) = resp.get("recommendations").and_then(Value::as_array) {
            for rec in recs {
                if let Some(s) = rec.as_str() {
                    println!("  - {s}");
                }
            }
        }
        if resp.get("overall_assessment").is_none() {
            println!(
                "{}",
                serde_json::to_string_pretty(&resp).unwrap_or_default()
            );
        }
    }
    Ok(())
}

// ── evaluation-reports ────────────────────────────────────────────────────────

async fn dispatch_evaluation_reports(
    cx: &CommandContext,
    cmd: EvaluationReportsCommand,
) -> Result<()> {
    match cmd {
        EvaluationReportsCommand::List { limit } => list_evaluation_reports(cx, limit).await,
        EvaluationReportsCommand::Get { id } => get_evaluation_report(cx, id).await,
        EvaluationReportsCommand::Create { config_file } => {
            create_evaluation_report(cx, config_file).await
        }
        EvaluationReportsCommand::Update { id, config_file } => {
            update_evaluation_report(cx, id, config_file).await
        }
        EvaluationReportsCommand::Delete { id } => delete_evaluation_report(cx, id).await,
        EvaluationReportsCommand::Generate { id } => generate_evaluation_report(cx, id).await,
        EvaluationReportsCommand::Runs { id, limit } => {
            list_evaluation_report_runs(cx, id, limit).await
        }
    }
}

#[derive(Serialize)]
struct EvaluationReportsListOutput {
    count: usize,
    results: Vec<EvaluationReport>,
}

async fn list_evaluation_reports(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/llm_analytics/evaluation_reports/");
    let results: Vec<EvaluationReport> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&EvaluationReportsListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "EVALUATION", "FREQUENCY", "ENABLED", "NEXT_DELIVERY"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.evaluation.clone().unwrap_or_else(|| "-".into()),
                    r.frequency.clone().unwrap_or_else(|| "-".into()),
                    r.enabled
                        .map(|b| if b { "yes" } else { "no" }.to_string())
                        .unwrap_or_else(|| "-".into()),
                    r.next_delivery_date.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn get_evaluation_report(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let r: EvaluationReport = client
        .get(&format!(
            "/api/environments/{env_id}/llm_analytics/evaluation_reports/{id}/"
        ))
        .await?;

    if cx.json_mode {
        output::print_json(&r);
    } else {
        println!("ID:           {}", r.id);
        if let Some(ev) = r.evaluation.as_deref() {
            println!("Evaluation:   {ev}");
        }
        if let Some(freq) = r.frequency.as_deref() {
            println!("Frequency:    {freq}");
        }
        if let Some(en) = r.enabled {
            println!("Enabled:      {}", if en { "yes" } else { "no" });
        }
        if let Some(nd) = r.next_delivery_date.as_deref() {
            println!("Next:         {nd}");
        }
        if let Some(ld) = r.last_delivered_at.as_deref() {
            println!("Last sent:    {ld}");
        }
    }
    Ok(())
}

async fn create_evaluation_report(cx: &CommandContext, config_file: PathBuf) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let body = read_json_file(&config_file).await?;

    cx.confirm("create evaluation report config; continue?")?;

    let created: EvaluationReport = client
        .post(
            &format!("/api/environments/{env_id}/llm_analytics/evaluation_reports/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&created);
    } else {
        println!("Created evaluation report {}", created.id);
    }
    Ok(())
}

async fn update_evaluation_report(
    cx: &CommandContext,
    id: String,
    config_file: PathBuf,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let body = read_json_file(&config_file).await?;

    cx.confirm(&format!("update evaluation report `{id}`; continue?"))?;

    let updated: EvaluationReport = client
        .patch(
            &format!("/api/environments/{env_id}/llm_analytics/evaluation_reports/{id}/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated evaluation report {}", updated.id);
    }
    Ok(())
}

async fn delete_evaluation_report(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "soft-delete evaluation report `{id}` (sets deleted=true); continue?"
    ))?;

    // PostHog returns 405 on hard DELETE for evaluation reports.
    // Use PATCH {"deleted": true} for soft-delete.
    let updated: EvaluationReport = client
        .patch(
            &format!("/api/environments/{env_id}/llm_analytics/evaluation_reports/{id}/"),
            &json!({ "deleted": true }),
        )
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
            action: "soft-delete",
            id: updated.id,
        });
    } else {
        println!("Soft-deleted evaluation report {}", updated.id);
    }
    Ok(())
}

async fn generate_evaluation_report(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("trigger report generation for `{id}`; continue?"))?;

    // POST /{id}/generate/ — returns 202 (no body).
    let _v: Value = client
        .post(
            &format!("/api/environments/{env_id}/llm_analytics/evaluation_reports/{id}/generate/"),
            &json!({}),
        )
        .await
        .unwrap_or(Value::Null);

    if cx.json_mode {
        output::print_json(&json!({ "ok": true, "action": "generate", "id": id }));
    } else {
        println!("Report generation triggered for {id} (async, check runs for status)");
    }
    Ok(())
}

async fn list_evaluation_report_runs(
    cx: &CommandContext,
    id: String,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/llm_analytics/evaluation_reports/{id}/runs/");
    let results: Vec<Value> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            count: usize,
            results: Vec<Value>,
        }
        output::print_json(&Out {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "STATUS", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.get("id")
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .to_string(),
                    r.get("status")
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .to_string(),
                    r.get("created_at")
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .to_string(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── provider-keys ─────────────────────────────────────────────────────────────

async fn dispatch_provider_keys(cx: &CommandContext, cmd: ProviderKeysCommand) -> Result<()> {
    match cmd {
        ProviderKeysCommand::List { limit } => list_provider_keys(cx, limit).await,
        ProviderKeysCommand::Get { id } => get_provider_key(cx, id).await,
        ProviderKeysCommand::Validate { id } => validate_provider_key(cx, id).await,
    }
}

#[derive(Serialize)]
struct ProviderKeysListOutput {
    count: usize,
    results: Vec<LlmProviderKey>,
}

async fn list_provider_keys(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/llm_analytics/provider_keys/");
    let results: Vec<LlmProviderKey> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ProviderKeysListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "PROVIDER", "STATE", "LAST_USED"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|k| {
                vec![
                    k.id.clone(),
                    k.name.clone(),
                    k.provider.clone().unwrap_or_else(|| "-".into()),
                    k.state.clone().unwrap_or_else(|| "-".into()),
                    k.last_used_at.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn get_provider_key(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let k: LlmProviderKey = client
        .get(&format!(
            "/api/environments/{env_id}/llm_analytics/provider_keys/{id}/"
        ))
        .await?;

    if cx.json_mode {
        output::print_json(&k);
    } else {
        println!("ID:          {}", k.id);
        println!("Name:        {}", k.name);
        if let Some(p) = k.provider.as_deref() {
            println!("Provider:    {p}");
        }
        if let Some(s) = k.state.as_deref() {
            println!("State:       {s}");
        }
        if let Some(m) = k.api_key_masked.as_deref() {
            println!("Key (masked): {m}");
        }
        if let Some(lu) = k.last_used_at.as_deref() {
            println!("Last used:   {lu}");
        }
        if let Some(err) = k.error_message.as_deref() {
            if !err.is_empty() {
                println!("Error:       {err}");
            }
        }
    }
    Ok(())
}

async fn validate_provider_key(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    // POST /{id}/validate/ with empty body; returns updated key with state.
    let k: LlmProviderKey = client
        .post(
            &format!("/api/environments/{env_id}/llm_analytics/provider_keys/{id}/validate/"),
            &json!({}),
        )
        .await
        .map_err(|e| BosshoggError::BadRequest(format!("validate failed: {e}")))?;

    if cx.json_mode {
        output::print_json(&k);
    } else {
        println!(
            "Provider key {id}: state = {}",
            k.state.as_deref().unwrap_or("unknown")
        );
        if let Some(err) = k.error_message.as_deref() {
            if !err.is_empty() {
                println!("Error: {err}");
            }
        }
    }
    Ok(())
}

// ── review-queue ──────────────────────────────────────────────────────────────

async fn dispatch_review_queue(cx: &CommandContext, cmd: ReviewQueueCommand) -> Result<()> {
    match cmd {
        ReviewQueueCommand::List { limit } => list_review_queue(cx, limit).await,
    }
}

#[derive(Serialize)]
struct ReviewQueueListOutput {
    count: usize,
    results: Vec<ReviewQueueItem>,
}

async fn list_review_queue(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/llm_analytics/review_queue_items/");
    let results: Vec<ReviewQueueItem> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ReviewQueueListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "TRACE_ID", "STATUS", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|item| {
                vec![
                    item.id.clone(),
                    item.trace_id.clone().unwrap_or_else(|| "-".into()),
                    item.status.clone().unwrap_or_else(|| "-".into()),
                    item.created_at.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_report_roundtrip_minimal() {
        let raw = r#"{"id":"er-1"}"#;
        let r: EvaluationReport = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "er-1");
        assert!(r.evaluation.is_none());
        assert!(r.frequency.is_none());
    }

    #[test]
    fn evaluation_report_roundtrip_full() {
        let raw = r#"{
            "id": "er-full",
            "evaluation": "ev-uuid-123",
            "frequency": "scheduled",
            "enabled": true,
            "deleted": false,
            "next_delivery_date": "2026-05-01T08:00:00Z",
            "last_delivered_at": "2026-04-01T08:00:00Z",
            "rrule": "FREQ=WEEKLY;BYDAY=MO",
            "timezone_name": "America/New_York",
            "delivery_targets": [{"type": "email", "value": "team@example.com"}],
            "max_sample_size": 100
        }"#;
        let r: EvaluationReport = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "er-full");
        assert_eq!(r.evaluation.as_deref(), Some("ev-uuid-123"));
        assert_eq!(r.frequency.as_deref(), Some("scheduled"));
        assert_eq!(r.enabled, Some(true));
        assert_eq!(r.max_sample_size, Some(100));
    }

    #[test]
    fn llm_provider_key_roundtrip() {
        let raw = r#"{
            "id": "pk-1",
            "provider": "openai",
            "name": "My OpenAI Key",
            "state": "ok",
            "error_message": null,
            "api_key_masked": "sk-****1234",
            "created_at": "2026-04-01T00:00:00Z",
            "last_used_at": null,
            "created_by": {"id": 1, "email": "admin@example.com"}
        }"#;
        let k: LlmProviderKey = serde_json::from_str(raw).unwrap();
        assert_eq!(k.id, "pk-1");
        assert_eq!(k.provider.as_deref(), Some("openai"));
        assert_eq!(k.state.as_deref(), Some("ok"));
        assert_eq!(k.api_key_masked.as_deref(), Some("sk-****1234"));
    }

    #[test]
    fn review_queue_item_roundtrip() {
        let raw = r#"{
            "id": "rqi-1",
            "trace_id": "trace-abc-123",
            "created_at": "2026-04-01T00:00:00Z",
            "status": "pending"
        }"#;
        let item: ReviewQueueItem = serde_json::from_str(raw).unwrap();
        assert_eq!(item.id, "rqi-1");
        assert_eq!(item.trace_id.as_deref(), Some("trace-abc-123"));
        assert_eq!(item.status.as_deref(), Some("pending"));
    }
}
