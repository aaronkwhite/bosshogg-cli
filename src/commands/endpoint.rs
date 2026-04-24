// src/commands/endpoint.rs
//! `bosshogg endpoint` — list / get / create / update / delete / run /
//!   materialize-preview / materialize-status / openapi.
//!
//! Endpoints (HogQL saved queries) are environment-scoped. Addressed by name string.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::commands::context::CommandContext;
use crate::commands::util::{env_id_required, read_text_file};
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Endpoint {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub query: serde_json::Value,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<serde_json::Value>,
    #[serde(default)]
    pub is_materialized: Option<bool>,
    #[serde(default)]
    pub last_materialized_at: Option<String>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct EndpointArgs {
    #[command(subcommand)]
    pub command: EndpointCommand,
}

#[derive(Subcommand, Debug)]
pub enum EndpointCommand {
    /// List all saved endpoints.
    List,
    /// Get a saved endpoint by name.
    Get { name: String },
    /// Create a new saved endpoint.
    Create {
        /// Endpoint name (used as the URL slug).
        #[arg(long)]
        name: String,
        /// Path to a SQL file containing the HogQL query.
        #[arg(long)]
        query_file: PathBuf,
        #[arg(long)]
        description: Option<String>,
    },
    /// Update an existing saved endpoint.
    Update {
        name: String,
        /// Path to a SQL file with the updated HogQL query.
        #[arg(long)]
        query_file: Option<PathBuf>,
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a saved endpoint.
    Delete { name: String },
    /// Execute the saved query and return results.
    Run { name: String },
    /// Preview what materialization would do for this endpoint.
    #[command(name = "materialize-preview")]
    MaterializePreview { name: String },
    /// Get the current materialization status of an endpoint.
    #[command(name = "materialize-status")]
    MaterializeStatus { name: String },
    /// Get the OpenAPI spec for an endpoint.
    Openapi { name: String },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: EndpointArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        EndpointCommand::List => list_endpoints(cx).await,
        EndpointCommand::Get { name } => get_endpoint(cx, &name).await,
        EndpointCommand::Create {
            name,
            query_file,
            description,
        } => create_endpoint(cx, name, query_file, description).await,
        EndpointCommand::Update {
            name,
            query_file,
            description,
        } => update_endpoint(cx, name, query_file, description).await,
        EndpointCommand::Delete { name } => delete_endpoint(cx, &name).await,
        EndpointCommand::Run { name } => run_endpoint(cx, &name).await,
        EndpointCommand::MaterializePreview { name } => materialize_preview(cx, &name).await,
        EndpointCommand::MaterializeStatus { name } => materialize_status(cx, &name).await,
        EndpointCommand::Openapi { name } => openapi_endpoint(cx, &name).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Endpoint>,
}

async fn list_endpoints(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/endpoints/");
    let results: Vec<Endpoint> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["NAME", "MATERIALIZED", "CREATED_AT", "DESCRIPTION"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|e| {
                vec![
                    e.name.clone(),
                    e.is_materialized
                        .map(|v| if v { "yes" } else { "no" })
                        .unwrap_or("-")
                        .to_string(),
                    e.created_at.clone().unwrap_or_default(),
                    e.description.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_endpoint(cx: &CommandContext, name: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let ep: Endpoint = client
        .get(&format!("/api/environments/{env_id}/endpoints/{name}/"))
        .await?;
    print_endpoint(&ep, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_endpoint(
    cx: &CommandContext,
    name: String,
    query_file: PathBuf,
    description: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let sql = read_text_file(&query_file).await?;

    let mut body = json!({
        "name": name,
        "query": { "kind": "HogQLQuery", "query": sql }
    });

    if let Some(d) = description {
        body["description"] = json!(d);
    }

    let created: Endpoint = client
        .post(&format!("/api/environments/{env_id}/endpoints/"), &body)
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            name: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            name: created.name,
        });
    } else {
        println!("Created endpoint '{}'", created.name);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_endpoint(
    cx: &CommandContext,
    name: String,
    query_file: Option<PathBuf>,
    description: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(p) = query_file {
        let sql = read_text_file(&p).await?;
        body.insert(
            "query".into(),
            json!({ "kind": "HogQLQuery", "query": sql }),
        );
    }
    if let Some(d) = description {
        body.insert("description".into(), Value::String(d));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --query-file, --description)".into(),
        ));
    }

    cx.confirm(&format!("update endpoint `{name}`; continue?"))?;

    let updated: Endpoint = client
        .patch(
            &format!("/api/environments/{env_id}/endpoints/{name}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated endpoint '{}'", updated.name);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_endpoint(cx: &CommandContext, name: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("delete endpoint `{name}`; continue?"))?;

    client
        .delete(&format!("/api/environments/{env_id}/endpoints/{name}/"))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            name: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "delete",
            name: name.to_string(),
        });
    } else {
        println!("Deleted endpoint '{name}'");
    }
    Ok(())
}

// ── run ───────────────────────────────────────────────────────────────────────

async fn run_endpoint(cx: &CommandContext, name: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!("/api/environments/{env_id}/endpoints/{name}/run/"))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── materialize-preview ───────────────────────────────────────────────────────

async fn materialize_preview(cx: &CommandContext, name: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let body = json!({});
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/endpoints/{name}/materialization_preview/"),
            &body,
        )
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── materialize-status ────────────────────────────────────────────────────────

async fn materialize_status(cx: &CommandContext, name: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/endpoints/{name}/materialization_status/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── openapi ───────────────────────────────────────────────────────────────────

async fn openapi_endpoint(cx: &CommandContext, name: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/endpoints/{name}/openapi.json/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_endpoint(ep: &Endpoint, json_mode: bool) {
    if json_mode {
        output::print_json(ep);
    } else {
        println!("Name:          {}", ep.name);
        if let Some(d) = ep.description.as_deref() {
            println!("Description:   {d}");
        }
        println!(
            "Materialized:  {}",
            ep.is_materialized
                .map(|v| if v { "yes" } else { "no" })
                .unwrap_or("-")
        );
        if let Some(ca) = ep.created_at.as_deref() {
            println!("Created:       {ca}");
        }
        if let Some(ua) = ep.updated_at.as_deref() {
            println!("Updated:       {ua}");
        }
        println!(
            "Query:         {}",
            serde_json::to_string(&ep.query).unwrap_or_default()
        );
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint_json() -> &'static str {
        r#"{
            "name": "daily-signups",
            "description": "Daily signups query",
            "query": {"kind": "HogQLQuery", "query": "SELECT count() FROM events"},
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-04-01T00:00:00Z",
            "is_materialized": false
        }"#
    }

    #[test]
    fn endpoint_roundtrip_minimal() {
        let raw = r#"{
            "name": "my-endpoint",
            "query": {"kind": "HogQLQuery", "query": "SELECT 1"}
        }"#;
        let e: Endpoint = serde_json::from_str(raw).unwrap();
        assert_eq!(e.name, "my-endpoint");
        assert!(e.description.is_none());
        assert!(e.is_materialized.is_none());
    }

    #[test]
    fn endpoint_roundtrip_full() {
        let e: Endpoint = serde_json::from_str(endpoint_json()).unwrap();
        assert_eq!(e.name, "daily-signups");
        assert_eq!(e.is_materialized, Some(false));
        assert_eq!(e.description, Some("Daily signups query".into()));
    }

    #[test]
    fn endpoint_query_is_fluid_value() {
        let raw = r#"{
            "name": "flexible",
            "query": {"kind": "HogQLQuery", "query": "SELECT event FROM events", "extra_field": 42}
        }"#;
        let e: Endpoint = serde_json::from_str(raw).unwrap();
        assert_eq!(e.query["extra_field"], serde_json::json!(42));
    }

    #[test]
    fn endpoint_serialize_roundtrip() {
        let e: Endpoint = serde_json::from_str(endpoint_json()).unwrap();
        let s = serde_json::to_string(&e).unwrap();
        let e2: Endpoint = serde_json::from_str(&s).unwrap();
        assert_eq!(e.name, e2.name);
        assert_eq!(e.is_materialized, e2.is_materialized);
    }

    #[test]
    fn endpoint_optional_timestamps_default() {
        let raw = r#"{"name": "bare", "query": {}}"#;
        let e: Endpoint = serde_json::from_str(raw).unwrap();
        assert!(e.created_at.is_none());
        assert!(e.updated_at.is_none());
        assert!(e.last_materialized_at.is_none());
    }
}
