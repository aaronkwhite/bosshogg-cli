// src/commands/dataset.rs
//! `bosshogg dataset` — list / get / create / update / delete.
//!
//! Datasets are project-scoped: `/api/projects/{project_id}/datasets/`.
//! Hard DELETE (not a soft-delete resource).

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed structs ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Dataset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub team: Option<Value>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct DatasetArgs {
    #[command(subcommand)]
    pub command: DatasetCommand,
}

#[derive(Subcommand, Debug)]
pub enum DatasetCommand {
    /// List datasets (paginated).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single dataset by ID (UUID).
    Get { id: String },
    /// Create a new dataset.
    Create {
        /// Dataset name.
        #[arg(long)]
        name: String,
        /// Optional description.
        #[arg(long)]
        description: Option<String>,
    },
    /// Update a dataset.
    Update {
        /// Dataset UUID.
        id: String,
        /// New name.
        #[arg(long)]
        name: Option<String>,
        /// New description.
        #[arg(long)]
        description: Option<String>,
    },
    /// Delete a dataset by ID (hard delete).
    Delete {
        /// Dataset UUID.
        id: String,
    },
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn project_id_required(client: &Client) -> Result<&str> {
    client.project_id().ok_or_else(|| {
        BosshoggError::Config(
            "no project_id configured; run `bosshogg configure` or set POSTHOG_CLI_PROJECT_ID"
                .into(),
        )
    })
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: DatasetArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        DatasetCommand::List { limit } => list_datasets(cx, limit).await,
        DatasetCommand::Get { id } => get_dataset(cx, id).await,
        DatasetCommand::Create { name, description } => create_dataset(cx, name, description).await,
        DatasetCommand::Update {
            id,
            name,
            description,
        } => update_dataset(cx, id, name, description).await,
        DatasetCommand::Delete { id } => delete_dataset(cx, id).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct DatasetsListOutput {
    count: usize,
    results: Vec<Dataset>,
}

async fn list_datasets(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path = format!("/api/projects/{project_id}/datasets/");
    let results: Vec<Dataset> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&DatasetsListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "DESCRIPTION", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|d| {
                vec![
                    d.id.clone(),
                    d.name.clone(),
                    d.description.clone().unwrap_or_else(|| "-".into()),
                    d.created_at.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_dataset(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let d: Dataset = client
        .get(&format!("/api/projects/{project_id}/datasets/{id}/"))
        .await?;

    if cx.json_mode {
        output::print_json(&d);
    } else {
        println!("ID:           {}", d.id);
        println!("Name:         {}", d.name);
        if let Some(desc) = d.description.as_deref() {
            println!("Description:  {desc}");
        }
        if let Some(ca) = d.created_at.as_deref() {
            println!("Created at:   {ca}");
        }
        if let Some(ua) = d.updated_at.as_deref() {
            println!("Updated at:   {ua}");
        }
    }
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_dataset(
    cx: &CommandContext,
    name: String,
    description: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm("create dataset; continue?")?;

    let mut body = serde_json::Map::new();
    body.insert("name".into(), json!(name));
    if let Some(desc) = description {
        body.insert("description".into(), json!(desc));
    }

    let created: Dataset = client
        .post(
            &format!("/api/projects/{project_id}/datasets/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&created);
    } else {
        println!("Created dataset {}", created.id);
        println!("Name:  {}", created.name);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_dataset(
    cx: &CommandContext,
    id: String,
    name: Option<String>,
    description: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), json!(n));
    }
    if let Some(d) = description {
        body.insert("description".into(), json!(d));
    }
    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name or --description)".into(),
        ));
    }

    cx.confirm(&format!("update dataset `{id}`; continue?"))?;

    let updated: Dataset = client
        .patch(
            &format!("/api/projects/{project_id}/datasets/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated dataset {}", updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_dataset(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("hard-delete dataset `{id}`; continue?"))?;

    client
        .delete(&format!("/api/projects/{project_id}/datasets/{id}/"))
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
        println!("Deleted dataset {id}");
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_roundtrip_minimal() {
        let raw =
            r#"{"id":"ds-1","name":"My dataset","created_at":"2026-04-01T00:00:00Z","team":1}"#;
        let d: Dataset = serde_json::from_str(raw).unwrap();
        assert_eq!(d.id, "ds-1");
        assert_eq!(d.name, "My dataset");
        assert!(d.description.is_none());
    }

    #[test]
    fn dataset_roundtrip_full() {
        let raw = r#"{
            "id": "ds-full",
            "name": "Full dataset",
            "description": "Test description",
            "created_at": "2026-04-01T00:00:00Z",
            "updated_at": "2026-04-02T00:00:00Z",
            "deleted": false,
            "team": 99,
            "created_by": {"id": 1, "email": "admin@example.com"},
            "metadata": null
        }"#;
        let d: Dataset = serde_json::from_str(raw).unwrap();
        assert_eq!(d.id, "ds-full");
        assert_eq!(d.description.as_deref(), Some("Test description"));
        assert_eq!(d.deleted, Some(false));
    }
}
