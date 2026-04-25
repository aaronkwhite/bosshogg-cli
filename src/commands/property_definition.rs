// src/commands/property_definition.rs
//! `bosshogg property-definition` — list / get / update / delete / seen-together.
//!
//! Property definitions are project-scoped. Deletion is a hard DELETE.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct PropertyDefinition {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub is_numerical: Option<bool>,
    #[serde(default)]
    pub property_type: Option<String>,
    #[serde(default)]
    pub is_seen_on_filtered_events: Option<bool>,
    #[serde(default)]
    pub verified: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_updated_at: Option<String>,
    #[serde(default)]
    pub last_seen_at: Option<String>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct PropertyDefinitionArgs {
    #[command(subcommand)]
    pub command: PropertyDefinitionCommand,
}

#[derive(Subcommand, Debug)]
pub enum PropertyDefinitionCommand {
    /// List property definitions with optional filters.
    List {
        /// Filter by property type: event | person | group | session.
        #[arg(long = "type")]
        prop_type: Option<String>,
        /// Comma-separated event names to filter by.
        #[arg(long)]
        event_names: Option<String>,
        /// Full-text search string.
        #[arg(long)]
        search: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single property definition by UUID.
    Get { id: String },
    /// Update a property definition's fields.
    Update {
        id: String,
        #[arg(long)]
        description: Option<String>,
        /// Mark the definition as verified.
        #[arg(long)]
        verified: bool,
    },
    /// Hard-delete a property definition.
    Delete { id: String },
    /// Find properties seen together with co-occurring events.
    #[command(name = "seen-together")]
    SeenTogether {
        /// First event name.
        #[arg(long)]
        event1: String,
        /// Second event name.
        #[arg(long)]
        event2: String,
        /// Optional property name to filter.
        #[arg(long)]
        property: Option<String>,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: PropertyDefinitionArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        PropertyDefinitionCommand::List {
            prop_type,
            event_names,
            search,
            limit,
        } => list_property_definitions(cx, prop_type, event_names, search, limit).await,
        PropertyDefinitionCommand::Get { id } => get_property_definition(cx, &id).await,
        PropertyDefinitionCommand::Update {
            id,
            description,
            verified,
        } => update_property_definition(cx, &id, description, verified).await,
        PropertyDefinitionCommand::Delete { id } => delete_property_definition(cx, &id).await,
        PropertyDefinitionCommand::SeenTogether {
            event1,
            event2,
            property,
        } => seen_together(cx, &event1, &event2, property.as_deref()).await,
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
    results: Vec<PropertyDefinition>,
}

async fn list_property_definitions(
    cx: &CommandContext,
    prop_type: Option<String>,
    event_names: Option<String>,
    search: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = prop_type {
        parts.push(format!("type={}", urlencoding::encode(&t)));
    }
    if let Some(en) = event_names {
        parts.push(format!("event_names={}", urlencoding::encode(&en)));
    }
    if let Some(s) = search {
        parts.push(format!("search={}", urlencoding::encode(&s)));
    }
    let qs = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };

    let path = format!("/api/projects/{project_id}/property_definitions/{qs}");
    let results: Vec<PropertyDefinition> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "TYPE", "VERIFIED", "TAGS"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|p| {
                vec![
                    p.id.clone(),
                    p.name.clone(),
                    p.property_type.clone().unwrap_or_else(|| "-".into()),
                    p.verified
                        .map(|v| if v { "yes" } else { "no" })
                        .unwrap_or("-")
                        .to_string(),
                    p.tags.join(", "),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_property_definition(cx: &CommandContext, id: &str) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let def: PropertyDefinition = client
        .get(&format!(
            "/api/projects/{project_id}/property_definitions/{id}/"
        ))
        .await?;
    print_property_definition(&def, cx.json_mode);
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_property_definition(
    cx: &CommandContext,
    id: &str,
    description: Option<String>,
    verified: bool,
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

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --description, --verified)".into(),
        ));
    }

