// src/commands/evaluation.rs
//! `bosshogg evaluation` — list / get / test-hog.
//!
//! Evaluations live under the environments path:
//!   `/api/environments/{project_id}/evaluations/`
//!
//! PostHog quirk: `{project_id}` here resolves from the env_id configured
//! in the current context (same as other `/api/environments/` endpoints).
//!
//! `test-hog` is synchronous — it runs Hog code against a sample of recent
//! `$ai_generation` events and returns results immediately (no polling required).

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::commands::context::CommandContext;
use crate::commands::util::{env_id_required, read_text_file};
use crate::error::Result;
use crate::output;

// ── Typed structs ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Evaluation {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct EvaluationArgs {
    #[command(subcommand)]
    pub command: EvaluationCommand,
}

#[derive(Subcommand, Debug)]
pub enum EvaluationCommand {
    /// List evaluations (paginated).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single evaluation by ID (UUID).
    Get { id: String },
    /// Test Hog evaluation code against recent $ai_generation events (synchronous).
    ///
    /// Runs the provided Hog code against a sample of recent events and returns
    /// pass/fail/N/A results immediately — no polling required.
    ///
    /// HOG CODE CONTRACT: the code must return a boolean (true = pass, false =
    /// fail) or null for N/A. The evaluation context exposes the generation event
    /// at `event` and the trace at `trace`.
    #[command(name = "test-hog")]
    TestHog {
        /// Path to a .hog or .js file containing the Hog evaluation source code.
        #[arg(long)]
        hog_file: PathBuf,
        /// Number of recent $ai_generation events to test against (1–10, default 5).
        #[arg(long, default_value = "5")]
        sample_count: u32,
        /// Allow the evaluation to return N/A for non-applicable generations.
        #[arg(long)]
        allows_na: bool,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: EvaluationArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        EvaluationCommand::List { limit } => list_evaluations(cx, limit).await,
        EvaluationCommand::Get { id } => get_evaluation(cx, id).await,
        EvaluationCommand::TestHog {
            hog_file,
            sample_count,
            allows_na,
        } => test_hog(cx, hog_file, sample_count, allows_na).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct EvaluationsListOutput {
    count: usize,
    results: Vec<Evaluation>,
}

async fn list_evaluations(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/evaluations/");
    let results: Vec<Evaluation> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&EvaluationsListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "ENABLED", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|e| {
                vec![
                    e.id.clone(),
                    e.name.clone().unwrap_or_else(|| "-".into()),
                    e.enabled
                        .map(|b| if b { "yes" } else { "no" }.to_string())
                        .unwrap_or_else(|| "-".into()),
                    e.created_at.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_evaluation(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let e: Evaluation = client
        .get(&format!("/api/environments/{env_id}/evaluations/{id}/"))
        .await?;

    if cx.json_mode {
        output::print_json(&e);
    } else {
        println!("ID:           {}", e.id);
        if let Some(n) = e.name.as_deref() {
            println!("Name:         {n}");
        }
        if let Some(d) = e.description.as_deref() {
            println!("Description:  {d}");
        }
        if let Some(en) = e.enabled {
            println!("Enabled:      {}", if en { "yes" } else { "no" });
        }
        if let Some(ca) = e.created_at.as_deref() {
            println!("Created at:   {ca}");
        }
        if let Some(ua) = e.updated_at.as_deref() {
            println!("Updated at:   {ua}");
        }
    }
    Ok(())
}

// ── test-hog ──────────────────────────────────────────────────────────────────

async fn test_hog(
    cx: &CommandContext,
    hog_file: PathBuf,
    sample_count: u32,
    allows_na: bool,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let source = read_text_file(&hog_file).await?;

    let body = json!({
        "source": source,
        "sample_count": sample_count,
        "allows_na": allows_na,
    });

    let resp: Value = client
        .post(
            &format!("/api/environments/{env_id}/evaluations/test_hog/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&resp);
    } else if let Some(results) = resp.get("results").and_then(Value::as_array) {
        println!("Tested against {} event(s):", results.len());
        for item in results {
            let event_uuid = item
                .get("event_uuid")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let result = match item.get("result") {
                Some(Value::Bool(true)) => "PASS",
                Some(Value::Bool(false)) => "FAIL",
                Some(Value::Null) | None => "N/A",
                _ => "?",
            };
            let err = item.get("error").and_then(Value::as_str).unwrap_or("");
            if err.is_empty() {
                println!("  {event_uuid}  {result}");
            } else {
                println!("  {event_uuid}  {result}  (error: {err})");
            }
        }
        if let Some(msg) = resp.get("message").and_then(Value::as_str) {
            if !msg.is_empty() {
                println!("{msg}");
            }
        }
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(&resp).unwrap_or_default()
        );
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluation_roundtrip_minimal() {
        let raw = r#"{"id":"ev-1"}"#;
        let e: Evaluation = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, "ev-1");
        assert!(e.name.is_none());
        assert!(e.enabled.is_none());
    }

    #[test]
    fn evaluation_roundtrip_full() {
        let raw = r#"{
            "id": "ev-full",
            "name": "My Evaluation",
            "description": "Test quality",
            "enabled": true,
            "created_at": "2026-04-01T00:00:00Z",
            "updated_at": "2026-04-02T00:00:00Z",
            "deleted": false,
            "source": "return true",
            "created_by": {"id": 1, "email": "admin@example.com"}
        }"#;
        let e: Evaluation = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, "ev-full");
        assert_eq!(e.name.as_deref(), Some("My Evaluation"));
        assert_eq!(e.enabled, Some(true));
        assert_eq!(e.source.as_deref(), Some("return true"));
    }
}
