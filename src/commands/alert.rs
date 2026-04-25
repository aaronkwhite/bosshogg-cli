// src/commands/alert.rs
//! `bosshogg alert` — list / get / create / update / delete.
//!
//! Alerts are project-scoped insight-threshold monitors.
//! Path: `/api/projects/{project_id}/alerts/`
//!
//! DELETE returns 204 (hard delete confirmed via live probe).

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::read_json_file;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Alert {
    pub id: String, // UUID
    #[serde(default)]
    pub name: Option<String>,
    /// Insight ID monitored by this alert (response returns full object).
    #[serde(default)]
    pub insight: Option<Value>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub state: Option<String>,
    #[serde(default)]
    pub threshold: Option<Value>,
    #[serde(default)]
    pub condition: Option<Value>,
    #[serde(default)]
    pub subscribed_users: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub last_notified_at: Option<String>,
    #[serde(default)]
    pub last_checked_at: Option<String>,
    #[serde(default)]
    pub next_check_at: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct AlertArgs {
    #[command(subcommand)]
    pub command: AlertCommand,
}

#[derive(Subcommand, Debug)]
pub enum AlertCommand {
    /// List insight alerts.
    List {
        /// Maximum number of results to return.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single alert by UUID.
    Get { id: String },
    /// Create a new insight alert.
    Create {
        /// Human-readable name for the alert.
        #[arg(long)]
        name: String,
        /// Numeric insight ID to monitor.
        #[arg(long)]
        insight: i64,
        /// Path to a JSON file with threshold/condition config.
        #[arg(long)]
        config_file: Option<PathBuf>,
    },
    /// Update an existing alert.
    Update {
        id: String,
        /// New name.
        #[arg(long)]
        name: Option<String>,
        /// Path to a JSON file with updated threshold/condition config.
        #[arg(long)]
        config_file: Option<PathBuf>,
        /// Enable or disable the alert.
        #[arg(long)]
        enabled: Option<bool>,
    },
    /// Hard-delete an alert (DELETE HTTP verb — returns 204).
    Delete { id: String },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: AlertArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        AlertCommand::List { limit } => list_alerts(cx, limit).await,
        AlertCommand::Get { id } => get_alert(cx, id).await,
        AlertCommand::Create {
            name,
            insight,
            config_file,
        } => create_alert(cx, name, insight, config_file).await,
        AlertCommand::Update {
            id,
            name,
            config_file,
            enabled,
        } => update_alert(cx, id, name, config_file, enabled).await,
        AlertCommand::Delete { id } => delete_alert(cx, id).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn project_id_required(client: &Client) -> Result<&str> {
    client.project_id().ok_or_else(|| {
        BosshoggError::Config("no project_id configured; run `bosshogg configure`".into())
    })
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Alert>,
}

async fn list_alerts(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path = format!("/api/projects/{project_id}/alerts/");
    let results: Vec<Alert> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "STATE", "ENABLED", "LAST_CHECKED"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|a| {
                vec![
                    a.id.clone(),
                    a.name.clone().unwrap_or_else(|| "-".into()),
                    a.state.clone().unwrap_or_else(|| "-".into()),
                    a.enabled
                        .map(|e| if e { "true" } else { "false" })
                        .unwrap_or("-")
                        .to_string(),
                    a.last_checked_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_alert(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let alert: Alert = client
        .get(&format!("/api/projects/{project_id}/alerts/{id}/"))
        .await?;
    print_alert(&alert, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_alert(
    cx: &CommandContext,
    name: String,
    insight: i64,
    config_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    body.insert("name".into(), Value::String(name));
    body.insert("insight".into(), Value::Number(insight.into()));

    if let Some(path) = config_file {
        let config = read_json_file(&path).await?;
        // Merge top-level keys from config into body (threshold, condition, subscribed_users, etc.)
        if let Some(obj) = config.as_object() {
            for (k, v) in obj {
                body.insert(k.clone(), v.clone());
            }
        }
    }

    cx.confirm(&format!("create alert '{}'; continue?", {
        body.get("name").and_then(|v| v.as_str()).unwrap_or("alert")
    }))?;

    let created: Alert = client
        .post(
            &format!("/api/projects/{project_id}/alerts/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: String,
            name: Option<String>,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            name: created.name,
        });
    } else {
        println!(
            "Created alert '{}' (id {})",
            created.name.as_deref().unwrap_or("-"),
            created.id
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_alert(
    cx: &CommandContext,
    id: String,
    name: Option<String>,
    config_file: Option<PathBuf>,
    enabled: Option<bool>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();

    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(e) = enabled {
        body.insert("enabled".into(), Value::Bool(e));
    }
    if let Some(path) = config_file {
        let config = read_json_file(&path).await?;
        if let Some(obj) = config.as_object() {
            for (k, v) in obj {
                body.insert(k.clone(), v.clone());
            }
        }
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --enabled, --config-file)".into(),
        ));
    }

    cx.confirm(&format!("update alert `{id}`; continue?"))?;

    let updated: Alert = client
        .patch(
            &format!("/api/projects/{project_id}/alerts/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated alert '{}' (id {})",
            updated.name.as_deref().unwrap_or("-"),
            updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_alert(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("hard-delete alert `{id}`; continue?"))?;

    client
        .delete(&format!("/api/projects/{project_id}/alerts/{id}/"))
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
        println!("Deleted alert {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_alert(alert: &Alert, json_mode: bool) {
    if json_mode {
        output::print_json(alert);
    } else {
        println!("ID:            {}", alert.id);
        if let Some(n) = alert.name.as_deref() {
            println!("Name:          {n}");
        }
        if let Some(s) = alert.state.as_deref() {
            println!("State:         {s}");
        }
        if let Some(e) = alert.enabled {
            println!("Enabled:       {e}");
        }
        if let Some(ca) = alert.created_at.as_deref() {
            println!("Created:       {ca}");
        }
        if let Some(lc) = alert.last_checked_at.as_deref() {
            println!("Last checked:  {lc}");
        }
        if let Some(nc) = alert.next_check_at.as_deref() {
            println!("Next check:    {nc}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alert_roundtrip_minimal() {
        let raw = r#"{
            "id": "alert-uuid-1",
            "name": "My Alert",
            "enabled": true,
            "state": "Not firing"
        }"#;
        let a: Alert = serde_json::from_str(raw).unwrap();
        assert_eq!(a.id, "alert-uuid-1");
        assert_eq!(a.name.as_deref(), Some("My Alert"));
        assert_eq!(a.enabled, Some(true));
    }

    #[test]
    fn alert_roundtrip_full() {
        let raw = r#"{
            "id": "alert-uuid-2",
            "name": "Conversion Drop",
            "enabled": false,
            "state": "Firing",
            "threshold": {"type": "absolute", "bounds": {"lower": 0.0}},
            "created_at": "2026-01-01T00:00:00Z",
            "last_checked_at": "2026-04-01T12:00:00Z",
            "next_check_at": "2026-04-01T13:00:00Z"
        }"#;
        let a: Alert = serde_json::from_str(raw).unwrap();
        assert_eq!(a.id, "alert-uuid-2");
        assert_eq!(a.state.as_deref(), Some("Firing"));
        assert!(a.threshold.is_some());
    }

    #[test]
    fn alert_state_variants() {
        for state in &["Firing", "Not firing", "Errored", "Snoozed"] {
            let raw = format!(r#"{{"id": "x", "state": "{state}"}}"#);
            let a: Alert = serde_json::from_str(&raw).unwrap();
            assert_eq!(a.state.as_deref(), Some(*state));
        }
    }
}
