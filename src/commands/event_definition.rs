// src/commands/event_definition.rs
//! `bosshogg event-definition` — list / get / update / delete / by-name / metrics / tag.
//!
//! Event definitions are project-scoped. Deletion is a hard DELETE (not in
//! SOFT_DELETE_RESOURCES). Tag operations use the bulk_update_tags endpoint.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct EventDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub owner: Option<serde_json::Value>,
    #[serde(default)]
    pub last_seen_at: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_updated_at: Option<String>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub verified_at: Option<String>,
    #[serde(default)]
    pub verified_by: Option<serde_json::Value>,
    #[serde(default)]
    pub is_action: Option<bool>,
    #[serde(default)]
    pub post_to_slack: Option<bool>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct EventDefinitionArgs {
    #[command(subcommand)]
    pub command: EventDefinitionCommand,
}

#[derive(Subcommand, Debug)]
pub enum EventDefinitionCommand {
    /// List event definitions with optional filters.
    List {
        /// Filter by exact event name.
        #[arg(long)]
        event_name: Option<String>,
        /// Full-text search string.
        #[arg(long)]
        search: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single event definition by UUID.
    Get { id: String },
    /// Update an event definition's fields.
    Update {
        id: String,
        #[arg(long)]
        description: Option<String>,
        /// Mark the definition as verified.
        #[arg(long)]
        verified: bool,
        #[arg(long)]
        owner_id: Option<String>,
    },
    /// Hard-delete an event definition.
    Delete { id: String },
    /// Look up an event definition by name.
    #[command(name = "by-name")]
    ByName { name: String },
    /// Get usage metrics for an event definition.
    Metrics { id: String },
    /// Add or remove a tag on an event definition.
    Tag {
        id: String,
        #[arg(long, conflicts_with = "remove")]
        add: Option<String>,
        #[arg(long, conflicts_with = "add")]
        remove: Option<String>,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: EventDefinitionArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        EventDefinitionCommand::List { event_name, search, limit } => {
            list_event_definitions(cx, event_name, search, limit).await
        }
        EventDefinitionCommand::Get { id } => get_event_definition(cx, &id).await,
        EventDefinitionCommand::Update {
            id,
            description,
            verified,
            owner_id,
        } => {
            update_event_definition(cx, &id, description, verified, owner_id).await
        }
        EventDefinitionCommand::Delete { id } => {
            delete_event_definition(cx, &id).await
        }
        EventDefinitionCommand::ByName { name } => {
            by_name_event_definition(cx, &name).await
        }
        EventDefinitionCommand::Metrics { id } => {
            metrics_event_definition(cx, &id).await
        }
        EventDefinitionCommand::Tag { id, add, remove } => {
            tag_event_definition(cx, &id, add, remove).await
        }
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
    results: Vec<EventDefinition>,
}

async fn list_event_definitions(
    cx: &CommandContext,
    event_name: Option<String>,
    search: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut parts: Vec<String> = Vec::new();
    if let Some(en) = event_name {
        parts.push(format!("event_name={}", urlencoding::encode(&en)));
    }
    if let Some(s) = search {
        parts.push(format!("search={}", urlencoding::encode(&s)));
    }
    let qs = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };

    let path = format!("/api/projects/{project_id}/event_definitions/{qs}");
    let results: Vec<EventDefinition> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "VERIFIED", "TAGS", "LAST_SEEN_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|e| {
                vec![
                    e.id.clone(),
                    e.name.clone(),
                    e.verified
                        .map(|v| if v { "yes" } else { "no" })
                        .unwrap_or("-")
                        .to_string(),
                    e.tags.join(", "),
                    e.last_seen_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_event_definition(cx: &CommandContext, id: &str) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let def: EventDefinition = client
        .get(&format!(
            "/api/projects/{project_id}/event_definitions/{id}/"
        ))
        .await?;
    print_event_definition(&def, cx.json_mode);
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_event_definition(
    cx: &CommandContext,
    id: &str,
    description: Option<String>,
    verified: bool,
    owner_id: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(d) = description {
        body.insert("description".into(), Value::String(d));
    }
    if verified {
        body.insert("verified".into(), Value::Bool(true));
    }
    if let Some(oid) = owner_id {
        body.insert("owner".into(), Value::String(oid));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --description, --verified, --owner-id)".into(),
        ));
    }

    cx.confirm(&format!("update event definition `{id}`; continue?"))?;

