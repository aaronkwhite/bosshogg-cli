// src/commands/annotation.rs
//! `bosshogg annotation` — list / get / create / update / delete.
//!
//! Annotations are project-scoped. Deletion is soft (PATCH deleted=true via
//! `client.delete`, since "annotations" is in SOFT_DELETE_RESOURCES).

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Annotation {
    pub id: i64,
    pub content: String,
    pub date_marker: String, // ISO timestamp
    #[serde(default)]
    pub creation_type: Option<String>,
    #[serde(default)]
    pub dashboard_item: Option<i64>,
    #[serde(default)]
    pub insight_short_id: Option<String>,
    #[serde(default)]
    pub insight_name: Option<String>,
    #[serde(default)]
    pub scope: Option<String>, // "organization", "project", "dashboard_item"
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub created_by: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct AnnotationArgs {
    #[command(subcommand)]
    pub command: AnnotationCommand,
}

#[derive(Subcommand, Debug)]
pub enum AnnotationCommand {
    /// List annotations with optional date filters.
    List {
        /// Only annotations before this ISO timestamp.
        #[arg(long)]
        before: Option<String>,
        /// Only annotations after this ISO timestamp.
        #[arg(long)]
        after: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single annotation by numeric id.
    Get { id: i64 },
    /// Create a new annotation.
    Create {
        /// Annotation text content.
        #[arg(long)]
        content: String,
        /// ISO timestamp to mark (date_marker).
        #[arg(long)]
        date_marker: String,
        /// Dashboard item (insight) id to scope to.
        #[arg(long)]
        dashboard_item: Option<i64>,
        /// Scope: organization | project | dashboard_item.
        #[arg(long)]
        scope: Option<String>,
    },
    /// Update annotation fields.
    Update {
        id: i64,
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        date_marker: Option<String>,
    },
    /// Soft-delete an annotation (PATCH deleted=true).
    Delete { id: i64 },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: AnnotationArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        AnnotationCommand::List {
            before,
            after,
            limit,
        } => list_annotations(cx, before, after, limit).await,
        AnnotationCommand::Get { id } => get_annotation(cx, id).await,
        AnnotationCommand::Create {
            content,
            date_marker,
            dashboard_item,
            scope,
        } => create_annotation(cx, content, date_marker, dashboard_item, scope).await,
        AnnotationCommand::Update {
            id,
            content,
            date_marker,
        } => update_annotation(cx, id, content, date_marker).await,
        AnnotationCommand::Delete { id } => delete_annotation(cx, id).await,
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
    results: Vec<Annotation>,
}

async fn list_annotations(
    cx: &CommandContext,
    before: Option<String>,
    after: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut parts: Vec<String> = Vec::new();
    if let Some(b) = before {
        parts.push(format!("before={}", urlencoding::encode(&b)));
    }
    if let Some(a) = after {
        parts.push(format!("after={}", urlencoding::encode(&a)));
    }
    let qs = if parts.is_empty() {
        String::new()
    } else {
        format!("?{}", parts.join("&"))
    };

    let path = format!("/api/projects/{project_id}/annotations/{qs}");
    let results: Vec<Annotation> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "DATE_MARKER", "SCOPE", "CONTENT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|a| {
                vec![
                    a.id.to_string(),
                    a.date_marker.clone(),
                    a.scope.clone().unwrap_or_else(|| "-".into()),
                    a.content.chars().take(60).collect::<String>()
                        + if a.content.len() > 60 { "..." } else { "" },
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_annotation(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let ann: Annotation = client
        .get(&format!("/api/projects/{project_id}/annotations/{id}/"))
        .await?;
    print_annotation(&ann, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_annotation(
    cx: &CommandContext,
    content: String,
    date_marker: String,
    dashboard_item: Option<i64>,
    scope: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = json!({
        "content": content,
        "date_marker": date_marker,
    });

    if let Some(di) = dashboard_item {
        body["dashboard_item"] = json!(di);
    }
    if let Some(s) = scope {
        body["scope"] = json!(s);
    }

    let created: Annotation = client
        .post(&format!("/api/projects/{project_id}/annotations/"), &body)
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
            action: "create",
            id: created.id,
        });
    } else {
        println!(
            "Created annotation {} at {} (scope: {})",
            created.id,
            created.date_marker,
            created.scope.as_deref().unwrap_or("-")
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_annotation(
    cx: &CommandContext,
    id: i64,
    content: Option<String>,
    date_marker: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(c) = content {
        body.insert("content".into(), Value::String(c));
    }
    if let Some(dm) = date_marker {
        body.insert("date_marker".into(), Value::String(dm));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --content, --date-marker)".into(),
        ));
    }

    cx.confirm(&format!("update annotation `{id}`; continue?"))?;

    let updated: Annotation = client
        .patch(
            &format!("/api/projects/{project_id}/annotations/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated annotation {} (date_marker: {})",
            updated.id, updated.date_marker
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_annotation(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("soft-delete annotation `{id}`; continue?"))?;

    client
        .delete(&format!("/api/projects/{project_id}/annotations/{id}/"))
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
        println!("Deleted annotation {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_annotation(ann: &Annotation, json_mode: bool) {
    if json_mode {
        output::print_json(ann);
    } else {
        println!("ID:          {}", ann.id);
        println!("Content:     {}", ann.content);
        println!("Date Marker: {}", ann.date_marker);
        println!("Scope:       {}", ann.scope.as_deref().unwrap_or("-"));
        if let Some(di) = ann.dashboard_item {
            println!("Dashboard:   {di}");
        }
        println!("Deleted:     {}", ann.deleted);
        if let Some(ca) = ann.created_at.as_deref() {
            println!("Created:     {ca}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annotation_roundtrip_minimal() {
        let raw = r#"{
            "id": 1,
            "content": "Deploy v1.0",
            "date_marker": "2026-01-01T12:00:00Z",
            "deleted": false
        }"#;
        let a: Annotation = serde_json::from_str(raw).unwrap();
        assert_eq!(a.id, 1);
        assert_eq!(a.content, "Deploy v1.0");
        assert_eq!(a.date_marker, "2026-01-01T12:00:00Z");
        assert!(!a.deleted);
    }

    #[test]
    fn annotation_roundtrip_full() {
        let raw = r#"{
            "id": 42,
            "content": "Feature launch",
            "date_marker": "2026-04-01T00:00:00Z",
            "creation_type": "USER",
            "dashboard_item": 5,
            "insight_short_id": "abc123",
            "insight_name": "Daily signups",
            "scope": "project",
            "deleted": false,
            "created_by": {"id": 1, "email": "admin@example.com"},
            "created_at": "2026-03-30T10:00:00Z",
            "updated_at": "2026-03-31T10:00:00Z"
        }"#;
        let a: Annotation = serde_json::from_str(raw).unwrap();
        assert_eq!(a.id, 42);
        assert_eq!(a.scope, Some("project".into()));
        assert_eq!(a.dashboard_item, Some(5));
        assert_eq!(a.insight_short_id, Some("abc123".into()));
    }

    #[test]
    fn annotation_deleted_flag() {
        let raw = r#"{
            "id": 7,
            "content": "Old event",
            "date_marker": "2025-12-01T00:00:00Z",
            "deleted": true
        }"#;
        let a: Annotation = serde_json::from_str(raw).unwrap();
        assert!(a.deleted);
    }

    #[test]
    fn annotation_scope_variants() {
        for scope in &["organization", "project", "dashboard_item"] {
            let raw = format!(
                r#"{{"id":1,"content":"x","date_marker":"2026-01-01T00:00:00Z","deleted":false,"scope":"{scope}"}}"#
            );
            let a: Annotation = serde_json::from_str(&raw).unwrap();
            assert_eq!(a.scope.as_deref(), Some(*scope));
        }
    }
}
