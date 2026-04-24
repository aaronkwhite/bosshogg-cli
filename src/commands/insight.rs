// src/commands/insight.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::{env_id_required, read_json_file};
use crate::error::{BosshoggError, Result};
use crate::output;
use crate::util::is_short_id;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Insight {
    pub id: i64,
    pub short_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub filters: Value,
    #[serde(default)]
    pub query: Option<Value>,
    #[serde(default)]
    pub result: Option<Value>,
    #[serde(default)]
    pub order: Option<i32>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorited: Option<bool>,
    #[serde(default)]
    pub dashboards: Option<Vec<i64>>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub last_refresh: Option<String>,
    #[serde(default)]
    pub refreshing: Option<bool>,
    #[serde(default)]
    pub saved: Option<bool>,
    #[serde(default)]
    pub is_sample: Option<bool>,
    #[serde(default)]
    pub timezone: Option<String>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct InsightArgs {
    #[command(subcommand)]
    pub command: InsightCommand,
}

#[derive(Subcommand, Debug)]
pub enum InsightCommand {
    /// List saved insights with optional filters.
    List(ListArgs),
    /// Get a single insight by short_id or numeric id.
    Get(GetArgs),
    /// Force-refresh an insight's cached result.
    Refresh(RefreshArgs),
    /// Create a new insight from a filters JSON file.
    Create(CreateArgs),
    /// Update insight fields (name, description, tags, filters, etc.).
    Update(UpdateArgs),
    /// Soft-delete an insight (PATCH deleted=true).
    Delete(DeleteArgs),
    /// Add or remove tags on an insight.
    Tag(TagArgs),
    /// View the activity log for an insight.
    Activity(ActivityArgs),
    /// View sharing settings for an insight (read-only in M3).
    Share(ShareArgs),
}

#[derive(Args, Debug)]
pub struct ListArgs {
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub search: Option<String>,
    #[arg(long)]
    pub short_id: Option<String>,
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Args, Debug)]
pub struct GetArgs {
    /// Insight short_id (e.g. abc123X9) or numeric id.
    pub identifier: String,
}

#[derive(Args, Debug)]
pub struct RefreshArgs {
    /// Insight short_id or numeric id.
    pub identifier: String,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    /// Path to a JSON file containing the legacy `filters` object.
    /// Mutually exclusive with `--query-file`. Newer PostHog accounts may
    /// reject legacy filters; prefer `--query-file`.
    #[arg(
        long,
        conflicts_with = "query_file",
        required_unless_present = "query_file"
    )]
    pub filters_file: Option<PathBuf>,
    /// Path to a JSON file containing the modern `query` object
    /// (e.g. `{"kind":"InsightVizNode","source":{"kind":"TrendsQuery",...}}`).
    #[arg(long)]
    pub query_file: Option<PathBuf>,
    #[arg(long)]
    pub saved: bool,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    /// Insight short_id or numeric id.
    pub identifier: String,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
    /// Path to a JSON file containing updated legacy `filters`.
    #[arg(long, conflicts_with = "query_file")]
    pub filters_file: Option<PathBuf>,
    /// Path to a JSON file containing the modern `query` object.
    #[arg(long)]
    pub query_file: Option<PathBuf>,
    /// Add a tag (can be repeated).
    #[arg(long)]
    pub tag: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DeleteArgs {
    /// Insight short_id or numeric id.
    pub identifier: String,
}

#[derive(Args, Debug)]
pub struct TagArgs {
    /// Insight short_id or numeric id.
    pub identifier: String,
    /// Tag to add.
    #[arg(long)]
    pub add: Option<String>,
    /// Tag to remove.
    #[arg(long)]
    pub remove: Option<String>,
}

#[derive(Args, Debug)]
pub struct ActivityArgs {
    /// Insight short_id or numeric id.
    pub identifier: String,
}

