// src/commands/insight_variable.rs
//! `bosshogg insight-variable` — list / get / create / update / delete.
//!
//! Insight (HogQL) variables are project-scoped.
//! Path: `/api/projects/{project_id}/insight_variables/`
//!
//! DELETE returns 204 (hard delete).

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct InsightVariable {
    pub id: String, // UUID
    pub name: String,
    #[serde(rename = "type")]
    pub variable_type: String, // String, Number, Boolean, List, Date
    #[serde(default)]
    pub default_value: Option<Value>,
    #[serde(default)]
    pub code_name: Option<String>,
    #[serde(default)]
    pub values: Option<Value>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct InsightVariableArgs {
    #[command(subcommand)]
    pub command: InsightVariableCommand,
}

#[derive(Subcommand, Debug)]
pub enum InsightVariableCommand {
    /// List HogQL insight variables.
    List {
        /// Maximum number of results to return.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single insight variable by UUID.
    Get { id: String },
    /// Create a new HogQL insight variable.
    Create {
        /// Variable name.
        #[arg(long)]
        name: String,
        /// Variable type: String, Number, Boolean, List, or Date.
        #[arg(long)]
        r#type: String,
        /// Default value (as a JSON-encoded string, e.g. '"hello"' or '42').
        #[arg(long)]
        default: Option<String>,
    },
    /// Update an existing insight variable.
    Update {
        id: String,
        /// New name.
        #[arg(long)]
        name: Option<String>,
        /// New default value (JSON-encoded string).
        #[arg(long)]
        default: Option<String>,
    },
    /// Hard-delete an insight variable (DELETE HTTP verb — returns 204).
    Delete { id: String },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: InsightVariableArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        InsightVariableCommand::List { limit } => list_variables(cx, limit).await,
        InsightVariableCommand::Get { id } => get_variable(cx, id).await,
        InsightVariableCommand::Create {
            name,
            r#type,
            default,
        } => create_variable(cx, name, r#type, default).await,
        InsightVariableCommand::Update { id, name, default } => {
            update_variable(cx, id, name, default).await
        }
        InsightVariableCommand::Delete { id } => delete_variable(cx, id).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn project_id_required(client: &Client) -> Result<&str> {
    client.project_id().ok_or_else(|| {
        BosshoggError::Config("no project_id configured; run `bosshogg configure`".into())
    })
}

/// Parse a `--default` flag value (JSON-encoded string) into a `serde_json::Value`.
fn parse_default(raw: &str) -> Result<Value> {
    serde_json::from_str(raw)
        .map_err(|e| BosshoggError::BadRequest(format!("--default must be valid JSON: {e}")))
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<InsightVariable>,
}

async fn list_variables(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path = format!("/api/projects/{project_id}/insight_variables/");
    let results: Vec<InsightVariable> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "TYPE", "CODE_NAME", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|v| {
                vec![
                    v.id.clone(),
                    v.name.clone(),
                    v.variable_type.clone(),
                    v.code_name.clone().unwrap_or_else(|| "-".into()),
                    v.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_variable(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let var: InsightVariable = client
        .get(&format!(
            "/api/projects/{project_id}/insight_variables/{id}/"
        ))
        .await?;
    print_variable(&var, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_variable(
    cx: &CommandContext,
    name: String,
    variable_type: String,
    default: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    body.insert("name".into(), Value::String(name));
    body.insert("type".into(), Value::String(variable_type));
    if let Some(raw) = default {
        body.insert("default_value".into(), parse_default(&raw)?);
    }

    cx.confirm("create insight variable; continue?")?;

    let created: InsightVariable = client
        .post(
            &format!("/api/projects/{project_id}/insight_variables/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: String,
            name: String,
            #[serde(rename = "type")]
            variable_type: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            name: created.name,
            variable_type: created.variable_type,
        });
    } else {
        println!(
            "Created insight variable '{}' (id {}, type {})",
            created.name, created.id, created.variable_type
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_variable(
    cx: &CommandContext,
    id: String,
    name: Option<String>,
    default: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(raw) = default {
        body.insert("default_value".into(), parse_default(&raw)?);
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --default)".into(),
        ));
    }

    cx.confirm(&format!("update insight variable `{id}`; continue?"))?;

    let updated: InsightVariable = client
        .patch(
            &format!("/api/projects/{project_id}/insight_variables/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated insight variable '{}' (id {})",
            updated.name, updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_variable(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("hard-delete insight variable `{id}`; continue?"))?;

    client
        .delete(&format!(
            "/api/projects/{project_id}/insight_variables/{id}/"
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
        println!("Deleted insight variable {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_variable(v: &InsightVariable, json_mode: bool) {
    if json_mode {
        output::print_json(v);
    } else {
        println!("ID:           {}", v.id);
        println!("Name:         {}", v.name);
        println!("Type:         {}", v.variable_type);
        if let Some(cn) = v.code_name.as_deref() {
            println!("Code name:    {cn}");
        }
        if let Some(dv) = &v.default_value {
            println!(
                "Default:      {}",
                serde_json::to_string(dv).unwrap_or_default()
            );
        }
        if let Some(ca) = v.created_at.as_deref() {
            println!("Created:      {ca}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insight_variable_roundtrip_minimal() {
        let raw = r#"{
            "id": "iv-uuid-1",
            "name": "date_range",
            "type": "Date",
            "code_name": "date_range",
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let v: InsightVariable = serde_json::from_str(raw).unwrap();
        assert_eq!(v.id, "iv-uuid-1");
        assert_eq!(v.name, "date_range");
        assert_eq!(v.variable_type, "Date");
    }

    #[test]
    fn insight_variable_roundtrip_with_default() {
        let raw = r#"{
            "id": "iv-uuid-2",
            "name": "limit",
            "type": "Number",
            "default_value": 100,
            "code_name": "limit",
            "values": null,
            "created_by": 5,
            "created_at": "2026-02-01T00:00:00Z"
        }"#;
        let v: InsightVariable = serde_json::from_str(raw).unwrap();
        assert_eq!(v.variable_type, "Number");
        assert_eq!(v.default_value.as_ref().and_then(|v| v.as_i64()), Some(100));
    }

    #[test]
    fn insight_variable_type_variants() {
        for vtype in &["String", "Number", "Boolean", "List", "Date"] {
            let raw = format!(r#"{{"id": "x", "name": "v", "type": "{vtype}", "code_name": "v"}}"#);
            let v: InsightVariable = serde_json::from_str(&raw).unwrap();
            assert_eq!(&v.variable_type, vtype);
        }
    }

    #[test]
    fn parse_default_valid_json() {
        assert!(parse_default("42").is_ok());
        assert!(parse_default(r#""hello""#).is_ok());
        assert!(parse_default("true").is_ok());
        assert!(parse_default(r#"["a","b"]"#).is_ok());
    }

    #[test]
    fn parse_default_invalid_json() {
        let err = parse_default("{not json").unwrap_err();
        assert!(matches!(err, BosshoggError::BadRequest(_)));
    }
}
