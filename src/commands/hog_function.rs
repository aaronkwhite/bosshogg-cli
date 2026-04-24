// src/commands/hog_function.rs
//! `bosshogg hog-function` — list / get / create / update / delete / enable /
//! disable / invoke / logs / metrics / enable-backfills.
//!
//! Hog functions are environment-scoped.
//! `hog_functions` IS in SOFT_DELETE_RESOURCES — `client.delete()` rewrites to
//! PATCH {"deleted": true}.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::commands::context::CommandContext;
use crate::commands::util::{env_id_required, read_json_file, read_text_file};
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HogFunction {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type", default)]
    pub fn_type: Option<String>,
    #[serde(default)]
    pub template_id: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub hog: Option<String>,
    #[serde(default)]
    pub inputs: Option<Value>,
    #[serde(default)]
    pub inputs_schema: Option<Value>,
    #[serde(default)]
    pub filters: Option<Value>,
    #[serde(default)]
    pub mappings: Option<Value>,
    #[serde(default)]
    pub masking: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct HogFunctionArgs {
    #[command(subcommand)]
    pub command: HogFunctionCommand,
}

#[derive(Subcommand, Debug)]
pub enum HogFunctionCommand {
    /// List hog functions with optional filters.
    List {
        /// Filter by function type (destination, transformation, etc.).
        #[arg(long = "type")]
        fn_type: Option<String>,
        /// Show only enabled functions.
        #[arg(long, conflicts_with = "disabled")]
        enabled: bool,
        /// Show only disabled functions.
        #[arg(long, conflicts_with = "enabled")]
        disabled: bool,
        /// Search by name.
        #[arg(long)]
        search: Option<String>,
    },
    /// Get a single hog function by UUID.
    Get { id: String },
    /// Create a new hog function from a template.
    Create {
        #[arg(long)]
        name: String,
        /// Template ID to base the function on.
        #[arg(long)]
        template_id: String,
        /// Path to a JSON file containing the inputs object.
        #[arg(long)]
        inputs_file: Option<PathBuf>,
    },
    /// Update a hog function's fields.
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        /// Path to a JSON file containing the updated inputs object.
        #[arg(long)]
        inputs_file: Option<PathBuf>,
        /// Path to a .hog file containing the updated Hog source.
        #[arg(long)]
        hog_file: Option<PathBuf>,
    },
    /// Soft-delete a hog function.
    Delete { id: String },
    /// Enable a hog function.
    Enable { id: String },
    /// Disable a hog function.
    Disable { id: String },
    /// Invoke a hog function with a sample event (POST /invocations/).
    Invoke {
        id: String,
        /// Path to a JSON file containing the sample event.
        #[arg(long)]
        event_file: PathBuf,
    },
    /// Fetch execution logs for a hog function.
    Logs {
        id: String,
        /// Filter logs after this timestamp (ISO 8601).
        #[arg(long)]
        after: Option<String>,
        /// Filter logs before this timestamp (ISO 8601).
        #[arg(long)]
        before: Option<String>,
    },
    /// Fetch metrics for a hog function.
    Metrics { id: String },
    /// Enable backfills for a hog function.
    #[command(name = "enable-backfills")]
    EnableBackfills { id: String },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: HogFunctionArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        HogFunctionCommand::List {
            fn_type,
            enabled,
            disabled,
            search,
        } => list_hog_functions(cx, fn_type, enabled, disabled, search).await,
        HogFunctionCommand::Get { id } => get_hog_function(cx, id).await,
        HogFunctionCommand::Create {
            name,
            template_id,
            inputs_file,
        } => create_hog_function(cx, name, template_id, inputs_file).await,
        HogFunctionCommand::Update {
            id,
            name,
            inputs_file,
            hog_file,
        } => update_hog_function(cx, id, name, inputs_file, hog_file).await,
        HogFunctionCommand::Delete { id } => delete_hog_function(cx, id).await,
        HogFunctionCommand::Enable { id } => enable_hog_function(cx, id).await,
        HogFunctionCommand::Disable { id } => disable_hog_function(cx, id).await,
        HogFunctionCommand::Invoke { id, event_file } => {
            invoke_hog_function(cx, id, event_file).await
        }
        HogFunctionCommand::Logs { id, after, before } => {
            logs_hog_function(cx, id, after, before).await
        }
        HogFunctionCommand::Metrics { id } => metrics_hog_function(cx, id).await,
        HogFunctionCommand::EnableBackfills { id } => enable_backfills(cx, id).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<HogFunction>,
}