#[derive(Args, Debug)]
pub struct ShareArgs {
    /// Insight short_id or numeric id.
    pub identifier: String,
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: InsightArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        InsightCommand::List(a) => list_insights(cx, a).await,
        InsightCommand::Get(a) => get_insight(cx, a).await,
        InsightCommand::Refresh(a) => refresh_insight(cx, a).await,
        InsightCommand::Create(a) => create_insight(cx, a).await,
        InsightCommand::Update(a) => update_insight(cx, a).await,
        InsightCommand::Delete(a) => delete_insight(cx, a).await,
        InsightCommand::Tag(a) => tag_insight(cx, a).await,
        InsightCommand::Activity(a) => activity_insight(cx, a).await,
        InsightCommand::Share(a) => share_insight(cx, a).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Resolve a user-supplied identifier (short_id or numeric) to a numeric insight id.
///
/// - If it looks like a short_id → list with `?short_id=…` to find the numeric id.
/// - Otherwise parse as i64 directly.
/// - Cache the mapping for the lifetime of this process.
async fn resolve_identifier_to_id(client: &Client, env_id: &str, identifier: &str) -> Result<i64> {
    if is_short_id(identifier) {
        // Check in-process cache first.
        if let Some(cached) = client.cache().insight_id_for_short_id(identifier) {
            return Ok(cached);
        }

        // List with short_id filter to resolve.
        let path = format!(
            "/api/environments/{env_id}/insights/?short_id={}",
            urlencoding::encode(identifier)
        );
        let hits: Vec<Insight> = client.get_paginated(&path, Some(10)).await?;
        let found = hits
            .into_iter()
            .find(|i| i.short_id == identifier)
            .ok_or_else(|| BosshoggError::NotFound(format!("insight short_id '{identifier}'")))?;

        client.cache().remember_insight(identifier, found.id);
        Ok(found.id)
    } else if let Ok(id) = identifier.parse::<i64>() {
        Ok(id)
    } else {
        Err(BosshoggError::BadRequest(format!(
            "'{identifier}' is not a numeric id or short_id (6–8 alphanumeric chars)"
        )))
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Insight>,
}

async fn list_insights(cx: &CommandContext, args: ListArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut qs: Vec<(String, String)> = Vec::new();
    if let Some(t) = &args.tag {
        qs.push(("tags".into(), t.clone()));
    }
    if let Some(s) = &args.search {
        qs.push(("search".into(), s.clone()));
    }
    if let Some(sid) = &args.short_id {
        qs.push(("short_id".into(), sid.clone()));
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

    let path = format!("/api/environments/{env_id}/insights/{query}");
    let results: Vec<Insight> = client.get_paginated(&path, args.limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "SHORT_ID", "NAME", "TAGS", "SAVED", "UPDATED"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|i| {
                vec![
                    i.id.to_string(),
                    i.short_id.clone(),
                    i.name.clone().unwrap_or_default(),
                    i.tags.join(","),
                    i.saved.map(|s| s.to_string()).unwrap_or_else(|| "-".into()),
                    i.updated_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_insight(cx: &CommandContext, args: GetArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let id = resolve_identifier_to_id(client, env_id, &args.identifier).await?;
    let insight: Insight = client
        .get(&format!("/api/environments/{env_id}/insights/{id}/"))
        .await?;
    print_insight(&insight, cx.json_mode);
    Ok(())
}

// ── refresh ───────────────────────────────────────────────────────────────────

async fn refresh_insight(cx: &CommandContext, args: RefreshArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let id = resolve_identifier_to_id(client, env_id, &args.identifier).await?;
    let insight: Insight = client
        .get(&format!(
            "/api/environments/{env_id}/insights/{id}/?refresh=true"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&insight);
    } else {
        println!(
            "Refreshed insight '{}' (id {})",
            insight.short_id, insight.id
        );
        if let Some(lr) = insight.last_refresh.as_deref() {
            println!("Last refresh: {lr}");
        }
    }
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_insight(cx: &CommandContext, args: CreateArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = json!({ "saved": args.saved });
    if let Some(p) = args.filters_file.as_deref() {
        body["filters"] = read_json_file(p).await?;
    }
    if let Some(p) = args.query_file.as_deref() {
        body["query"] = read_json_file(p).await?;
    }
    if let Some(n) = args.name.as_deref() {
        body["name"] = Value::String(n.into());
    }
    if let Some(d) = args.description.as_deref() {
        body["description"] = Value::String(d.into());
    }

    let created: Insight = client
        .post(&format!("/api/environments/{env_id}/insights/"), &body)
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            action: &'a str,
            id: i64,
            short_id: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            short_id: &created.short_id,
        });
    } else {
        println!(
            "Created insight '{}' (id {}, short_id {})",
            created.name.as_deref().unwrap_or("<unnamed>"),
            created.id,
            created.short_id
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_insight(cx: &CommandContext, args: UpdateArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let id = resolve_identifier_to_id(client, env_id, &args.identifier).await?;

    let mut body = serde_json::Map::new();
    if let Some(n) = args.name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(d) = args.description {
        body.insert("description".into(), Value::String(d));
    }
    if let Some(p) = args.filters_file.as_deref() {
        body.insert("filters".into(), read_json_file(p).await?);
    }
    if let Some(p) = args.query_file.as_deref() {
        body.insert("query".into(), read_json_file(p).await?);
    }
    if !args.tag.is_empty() {
        let tag_vals: Vec<Value> = args.tag.iter().map(|t| Value::String(t.clone())).collect();
        body.insert("tags".into(), Value::Array(tag_vals));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --description, --filters-file, --query-file, --tag)".into(),
        ));
    }

    let changes: Vec<String> = body.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let changes_str = changes.join(", ");

    cx.confirm(&format!(
        "update insight `{}` with these changes: {}; continue?",
        args.identifier, changes_str
    ))?;

    let updated: Insight = client
        .patch(
            &format!("/api/environments/{env_id}/insights/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated insight '{}' (id {})", updated.short_id, updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_insight(cx: &CommandContext, args: DeleteArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "soft-delete insight `{}`; continue?",
        args.identifier
    ))?;

    let id = resolve_identifier_to_id(client, env_id, &args.identifier).await?;
    // client.delete routes soft-delete resources through PATCH {deleted: true}.
    client
        .delete(&format!("/api/environments/{env_id}/insights/{id}/"))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            action: &'a str,
            deleted: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            action: "delete",
            deleted: &args.identifier,
        });
    } else {
        println!("Deleted insight '{}'", args.identifier);
    }
    Ok(())
}

// ── tag ───────────────────────────────────────────────────────────────────────

async fn tag_insight(cx: &CommandContext, args: TagArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let id = resolve_identifier_to_id(client, env_id, &args.identifier).await?;

    if args.add.is_none() && args.remove.is_none() {
        return Err(BosshoggError::BadRequest(
            "provide --add <tag> or --remove <tag> (or both)".into(),
        ));
    }

    // Fetch current tags first so we can compute the new list.
    let current: Insight = client
        .get(&format!("/api/environments/{env_id}/insights/{id}/"))
        .await?;

    let mut tags: Vec<String> = current.tags.clone();

    if let Some(ref add) = args.add {
        if !tags.contains(add) {
            tags.push(add.clone());
        }
    }
    if let Some(ref remove) = args.remove {
        tags.retain(|t| t != remove);
    }

    let tag_vals: Vec<Value> = tags.iter().map(|t| Value::String(t.clone())).collect();
    let updated: Insight = client
        .patch(
            &format!("/api/environments/{env_id}/insights/{id}/"),
            &json!({ "tags": tag_vals }),
        )
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            id: i64,
            short_id: &'a str,
            tags: &'a Vec<String>,
        }
        output::print_json(&Out {
            ok: true,
            id: updated.id,
            short_id: &updated.short_id,
            tags: &updated.tags,
        });
    } else {
        println!(
            "Tags for insight '{}': {}",
            updated.short_id,
            updated.tags.join(", ")
        );
    }
    Ok(())
}

// ── activity ──────────────────────────────────────────────────────────────────

async fn activity_insight(cx: &CommandContext, args: ActivityArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let id = resolve_identifier_to_id(client, env_id, &args.identifier).await?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/insights/{id}/activity/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(results) = v.get("results").and_then(Value::as_array) {
        for e in results {
            let a = e.get("activity").and_then(Value::as_str).unwrap_or("-");
            let t = e.get("created_at").and_then(Value::as_str).unwrap_or("-");
            let u = e
                .pointer("/user/email")
                .and_then(Value::as_str)
                .unwrap_or("-");
            println!("{t}  {a:<12}  {u}");
        }
    }
    Ok(())
}

// ── share ─────────────────────────────────────────────────────────────────────

async fn share_insight(cx: &CommandContext, args: ShareArgs) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let id = resolve_identifier_to_id(client, env_id, &args.identifier).await?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/insights/{id}/sharing/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        let url = v
            .get("access_token")
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        println!("Sharing enabled: {enabled}");
        println!("Access token:    {url}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_insight(insight: &Insight, json_mode: bool) {
    if json_mode {
        output::print_json(insight);
    } else {
        println!("ID:           {}", insight.id);
        println!("Short ID:     {}", insight.short_id);
        if let Some(n) = insight.name.as_deref() {
            println!("Name:         {n}");
        }
        if let Some(d) = insight.description.as_deref() {
            println!("Description:  {d}");
        }
        if !insight.tags.is_empty() {
            println!("Tags:         {}", insight.tags.join(", "));
        }
        if let Some(lr) = insight.last_refresh.as_deref() {
            println!("Last refresh: {lr}");
        }
        if let Some(ua) = insight.updated_at.as_deref() {
            println!("Updated:      {ua}");
        }
        println!("Deleted:      {}", insight.deleted);
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insight_roundtrip_minimal() {
        let raw = r#"{
            "id": 42,
            "short_id": "abc123XY",
            "name": "My trend",
            "filters": {"events": []},
            "deleted": false,
            "tags": ["prod"]
        }"#;
        let i: Insight = serde_json::from_str(raw).unwrap();
        assert_eq!(i.id, 42);
        assert_eq!(i.short_id, "abc123XY");
        assert_eq!(i.tags, vec!["prod"]);
        assert!(!i.deleted);
    }

    #[test]
    fn insight_roundtrip_full_optional_fields() {
        let raw = r#"{
            "id": 1, "short_id": "xY12ab",
            "name": "Funnel", "description": "Test",
            "filters": {}, "query": null, "result": [1,2,3],
            "deleted": false, "tags": [],
            "saved": true, "favorited": false,
            "last_refresh": "2026-04-01T00:00:00Z",
            "refreshing": false, "is_sample": false,
            "timezone": "UTC",
            "created_by": {"id": 1, "email": "a@b.com"},
            "dashboards": [10, 20]
        }"#;
        let i: Insight = serde_json::from_str(raw).unwrap();
        assert_eq!(i.saved, Some(true));
        assert_eq!(i.dashboards, Some(vec![10, 20]));
        assert_eq!(i.timezone.as_deref(), Some("UTC"));
    }
}
