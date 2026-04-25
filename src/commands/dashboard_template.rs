// src/commands/dashboard_template.rs
//! `bosshogg dashboard-template` — list / get / create / use.
//!
//! Dashboard templates are project-scoped.
//! Path: `/api/projects/{project_id}/dashboard_templates/`
//!
//! DELETE returns 405 (soft-delete only — PATCH `{deleted: true}`).
//! There is no dedicated "use/instantiate" endpoint in the OpenAPI spec.
//! The `use` verb is implemented as a wrapper that creates a new dashboard
//! via `/api/projects/{project_id}/dashboards/` with `use_template` set to
//! the template id and `name` set to `--new-name`.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::read_json_file;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct DashboardTemplate {
    pub id: String, // UUID
    #[serde(default)]
    pub template_name: Option<String>,
    #[serde(default)]
    pub dashboard_description: Option<String>,
    #[serde(default)]
    pub scope: Option<Value>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub is_featured: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub team_id: Option<i64>,
    #[serde(default)]
    pub tiles: Option<Value>,
    #[serde(default)]
    pub variables: Option<Value>,
    #[serde(default)]
    pub image_url: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct DashboardTemplateArgs {
    #[command(subcommand)]
    pub command: DashboardTemplateCommand,
}

#[derive(Subcommand, Debug)]
pub enum DashboardTemplateCommand {
    /// List dashboard templates.
    List {
        /// Maximum number of results to return.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single dashboard template by UUID.
    Get { id: String },
    /// Create a new dashboard template.
    Create {
        /// Template name.
        #[arg(long)]
        name: String,
        /// Path to a JSON file with the full dashboard template config (tiles, variables, etc.).
        #[arg(long)]
        template_file: Option<PathBuf>,
    },
    /// Instantiate a dashboard from this template (wrapper: creates a dashboard with use_template=<id>).
    ///
    /// NOTE: there is no dedicated instantiate endpoint in the PostHog API.
    /// This verb wraps `POST /api/projects/{project_id}/dashboards/` with
    /// `use_template = <id>` and `name = <new-name>`, which is the same action
    /// the PostHog web UI performs.
    Use {
        /// Template UUID to instantiate.
        id: String,
        /// Name for the newly created dashboard.
        #[arg(long)]
        new_name: String,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: DashboardTemplateArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        DashboardTemplateCommand::List { limit } => list_templates(cx, limit).await,
        DashboardTemplateCommand::Get { id } => get_template(cx, id).await,
        DashboardTemplateCommand::Create {
            name,
            template_file,
        } => create_template(cx, name, template_file).await,
        DashboardTemplateCommand::Use { id, new_name } => use_template(cx, id, new_name).await,
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
    results: Vec<DashboardTemplate>,
}

async fn list_templates(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path = format!("/api/projects/{project_id}/dashboard_templates/");
    let results: Vec<DashboardTemplate> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "TEMPLATE_NAME", "SCOPE", "FEATURED", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|t| {
                vec![
                    t.id.clone(),
                    t.template_name.clone().unwrap_or_else(|| "-".into()),
                    t.scope
                        .as_ref()
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_else(|| "-".into()),
                    t.is_featured
                        .map(|f| if f { "yes" } else { "no" })
                        .unwrap_or("-")
                        .to_string(),
                    t.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_template(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let template: DashboardTemplate = client
        .get(&format!(
            "/api/projects/{project_id}/dashboard_templates/{id}/"
        ))
        .await?;
    print_template(&template, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_template(
    cx: &CommandContext,
    name: String,
    template_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    body.insert("template_name".into(), Value::String(name));

    if let Some(path) = template_file {
        let config = read_json_file(&path).await?;
        if let Some(obj) = config.as_object() {
            for (k, v) in obj {
                body.insert(k.clone(), v.clone());
            }
        }
    }

    cx.confirm("create dashboard template; continue?")?;

    let created: DashboardTemplate = client
        .post(
            &format!("/api/projects/{project_id}/dashboard_templates/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: String,
            template_name: Option<String>,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            template_name: created.template_name,
        });
    } else {
        println!(
            "Created dashboard template '{}' (id {})",
            created.template_name.as_deref().unwrap_or("-"),
            created.id
        );
    }
    Ok(())
}

// ── use (instantiate) ─────────────────────────────────────────────────────────

/// Instantiate a dashboard from a template.
///
/// The PostHog OpenAPI spec does not expose a dedicated "instantiate from
/// template" endpoint on the dashboard_templates resource. This verb wraps
/// `POST /api/projects/{project_id}/dashboards/` with `use_template = <id>`,
/// which is the same action the PostHog web UI performs.
async fn use_template(cx: &CommandContext, id: String, new_name: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "create dashboard from template `{id}` named '{new_name}'; continue?"
    ))?;

    let body = json!({
        "name": new_name,
        "use_template": id,
    });

    // Dashboards endpoint returns a minimal object; use Value to avoid tight coupling.
    let created: Value = client
        .post(&format!("/api/projects/{project_id}/dashboards/"), &body)
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            dashboard_id: Value,
            name: Value,
            from_template: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "use",
            dashboard_id: created.get("id").cloned().unwrap_or(Value::Null),
            name: created.get("name").cloned().unwrap_or(Value::Null),
            from_template: id,
        });
    } else {
        let dash_id = created
            .get("id")
            .and_then(|v| v.as_i64())
            .map(|n| n.to_string())
            .unwrap_or_else(|| "-".into());
        println!("Created dashboard '{new_name}' (id {dash_id}) from template {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_template(t: &DashboardTemplate, json_mode: bool) {
    if json_mode {
        output::print_json(t);
    } else {
        println!("ID:           {}", t.id);
        if let Some(n) = t.template_name.as_deref() {
            println!("Name:         {n}");
        }
        if let Some(d) = t.dashboard_description.as_deref() {
            println!("Description:  {d}");
        }
        if let Some(f) = t.is_featured {
            println!("Featured:     {f}");
        }
        if let Some(ca) = t.created_at.as_deref() {
            println!("Created:      {ca}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_template_roundtrip_minimal() {
        let raw = r#"{
            "id": "dt-uuid-1",
            "template_name": "My Template",
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let t: DashboardTemplate = serde_json::from_str(raw).unwrap();
        assert_eq!(t.id, "dt-uuid-1");
        assert_eq!(t.template_name.as_deref(), Some("My Template"));
    }

    #[test]
    fn dashboard_template_roundtrip_full() {
        let raw = r#"{
            "id": "dt-uuid-2",
            "template_name": "Product Analytics",
            "dashboard_description": "Core metrics",
            "scope": "global",
            "is_featured": true,
            "deleted": false,
            "team_id": 42,
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let t: DashboardTemplate = serde_json::from_str(raw).unwrap();
        assert_eq!(t.id, "dt-uuid-2");
        assert_eq!(t.is_featured, Some(true));
        assert_eq!(t.team_id, Some(42));
    }

    #[test]
    fn dashboard_template_scope_variants() {
        for scope in &["team", "global", "feature_flag"] {
            let raw = format!(
                r#"{{"id": "x", "scope": "{scope}", "created_at": null, "created_by": null, "team_id": null}}"#
            );
            let t: DashboardTemplate = serde_json::from_str(&raw).unwrap();
            assert_eq!(t.scope.as_ref().and_then(|v| v.as_str()), Some(*scope));
        }
    }
}