async fn list_hog_functions(
    cx: &CommandContext,
    fn_type: Option<String>,
    enabled: bool,
    disabled: bool,
    search: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut qs: Vec<(String, String)> = Vec::new();
    if let Some(t) = fn_type {
        qs.push(("type".into(), t));
    }
    if enabled {
        qs.push(("enabled".into(), "true".into()));
    }
    if disabled {
        qs.push(("enabled".into(), "false".into()));
    }
    if let Some(s) = search {
        qs.push(("search".into(), s));
    }

    let query = if qs.is_empty() {
        String::new()
    } else {
        let joined = qs
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("?{joined}")
    };

    let path = format!("/api/environments/{env_id}/hog_functions/{query}");
    let results: Vec<HogFunction> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "TYPE", "ENABLED"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|f| {
                vec![
                    f.id.clone(),
                    f.name.clone(),
                    f.fn_type.clone().unwrap_or_else(|| "-".into()),
                    f.enabled
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_hog_function(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let f: HogFunction = client
        .get(&format!("/api/environments/{env_id}/hog_functions/{id}/"))
        .await?;
    if cx.json_mode {
        output::print_json(&f);
    } else {
        println!("ID:          {}", f.id);
        println!("Name:        {}", f.name);
        if let Some(t) = f.fn_type.as_deref() {
            println!("Type:        {t}");
        }
        if let Some(tid) = f.template_id.as_deref() {
            println!("Template:    {tid}");
        }
        println!(
            "Enabled:     {}",
            f.enabled
                .map(|e| e.to_string())
                .unwrap_or_else(|| "-".into())
        );
        if let Some(ca) = f.created_at.as_deref() {
            println!("Created:     {ca}");
        }
    }
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_hog_function(
    cx: &CommandContext,
    name: String,
    template_id: String,
    inputs_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = json!({
        "name": name,
        "template_id": template_id,
    });

    if let Some(p) = inputs_file.as_deref() {
        body["inputs"] = read_json_file(p).await?;
    }

    let created: HogFunction = client
        .post(&format!("/api/environments/{env_id}/hog_functions/"), &body)
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
            "Created hog function '{}' (id {})",
            created.name, created.id
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_hog_function(
    cx: &CommandContext,
    id: String,
    name: Option<String>,
    inputs_file: Option<PathBuf>,
    hog_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(p) = inputs_file.as_deref() {
        body.insert("inputs".into(), read_json_file(p).await?);
    }
    if let Some(p) = hog_file.as_deref() {
        let hog_src = read_text_file(p).await?;
        body.insert("hog".into(), Value::String(hog_src));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --inputs-file, --hog-file)".into(),
        ));
    }

    cx.confirm(&format!("update hog function `{id}`; continue?"))?;

    let updated: HogFunction = client
        .patch(
            &format!("/api/environments/{env_id}/hog_functions/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated hog function '{}' (id {})",
            updated.name, updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_hog_function(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("soft-delete hog function `{id}`; continue?"))?;

    client
        .delete(&format!("/api/environments/{env_id}/hog_functions/{id}/"))
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
        println!("Deleted hog function {id}");
    }
    Ok(())
}

// ── enable ────────────────────────────────────────────────────────────────────

async fn enable_hog_function(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("enable hog function `{id}`; continue?"))?;

    let updated: HogFunction = client
        .patch(
            &format!("/api/environments/{env_id}/hog_functions/{id}/"),
            &json!({ "enabled": true }),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Enabled hog function '{}'", updated.name);
    }
    Ok(())
}

// ── disable ───────────────────────────────────────────────────────────────────

async fn disable_hog_function(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("disable hog function `{id}`; continue?"))?;

    let updated: HogFunction = client
        .patch(
            &format!("/api/environments/{env_id}/hog_functions/{id}/"),
            &json!({ "enabled": false }),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Disabled hog function '{}'", updated.name);
    }
    Ok(())
}

// ── invoke ────────────────────────────────────────────────────────────────────

async fn invoke_hog_function(
    cx: &CommandContext,
    id: String,
    event_file: PathBuf,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "invoke hog function `{id}` with sample event; continue?"
    ))?;

    let event = read_json_file(&event_file).await?;
    let body = json!({ "event": event });

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/hog_functions/{id}/invocations/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Invoked hog function {id}");
        if let Some(status) = v.get("status").and_then(Value::as_str) {
            println!("Status: {status}");
        }
    }
    Ok(())
}