    cx.confirm(&format!("update property definition `{id}`; continue?"))?;

    let updated: PropertyDefinition = client
        .patch(
            &format!("/api/projects/{project_id}/property_definitions/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated property definition '{}' (id {})",
            updated.name, updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_property_definition(cx: &CommandContext, id: &str) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "hard-delete property definition `{id}`; continue?"
    ))?;

    client
        .delete(&format!(
            "/api/projects/{project_id}/property_definitions/{id}/"
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
        println!("Deleted property definition {id}");
    }
    Ok(())
}

// ── seen-together ─────────────────────────────────────────────────────────────

async fn seen_together(
    cx: &CommandContext,
    event1: &str,
    event2: &str,
    property: Option<&str>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let event_names_encoded = urlencoding::encode(&format!("{},{}", event1, event2)).into_owned();
    let mut qs = format!("event_names={event_names_encoded}");
    if let Some(p) = property {
        qs.push_str(&format!("&property={}", urlencoding::encode(p)));
    }

    let v: Value = client
        .get(&format!(
            "/api/projects/{project_id}/property_definitions/seen_together/?{qs}"
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

fn print_property_definition(def: &PropertyDefinition, json_mode: bool) {
    if json_mode {
        output::print_json(def);
    } else {
        println!("ID:           {}", def.id);
        println!("Name:         {}", def.name);
        println!(
            "Type:         {}",
            def.property_type.as_deref().unwrap_or("-")
        );
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
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn prop_def_json() -> &'static str {
        r#"{
            "id": "prop-uuid-001",
            "name": "$browser",
            "description": "Browser name",
            "tags": ["auto"],
            "is_numerical": false,
            "property_type": "String",
            "verified": true,
            "created_at": "2026-01-01T00:00:00Z",
            "last_updated_at": "2026-04-01T00:00:00Z",
            "last_seen_at": "2026-04-20T00:00:00Z"
        }"#
    }

    #[test]
    fn property_definition_roundtrip_minimal() {
        let raw = r#"{"id": "p1", "name": "$os", "tags": []}"#;
        let p: PropertyDefinition = serde_json::from_str(raw).unwrap();
        assert_eq!(p.id, "p1");
        assert_eq!(p.name, "$os");
        assert!(p.tags.is_empty());
    }

    #[test]
    fn property_definition_roundtrip_full() {
        let p: PropertyDefinition = serde_json::from_str(prop_def_json()).unwrap();
        assert_eq!(p.name, "$browser");
        assert_eq!(p.property_type, Some("String".into()));
        assert_eq!(p.verified, Some(true));
        assert_eq!(p.tags, vec!["auto"]);
    }

    #[test]
    fn property_definition_optional_fields_default() {
        let raw = r#"{"id": "x", "name": "custom_prop", "tags": []}"#;
        let p: PropertyDefinition = serde_json::from_str(raw).unwrap();
        assert!(p.description.is_none());
        assert!(p.property_type.is_none());
        assert!(p.verified.is_none());
        assert!(p.is_numerical.is_none());
    }

    #[test]
    fn property_definition_numerical_flag() {
        let raw = r#"{"id": "n1", "name": "revenue", "tags": [], "is_numerical": true}"#;
        let p: PropertyDefinition = serde_json::from_str(raw).unwrap();
        assert_eq!(p.is_numerical, Some(true));
    }

    #[test]
    fn property_definition_serialize_roundtrip() {
        let p: PropertyDefinition = serde_json::from_str(prop_def_json()).unwrap();
        let s = serde_json::to_string(&p).unwrap();
        let p2: PropertyDefinition = serde_json::from_str(&s).unwrap();
        assert_eq!(p.id, p2.id);
        assert_eq!(p.name, p2.name);
        assert_eq!(p.property_type, p2.property_type);
    }
}
