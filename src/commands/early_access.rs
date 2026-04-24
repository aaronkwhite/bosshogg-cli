// src/commands/early_access.rs
//! `bosshogg early-access` — list / get / create / update / delete.
//!
//! Early access features are project-scoped. The PostHog API path is
//! `early_access_feature` (singular). Deletion is a HARD DELETE.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EarlyAccessFeature {
    pub id: String, // UUID
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub stage: String, // alpha, beta, general-availability, etc.
    #[serde(default)]
    pub feature_flag: Option<Value>,
    #[serde(default)]
    pub feature_flag_id: Option<i64>,
    #[serde(default)]
    pub documentation_url: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct EarlyAccessArgs {
    #[command(subcommand)]
    pub command: EarlyAccessCommand,
}

#[derive(Subcommand, Debug)]
pub enum EarlyAccessCommand {
    /// List early access features.
    List,
    /// Get a single early access feature by UUID.
    Get { id: String },
    /// Create a new early access feature.
    Create {
        #[arg(long)]
        name: String,
        #[arg(long)]
        description: String,
        /// Stage (alpha, beta, general-availability, etc.).
        #[arg(long)]
        stage: String,
        /// Numeric feature flag ID to associate.
        #[arg(long)]
        feature_flag_id: i64,
    },
    /// Update early access feature fields.
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Stage (alpha, beta, general-availability, etc.).
        #[arg(long)]
        stage: Option<String>,
    },
    /// Hard-delete an early access feature (DELETE HTTP verb).
    Delete { id: String },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: EarlyAccessArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        EarlyAccessCommand::List => list_early_access(cx).await,
        EarlyAccessCommand::Get { id } => get_early_access(cx, id).await,
        EarlyAccessCommand::Create {
            name,
            description,
            stage,
            feature_flag_id,
        } => create_early_access(cx, name, description, stage, feature_flag_id).await,
        EarlyAccessCommand::Update {
            id,
            name,
            description,
            stage,
        } => update_early_access(cx, id, name, description, stage).await,
        EarlyAccessCommand::Delete { id } => delete_early_access(cx, id).await,
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
    results: Vec<EarlyAccessFeature>,
}

async fn list_early_access(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path = format!("/api/projects/{project_id}/early_access_feature/");
    let results: Vec<EarlyAccessFeature> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "STAGE", "FEATURE_FLAG_ID", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|e| {
                vec![
                    e.id.clone(),
                    e.name.clone(),
                    e.stage.clone(),
                    e.feature_flag_id
                        .map(|n| n.to_string())
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

async fn get_early_access(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let feature: EarlyAccessFeature = client
        .get(&format!(
            "/api/projects/{project_id}/early_access_feature/{id}/"
        ))
        .await?;
    print_early_access(&feature, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_early_access(
    cx: &CommandContext,
    name: String,
    description: String,
    stage: String,
    feature_flag_id: i64,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let body = json!({
        "name": name,
        "description": description,
        "stage": stage,
        "feature_flag_id": feature_flag_id,
    });

    let created: EarlyAccessFeature = client
        .post(
            &format!("/api/projects/{project_id}/early_access_feature/"),
            &body,
        )
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
            "Created early access feature '{}' (id {})",
            created.name, created.id
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_early_access(
    cx: &CommandContext,
    id: String,
    name: Option<String>,
    description: Option<String>,
    stage: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(d) = description {
        body.insert("description".into(), Value::String(d));
    }
    if let Some(s) = stage {
        body.insert("stage".into(), Value::String(s));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --description, --stage)".into(),
        ));
    }

    cx.confirm(&format!("update early access feature `{id}`; continue?"))?;

    let updated: EarlyAccessFeature = client
        .patch(
            &format!("/api/projects/{project_id}/early_access_feature/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated early access feature '{}' (id {})",
            updated.name, updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_early_access(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "hard-delete early access feature `{id}`; continue?"
    ))?;

    client
        .delete(&format!(
            "/api/projects/{project_id}/early_access_feature/{id}/"
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
        println!("Deleted early access feature {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_early_access(feature: &EarlyAccessFeature, json_mode: bool) {
    if json_mode {
        output::print_json(feature);
    } else {
        println!("ID:               {}", feature.id);
        println!("Name:             {}", feature.name);
        if let Some(d) = feature.description.as_deref() {
            println!("Description:      {d}");
        }
        println!("Stage:            {}", feature.stage);
        if let Some(flag_id) = feature.feature_flag_id {
            println!("Feature Flag ID:  {flag_id}");
        }
        if let Some(url) = feature.documentation_url.as_deref() {
            println!("Docs URL:         {url}");
        }
        if let Some(ca) = feature.created_at.as_deref() {
            println!("Created:          {ca}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn early_access_roundtrip_minimal() {
        let raw = r#"{
            "id": "ea-uuid-1",
            "name": "My Feature",
            "stage": "beta"
        }"#;
        let e: EarlyAccessFeature = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, "ea-uuid-1");
        assert_eq!(e.name, "My Feature");
        assert_eq!(e.stage, "beta");
    }

    #[test]
    fn early_access_roundtrip_full() {
        let raw = r#"{
            "id": "ea-uuid-2",
            "name": "Full Feature",
            "description": "A beta feature",
            "stage": "alpha",
            "feature_flag": {"id": 55, "key": "full-feature-flag"},
            "feature_flag_id": 55,
            "documentation_url": "https://docs.example.com/full-feature",
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let e: EarlyAccessFeature = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, "ea-uuid-2");
        assert_eq!(e.feature_flag_id, Some(55));
        assert_eq!(
            e.documentation_url.as_deref(),
            Some("https://docs.example.com/full-feature")
        );
    }

    #[test]
    fn early_access_stage_variants() {
        for stage in &["alpha", "beta", "general-availability"] {
            let raw = format!(r#"{{"id": "x", "name": "Y", "stage": "{stage}"}}"#);
            let e: EarlyAccessFeature = serde_json::from_str(&raw).unwrap();
            assert_eq!(&e.stage, stage);
        }
    }
}
