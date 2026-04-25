// src/commands/dataset_item.rs
//! `bosshogg dataset-item` — list / get / create / update / delete.
//!
//! Dataset items are project-scoped: `/api/projects/{project_id}/dataset_items/`.
//! Items hold input/output pairs for evaluation datasets.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::read_json_file;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed structs ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DatasetItem {
    pub id: String,
    pub dataset: String,
    #[serde(default)]
    pub input: Option<Value>,
    #[serde(default)]
    pub output: Option<Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
    #[serde(default)]
    pub ref_trace_id: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct DatasetItemArgs {
    #[command(subcommand)]
    pub command: DatasetItemCommand,
}

#[derive(Subcommand, Debug)]
pub enum DatasetItemCommand {
    /// List dataset items (paginated). Optionally filter by dataset.
    List {
        /// Filter by dataset UUID.
        #[arg(long)]
        dataset: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single dataset item by ID (UUID).
    Get { id: String },
    /// Create a dataset item.
    Create {
        /// Dataset UUID this item belongs to.
        #[arg(long)]
        dataset: String,
        /// Path to a JSON file containing the input value.
        #[arg(long)]
        inputs_file: PathBuf,
        /// Path to a JSON file containing the expected output value.
        #[arg(long)]
        outputs_file: PathBuf,
    },
    /// Update a dataset item.
    Update {
        /// Dataset item UUID.
        id: String,
        /// Path to a JSON file with the new input value.
        #[arg(long)]
        inputs_file: Option<PathBuf>,
        /// Path to a JSON file with the new expected output value.
        #[arg(long)]
        outputs_file: Option<PathBuf>,
    },
    /// Delete a dataset item by ID.
    Delete {
        /// Dataset item UUID.
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

pub async fn execute(args: DatasetItemArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        DatasetItemCommand::List { dataset, limit } => list_items(cx, dataset, limit).await,
        DatasetItemCommand::Get { id } => get_item(cx, id).await,
        DatasetItemCommand::Create {
            dataset,
            inputs_file,
            outputs_file,
        } => create_item(cx, dataset, inputs_file, outputs_file).await,
        DatasetItemCommand::Update {
            id,
            inputs_file,
            outputs_file,
        } => update_item(cx, id, inputs_file, outputs_file).await,
        DatasetItemCommand::Delete { id } => delete_item(cx, id).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct DatasetItemsListOutput {
    count: usize,
    results: Vec<DatasetItem>,
}

async fn list_items(
    cx: &CommandContext,
    dataset: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut path = format!("/api/projects/{project_id}/dataset_items/");
    if let Some(ds_id) = dataset {
        path.push_str(&format!("?dataset={}", urlencoding::encode(&ds_id)));
    }

    let results: Vec<DatasetItem> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&DatasetItemsListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "DATASET", "REF_TRACE_ID", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|item| {
                vec![
                    item.id.clone(),
                    item.dataset.clone(),
                    item.ref_trace_id.clone().unwrap_or_else(|| "-".into()),
                    item.created_at.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_item(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let item: DatasetItem = client
        .get(&format!("/api/projects/{project_id}/dataset_items/{id}/"))
        .await?;

    if cx.json_mode {
        output::print_json(&item);
    } else {
        println!("ID:           {}", item.id);
        println!("Dataset:      {}", item.dataset);
        if let Some(t) = item.ref_trace_id.as_deref() {
            println!("Trace ID:     {t}");
        }
        if let Some(ca) = item.created_at.as_deref() {
            println!("Created at:   {ca}");
        }
        if let Some(ua) = item.updated_at.as_deref() {
            println!("Updated at:   {ua}");
        }
        if let Some(inp) = &item.input {
            println!(
                "Input:        {}",
                serde_json::to_string(inp).unwrap_or_default()
            );
        }
        if let Some(out) = &item.output {
            println!(
                "Output:       {}",
                serde_json::to_string(out).unwrap_or_default()
            );
        }
    }
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_item(
    cx: &CommandContext,
    dataset: String,
    inputs_file: PathBuf,
    outputs_file: PathBuf,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let input = read_json_file(&inputs_file).await?;
    let output_val = read_json_file(&outputs_file).await?;

    cx.confirm("create dataset item; continue?")?;

    let body = json!({
        "dataset": dataset,
        "input": input,
        "output": output_val,
    });

    let created: DatasetItem = client
        .post(&format!("/api/projects/{project_id}/dataset_items/"), &body)
        .await?;

    if cx.json_mode {
        output::print_json(&created);
    } else {
        println!("Created dataset item {}", created.id);
        println!("Dataset:  {}", created.dataset);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_item(
    cx: &CommandContext,
    id: String,
    inputs_file: Option<PathBuf>,
    outputs_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(f) = inputs_file {
        body.insert("input".into(), read_json_file(&f).await?);
    }
    if let Some(f) = outputs_file {
        body.insert("output".into(), read_json_file(&f).await?);
    }
    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --inputs-file or --outputs-file)".into(),
        ));
    }

    cx.confirm(&format!("update dataset item `{id}`; continue?"))?;

    let updated: DatasetItem = client
        .patch(
            &format!("/api/projects/{project_id}/dataset_items/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated dataset item {}", updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_item(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("hard-delete dataset item `{id}`; continue?"))?;

    client
        .delete(&format!("/api/projects/{project_id}/dataset_items/{id}/"))
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
        println!("Deleted dataset item {id}");
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_item_roundtrip_minimal() {
        let raw = r#"{"id":"di-1","dataset":"ds-abc"}"#;
        let item: DatasetItem = serde_json::from_str(raw).unwrap();
        assert_eq!(item.id, "di-1");
        assert_eq!(item.dataset, "ds-abc");
        assert!(item.input.is_none());
        assert!(item.output.is_none());
    }

    #[test]
    fn dataset_item_roundtrip_full() {
        let raw = r#"{
            "id": "di-full",
            "dataset": "ds-xyz",
            "input": {"question": "What is 2+2?"},
            "output": {"answer": "4"},
            "metadata": null,
            "ref_trace_id": "trace-abc",
            "created_at": "2026-04-01T00:00:00Z",
            "updated_at": "2026-04-02T00:00:00Z"
        }"#;
        let item: DatasetItem = serde_json::from_str(raw).unwrap();
        assert_eq!(item.id, "di-full");
        assert_eq!(item.ref_trace_id.as_deref(), Some("trace-abc"));
        assert!(item.input.is_some());
        assert!(item.output.is_some());
    }
}