// ── logs ──────────────────────────────────────────────────────────────────────

async fn logs_hog_function(
    cx: &CommandContext,
    id: String,
    after: Option<String>,
    before: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut qs: Vec<(String, String)> = Vec::new();
    if let Some(a) = after {
        qs.push(("after".into(), a));
    }
    if let Some(b) = before {
        qs.push(("before".into(), b));
    }

    let query = if qs.is_empty() {
        String::new()
    } else {
        let joined = qs
            .iter()
            .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");
        format!("?{joined}")
    };

    let path = format!("/api/environments/{env_id}/hog_functions/{id}/logs/{query}");
    let v: Value = client.get(&path).await?;

    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(results) = v.get("results").and_then(Value::as_array) {
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
    Ok(())
}

// ── metrics ───────────────────────────────────────────────────────────────────

async fn metrics_hog_function(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/hog_functions/{id}/metrics/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{v}");
    }
    Ok(())
}

// ── enable-backfills ──────────────────────────────────────────────────────────

async fn enable_backfills(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "enable backfills for hog function `{id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/hog_functions/{id}/enable_backfills/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Enabled backfills for hog function {id}");
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hog_function_roundtrip_minimal() {
        let raw = r#"{
            "id": "abc-123",
            "name": "My Function",
            "type": "destination",
            "enabled": true,
            "deleted": false
        }"#;
        let f: HogFunction = serde_json::from_str(raw).unwrap();
        assert_eq!(f.id, "abc-123");
        assert_eq!(f.name, "My Function");
        assert_eq!(f.fn_type, Some("destination".to_string()));
        assert_eq!(f.enabled, Some(true));
    }

    #[test]
    fn hog_function_roundtrip_full() {
        let raw = r#"{
            "id": "def-456",
            "name": "Transform Events",
            "description": "Transforms events before sending",
            "type": "transformation",
            "template_id": "template-abc",
            "enabled": false,
            "deleted": false,
            "hog": "return event",
            "inputs": {"key": "value"},
            "inputs_schema": [],
            "filters": null,
            "mappings": null,
            "masking": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-04-01T00:00:00Z",
            "created_by": {"id": 1, "email": "test@example.com"}
        }"#;
        let f: HogFunction = serde_json::from_str(raw).unwrap();
        assert_eq!(f.id, "def-456");
        assert_eq!(f.fn_type, Some("transformation".to_string()));
        assert_eq!(f.template_id, Some("template-abc".to_string()));
        assert_eq!(f.enabled, Some(false));
    }

    #[test]
    fn hog_function_type_field_serializes_correctly() {
        let f = HogFunction {
            id: "xyz".into(),
            name: "Test".into(),
            description: None,
            fn_type: Some("destination".into()),
            template_id: None,
            enabled: Some(true),
            deleted: None,
            hog: None,
            inputs: None,
            inputs_schema: None,
            filters: None,
            mappings: None,
            masking: None,
            created_at: None,
            updated_at: None,
            created_by: None,
        };
        let v = serde_json::to_value(&f).unwrap();
        // The field should serialize as "type" (the serde rename)
        assert_eq!(v.get("type").and_then(Value::as_str), Some("destination"));
        // Not as fn_type
        assert!(v.get("fn_type").is_none());
    }
}