    let updated: EventDefinition = client
        .patch(
            &format!("/api/projects/{project_id}/event_definitions/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated event definition '{}' (id {})",
            updated.name, updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_event_definition(
    cx: &CommandContext,
    id: &str,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("hard-delete event definition `{id}`; continue?"))?;

    client
        .delete(&format!(
            "/api/projects/{project_id}/event_definitions/{id}/"
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
            id: id.to_string(),
        });
    } else {
        println!("Deleted event definition {id}");
    }
    Ok(())
}

// ── by-name ───────────────────────────────────────────────────────────────────

async fn by_name_event_definition(cx: &CommandContext, name: &str) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let encoded = urlencoding::encode(name);
    let def: EventDefinition = client
        .get(&format!(
            "/api/projects/{project_id}/event_definitions/by_name/?name={encoded}"
        ))
        .await?;
    print_event_definition(&def, cx.json_mode);
    Ok(())
}

// ── metrics ───────────────────────────────────────────────────────────────────

async fn metrics_event_definition(cx: &CommandContext, id: &str) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/projects/{project_id}/event_definitions/{id}/metrics/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── tag ───────────────────────────────────────────────────────────────────────

async fn tag_event_definition(
    cx: &CommandContext,
    id: &str,
    add: Option<String>,
    remove: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    // Fetch current definition to get existing tags.
    let def: EventDefinition = client
        .get(&format!(
            "/api/projects/{project_id}/event_definitions/{id}/"
        ))
        .await?;

    let mut tags = def.tags.clone();

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

    cx.confirm(&format!("update tags on event definition `{id}`; continue?"))?;

    let body = json!({
        "add_tags": tags.iter().filter(|t| !def.tags.contains(t)).cloned().collect::<Vec<_>>(),
        "remove_tags": def.tags.iter().filter(|t| !tags.contains(t)).cloned().collect::<Vec<_>>(),
        "ids": [id],
    });

    let result: Value = client
        .post(
            &format!("/api/projects/{project_id}/event_definitions/bulk_update_tags/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&result);
    } else {
        println!(
            "Updated tags on event definition '{}': {}",
            def.name,
            tags.join(", ")
        );
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_event_definition(def: &EventDefinition, json_mode: bool) {
    if json_mode {
        output::print_json(def);
    } else {
        println!("ID:           {}", def.id);
        println!("Name:         {}", def.name);
        if let Some(d) = def.description.as_deref() {
            println!("Description:  {d}");
        }
        println!(
            "Verified:     {}",
            def.verified
                .map(|v| if v { "yes" } else { "no" })
                .unwrap_or("-")
        );
        println!("Tags:         {}", def.tags.join(", "));
        if let Some(ls) = def.last_seen_at.as_deref() {
            println!("Last Seen:    {ls}");
        }
        if let Some(ca) = def.created_at.as_deref() {
            println!("Created:      {ca}");
        }
        if let Some(ua) = def.last_updated_at.as_deref() {
            println!("Updated:      {ua}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn event_def_json() -> &'static str {
        r#"{
            "id": "01234567-89ab-cdef-0123-456789abcdef",
            "name": "$pageview",
            "description": "User viewed a page",
            "tags": ["web"],
            "last_seen_at": "2026-04-01T00:00:00Z",
            "created_at": "2026-01-01T00:00:00Z",
            "verified": true
        }"#
    }

    #[test]
    fn event_definition_roundtrip_minimal() {
        let raw = r#"{
            "id": "abc-123",
            "name": "$pageview",
            "tags": []
        }"#;
        let e: EventDefinition = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, "abc-123");
        assert_eq!(e.name, "$pageview");
        assert!(e.tags.is_empty());
    }

    #[test]
    fn event_definition_roundtrip_full() {
        let e: EventDefinition = serde_json::from_str(event_def_json()).unwrap();
        assert_eq!(e.name, "$pageview");
        assert_eq!(e.verified, Some(true));
        assert_eq!(e.tags, vec!["web"]);
        assert_eq!(e.description, Some("User viewed a page".into()));
    }

    #[test]
    fn event_definition_optional_fields_default() {
        let raw = r#"{"id": "x", "name": "click", "tags": []}"#;
        let e: EventDefinition = serde_json::from_str(raw).unwrap();
        assert!(e.description.is_none());
        assert!(e.verified.is_none());
        assert!(e.owner.is_none());
        assert!(e.last_seen_at.is_none());
    }

    #[test]
    fn event_definition_serialize_roundtrip() {
        let e: EventDefinition = serde_json::from_str(event_def_json()).unwrap();
        let serialized = serde_json::to_string(&e).unwrap();
        let e2: EventDefinition = serde_json::from_str(&serialized).unwrap();
        assert_eq!(e.id, e2.id);
        assert_eq!(e.name, e2.name);
        assert_eq!(e.verified, e2.verified);
    }
}
