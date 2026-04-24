// src/commands/action.rs
//! `bosshogg action` — list / get / create / update / delete / references / tag.
//!
//! Actions are project-scoped. Deletion is soft (PATCH deleted=true via
//! `client.delete`, since "actions" is in SOFT_DELETE_RESOURCES).

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::read_json_file;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Action {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub post_to_slack: Option<bool>,
    #[serde(default)]
    pub slack_message_format: Option<String>,
    pub steps: serde_json::Value, // fluid
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub is_calculating: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<serde_json::Value>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub verified: Option<bool>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ActionArgs {
    #[command(subcommand)]
    pub command: ActionCommand,
}

#[derive(Subcommand, Debug)]
pub enum ActionCommand {
    /// List actions with optional search filter.
    List {
        #[arg(long)]
        search: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single action by numeric id.
    Get { id: i64 },
    /// Create a new action.
    Create {
        #[arg(long)]
        name: String,
        /// Path to a JSON file containing the steps array.
        #[arg(long)]
        steps_file: PathBuf,
    },
    /// Update action fields.
    Update {
        id: i64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Path to a JSON file with updated steps.
        #[arg(long)]
        steps_file: Option<PathBuf>,
    },
    /// Soft-delete an action (PATCH deleted=true).
    Delete { id: i64 },
    /// List references to this action.
    References { id: i64 },
    /// Add or remove a tag on an action.
    Tag {
        id: i64,
        #[arg(long, conflicts_with = "remove")]
        add: Option<String>,
        #[arg(long, conflicts_with = "add")]
        remove: Option<String>,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: ActionArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        ActionCommand::List { search, limit } => list_actions(cx, search, limit).await,
        ActionCommand::Get { id } => get_action(cx, id).await,
        ActionCommand::Create { name, steps_file } => {
            create_action(cx, name, steps_file).await
        }
        ActionCommand::Update {
            id,
            name,
            description,
            steps_file,
        } => update_action(cx, id, name, description, steps_file).await,
        ActionCommand::Delete { id } => delete_action(cx, id).await,
        ActionCommand::References { id } => references(cx, id).await,
        ActionCommand::Tag { id, add, remove } => tag_action(cx, id, add, remove).await,
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
    results: Vec<Action>,
}

async fn list_actions(cx: &CommandContext, search: Option<String>, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let qs = if let Some(s) = search {
        format!("?search={}", urlencoding::encode(&s))
    } else {
        String::new()
    };

    let path = format!("/api/projects/{project_id}/actions/{qs}");
    let results: Vec<Action> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "TAGS", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|a| {
                vec![
                    a.id.to_string(),
                    a.name.clone(),
                    a.tags.join(", "),
                    a.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_action(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let action: Action = client
        .get(&format!("/api/projects/{project_id}/actions/{id}/"))
        .await?;
    print_action(&action, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_action(cx: &CommandContext, name: String, steps_file: PathBuf) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let steps = read_json_file(&steps_file).await?;
    let body = json!({ "name": name, "steps": steps });

    let created: Action = client
        .post(&format!("/api/projects/{project_id}/actions/"), &body)
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: i64,
            name: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            name: created.name,
        });
    } else {
        println!("Created action '{}' (id {})", created.name, created.id);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_action(
    cx: &CommandContext,
    id: i64,
    name: Option<String>,
    description: Option<String>,
    steps_file: Option<PathBuf>,
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
    if let Some(p) = steps_file.as_deref() {
        body.insert("steps".into(), read_json_file(p).await?);
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --description, --steps-file)".into(),
        ));
    }

    cx.confirm(&format!("update action `{id}`; continue?"))?;

    let updated: Action = client
        .patch(
            &format!("/api/projects/{project_id}/actions/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated action '{}' (id {})", updated.name, updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_action(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("soft-delete action `{id}`; continue?"))?;

    client
        .delete(&format!("/api/projects/{project_id}/actions/{id}/"))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: i64,
        }
        output::print_json(&Out {
            ok: true,
            action: "delete",
            id,
        });
    } else {
        println!("Deleted action {id}");
    }
    Ok(())
}

// ── references ────────────────────────────────────────────────────────────────

async fn references(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/projects/{project_id}/actions/{id}/references/"
        ))
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(arr) = v.as_array() {
        println!("References for action {id}:");
        for item in arr {
            println!("  {item}");
        }
    } else if let Some(results) = v.get("results").and_then(Value::as_array) {
        println!("References for action {id}:");
        for item in results {
            println!("  {item}");
        }
    } else {
        output::print_json(&v);
    }
    Ok(())
}

// ── tag ───────────────────────────────────────────────────────────────────────

async fn tag_action(
    cx: &CommandContext,
    id: i64,
    add: Option<String>,
    remove: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    // Fetch current tags first.
    let action: Action = client
        .get(&format!("/api/projects/{project_id}/actions/{id}/"))
        .await?;

    let mut tags = action.tags.clone();

    if let Some(tag) = add {
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    } else if let Some(tag) = remove {
        tags.retain(|t| t != &tag);
    } else {
        return Err(BosshoggError::BadRequest(
            "provide --add TAG or --remove TAG".into(),
        ));
    }

    let body = json!({ "tags": tags });
    let updated: Action = client
        .patch(&format!("/api/projects/{project_id}/actions/{id}/"), &body)
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated tags on action '{}' (id {}): {}",
            updated.name,
            updated.id,
            updated.tags.join(", ")
        );
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_action(action: &Action, json_mode: bool) {
    if json_mode {
        output::print_json(action);
    } else {
        println!("ID:          {}", action.id);
        println!("Name:        {}", action.name);
        if let Some(d) = action.description.as_deref() {
            println!("Description: {d}");
        }
        println!("Tags:        {}", action.tags.join(", "));
        println!("Deleted:     {}", action.deleted);
        if let Some(ca) = action.created_at.as_deref() {
            println!("Created:     {ca}");
        }
        if let Some(ua) = action.updated_at.as_deref() {
            println!("Updated:     {ua}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn action_json(id: i64, name: &str) -> &'static str {
        // We use a helper below that takes id/name dynamically — leaking a
        // static string isn't practical, so use the parse helper instead.
        let _ = (id, name);
        r#"{
            "id": 1,
            "name": "My Action",
            "steps": [],
            "deleted": false,
            "tags": []
        }"#
    }

    #[test]
    fn action_roundtrip_minimal() {
        let raw = action_json(1, "My Action");
        let a: Action = serde_json::from_str(raw).unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(a.name, "My Action");
        assert!(!a.deleted);
        assert!(a.tags.is_empty());
    }

    #[test]
    fn action_roundtrip_full() {
        let raw = r#"{
            "id": 42,
            "name": "Sign Up Action",
            "description": "User signs up",
            "post_to_slack": true,
            "slack_message_format": "New signup: {person.email}",
            "steps": [{"event": "$pageview", "url": "/signup"}],
            "deleted": false,
            "is_calculating": false,
            "created_at": "2026-01-01T00:00:00Z",
            "created_by": {"id": 1, "email": "admin@example.com"},
            "updated_at": "2026-04-01T00:00:00Z",
            "tags": ["marketing", "growth"],
            "verified": true
        }"#;
        let a: Action = serde_json::from_str(raw).unwrap();
        assert_eq!(a.id, 42);
        assert_eq!(a.tags, vec!["marketing", "growth"]);
        assert_eq!(a.verified, Some(true));
        assert_eq!(a.post_to_slack, Some(true));
    }

    #[test]
    fn action_missing_optional_fields_ok() {
        let raw = r#"{
            "id": 5,
            "name": "Minimal",
            "steps": null,
            "deleted": true,
            "tags": []
        }"#;
        let a: Action = serde_json::from_str(raw).unwrap();
        assert_eq!(a.id, 5);
        assert!(a.deleted);
        assert!(a.description.is_none());
    }
}
