// src/commands/flag.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::read_json_file;
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flag {
    pub id: i64,
    pub key: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default, rename = "filters")]
    pub filters: Value,
    #[serde(default)]
    pub rollout_percentage: Option<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub evaluation_runtime: Option<String>,
    #[serde(default, rename = "created_at")]
    pub created_at: Option<String>,
    #[serde(default, rename = "updated_at")]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub ensure_experience_continuity: Option<bool>,
    #[serde(flatten, default)]
    pub extra: HashMap<String, Value>,
}

#[derive(Args, Debug)]
pub struct FlagArgs {
    #[command(subcommand)]
    pub command: FlagCommand,
}

#[derive(Subcommand, Debug)]
pub enum FlagCommand {
    /// List feature flags with optional filters.
    List(ListArgs),
    /// Get a single flag by key.
    Get(GetArgs),
    /// Create a new flag.
    Create(CreateArgs),
    /// Update a flag's fields.
    Update(UpdateArgs),
    /// Soft-delete a flag (PATCH deleted=true).
    Delete { key: String },
    /// Convenience wrapper: --enabled.
    Enable { key: String },
    /// Convenience wrapper: --disabled.
    Disable { key: String },
    /// Set rollout percentage.
    Rollout { key: String, percent: u8 },
    /// Evaluate via POST /flags?v=2 using the project token.
    Evaluate(EvaluateArgs),
    /// List flags that depend on this one.
    Dependents { key: String },
    /// Activity log for a flag.
    Activity { key: String },
}

#[derive(Args, Debug)]
pub struct ListArgs {
    #[arg(long)]
    pub active: bool,
    #[arg(long)]
    pub r#type: Option<String>,
    #[arg(long)]
    pub runtime: Option<String>,
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub search: Option<String>,
    #[arg(long)]
    pub limit: Option<usize>,
}

#[derive(Args, Debug)]
pub struct GetArgs {
    pub key: String,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    #[arg(long)]
    pub name: String,
    #[arg(long)]
    pub key: String,
    #[arg(long)]
    pub filters_file: Option<PathBuf>,
    #[arg(long)]
    pub payload_file: Option<PathBuf>,
    #[arg(long)]
    pub description: Option<String>,
    #[arg(long)]
    pub active: bool,
}

#[derive(Args, Debug)]
pub struct UpdateArgs {
    pub key: String,
    #[arg(long)]
    pub enabled: bool,
    #[arg(long, conflicts_with = "enabled")]
    pub disabled: bool,
    #[arg(long)]
    pub rollout: Option<u8>,
    #[arg(long)]
    pub filters_file: Option<PathBuf>,
    #[arg(long)]
    pub payload_file: Option<PathBuf>,
    #[arg(long)]
    pub name: Option<String>,
    #[arg(long)]
    pub description: Option<String>,
}

#[derive(Args, Debug)]
pub struct EvaluateArgs {
    #[arg(long)]
    pub distinct_id: String,
    #[arg(long)]
    pub groups: Vec<String>, // "type=key" pairs
    #[arg(long)]
    pub person_props: Option<PathBuf>,
    /// Override project token (phc_...). Defaults to context's project_token if configured.
    #[arg(long)]
    pub project_token: Option<String>,
}

pub async fn execute(args: FlagArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        FlagCommand::List(a) => list_flags(cx, a).await,
        FlagCommand::Get(a) => get_flag(cx, a).await,
        FlagCommand::Create(a) => create_flag(cx, a).await,
        FlagCommand::Update(a) => update_flag(cx, a).await,
        FlagCommand::Delete { key } => delete_flag(cx, key).await,
        FlagCommand::Enable { key } => enable_flag(cx, key).await,
        FlagCommand::Disable { key } => disable_flag(cx, key).await,
        FlagCommand::Rollout { key, percent } => rollout_flag(cx, key, percent).await,
        FlagCommand::Evaluate(a) => evaluate_flags(cx, a).await,
        FlagCommand::Dependents { key } => dependents(cx, key).await,
        FlagCommand::Activity { key } => activity(cx, key).await,
    }
}

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    next_cursor: Option<String>,
    results: Vec<Flag>,
}

async fn list_flags(cx: &CommandContext, args: ListArgs) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;

    let mut qs: Vec<(String, String)> = Vec::new();
    if args.active {
        qs.push(("active".into(), "true".into()));
    }
    if let Some(t) = args.r#type {
        qs.push(("type".into(), t));
    }
    if let Some(r) = args.runtime {
        qs.push(("evaluation_runtime".into(), r));
    }
    if let Some(t) = args.tag {
        qs.push(("tags".into(), t));
    }
    if let Some(s) = args.search {
        qs.push(("search".into(), s));
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

    let path = format!("/api/projects/{pid}/feature_flags/{query}");
    let results: Vec<Flag> = client.get_paginated(&path, args.limit).await?;

    if cx.is_json() {
        output::print_json(&ListOutput {
            count: results.len(),
            next_cursor: None,
            results,
        });
    } else {
        let headers = &["ID", "KEY", "NAME", "ACTIVE", "ROLLOUT", "TAGS"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|f| {
                vec![
                    f.id.to_string(),
                    f.key.clone(),
                    f.name.clone().unwrap_or_default(),
                    f.active.to_string(),
                    f.rollout_percentage
                        .map(|r| format!("{r}%"))
                        .unwrap_or_else(|| "-".into()),
                    f.tags.join(","),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

fn rollout_filters(pct: u8) -> Value {
    json!({ "groups": [{ "rollout_percentage": pct }] })
}

async fn resolve_key_to_id(client: &Client, pid: &str, key: &str) -> Result<i64> {
    // Check the Phase B cache, if exposed. The shared Client struct field `cache`
    // in the spec signature is assumed to carry name->id. If the phase B agent
    // exposes it via e.g. `client.cache().flag_id(key)`, use that first. Otherwise
    // fall through to the list lookup every time. v1 cache is in-memory per-proc.
    // (The lookup below is tolerated as the MVP behavior.)
    let path = format!(
        "/api/projects/{pid}/feature_flags/?search={}",
        urlencoding::encode(key)
    );
    let hits: Vec<Flag> = client.get_paginated(&path, Some(50)).await?;
    hits.into_iter()
        .find(|f| f.key == key)
        .map(|f| f.id)
        .ok_or_else(|| BosshoggError::NotFound(format!("flag '{key}'")))
}

async fn get_flag(cx: &CommandContext, args: GetArgs) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;
    let id = resolve_key_to_id(client, pid, &args.key).await?;
    let flag: Flag = client
        .get(&format!("/api/projects/{pid}/feature_flags/{id}/"))
        .await?;
    if cx.is_json() {
        output::print_json(&flag);
    } else {
        println!("ID:          {}", flag.id);
        println!("Key:         {}", flag.key);
        if let Some(n) = flag.name.as_deref() {
            println!("Name:        {n}");
        }
        println!("Active:      {}", flag.active);
        if let Some(r) = flag.rollout_percentage {
            println!("Rollout:     {r}%");
        }
        if !flag.tags.is_empty() {
            println!("Tags:        {}", flag.tags.join(", "));
        }
        if let Some(d) = flag.description.as_deref() {
            println!("Description: {d}");
        }
    }
    Ok(())
}

async fn create_flag(cx: &CommandContext, args: CreateArgs) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;
    let filters = if let Some(p) = args.filters_file.as_deref() {
        read_json_file(p).await?
    } else {
        json!({ "groups": [{ "rollout_percentage": 0 }] })
    };
    let payload = if let Some(p) = args.payload_file.as_deref() {
        Some(read_json_file(p).await?)
    } else {
        None
    };

    let mut body = serde_json::json!({
        "key": args.key,
        "name": args.name,
        "active": args.active,
        "filters": filters,
    });
    if let Some(d) = args.description.as_deref() {
        body["description"] = Value::String(d.into());
    }
    if let Some(p) = payload {
        body["payloads"] = p;
    }

    let created: Flag = client
        .post(&format!("/api/projects/{pid}/feature_flags/"), &body)
        .await?;

    if cx.is_json() {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            action: &'a str,
            id: i64,
            key: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            key: &created.key,
        });
    } else {
        println!("Created flag '{}' (id {})", created.key, created.id);
    }
    Ok(())
}

async fn update_flag(cx: &CommandContext, args: UpdateArgs) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;
    let id = resolve_key_to_id(client, pid, &args.key).await?;

    let mut body = serde_json::Map::new();
    if args.enabled {
        body.insert("active".into(), Value::Bool(true));
    }
    if args.disabled {
        body.insert("active".into(), Value::Bool(false));
    }
    if let Some(pct) = args.rollout {
        if pct > 100 {
            return Err(BosshoggError::BadRequest(format!(
                "--rollout must be 0-100 (got {pct})"
            )));
        }
        body.insert("filters".into(), rollout_filters(pct));
    }
    if let Some(p) = args.filters_file.as_deref() {
        body.insert("filters".into(), read_json_file(p).await?);
    }
    if let Some(p) = args.payload_file.as_deref() {
        body.insert("payloads".into(), read_json_file(p).await?);
    }
    if let Some(n) = args.name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(d) = args.description {
        body.insert("description".into(), Value::String(d));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --enabled, --rollout N, --name, ...)".into(),
        ));
    }

    // Build a human-readable summary of what's changing for the confirm prompt.
    let changes: Vec<String> = body.iter().map(|(k, v)| format!("{k}={v}")).collect();
    let changes_str = changes.join(", ");

    cx.confirm(&format!(
        "update flag `{}` with these changes: {}; continue?",
        args.key, changes_str
    ))?;

    let updated: Flag = client
        .patch(
            &format!("/api/projects/{pid}/feature_flags/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.is_json() {
        output::print_json(&updated);
    } else {
        println!("Updated flag '{}' (id {})", updated.key, updated.id);
    }
    Ok(())
}

async fn delete_flag(cx: &CommandContext, key: String) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;

    cx.confirm(&format!("soft-delete flag `{key}`; continue?"))?;

    let id = resolve_key_to_id(client, pid, &key).await?;
    client
        .delete(&format!("/api/projects/{pid}/feature_flags/{id}/"))
        .await?;
    if cx.is_json() {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            action: &'a str,
            deleted: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            action: "delete",
            deleted: &key,
        });
    } else {
        println!("Deleted flag '{key}'");
    }
    Ok(())
}

async fn enable_flag(cx: &CommandContext, key: String) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;

    cx.confirm(&format!("enable flag `{key}`; continue?"))?;

    let id = resolve_key_to_id(client, pid, &key).await?;
    let updated: Flag = client
        .patch(
            &format!("/api/projects/{pid}/feature_flags/{id}/"),
            &json!({ "active": true }),
        )
        .await?;
    if cx.is_json() {
        output::print_json(&updated);
    } else {
        println!("Enabled flag '{}'", updated.key);
    }
    Ok(())
}

async fn disable_flag(cx: &CommandContext, key: String) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;

    cx.confirm(&format!(
        "disable flag `{key}` (all users fall back to default); continue?"
    ))?;

    let id = resolve_key_to_id(client, pid, &key).await?;
    let updated: Flag = client
        .patch(
            &format!("/api/projects/{pid}/feature_flags/{id}/"),
            &json!({ "active": false }),
        )
        .await?;
    if cx.is_json() {
        output::print_json(&updated);
    } else {
        println!("Disabled flag '{}'", updated.key);
    }
    Ok(())
}

async fn rollout_flag(cx: &CommandContext, key: String, percent: u8) -> Result<()> {
    if percent > 100 {
        return Err(BosshoggError::BadRequest(format!(
            "rollout must be 0-100 (got {percent})"
        )));
    }
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;

    cx.confirm(&format!(
        "change rollout of flag `{key}` to {percent}%; continue?"
    ))?;

    let id = resolve_key_to_id(client, pid, &key).await?;
    let updated: Flag = client
        .patch(
            &format!("/api/projects/{pid}/feature_flags/{id}/"),
            &json!({ "filters": rollout_filters(percent) }),
        )
        .await?;
    if cx.is_json() {
        output::print_json(&updated);
    } else {
        println!("Set rollout for '{}' to {}%", updated.key, percent);
    }
    Ok(())
}

async fn evaluate_flags(cx: &CommandContext, args: EvaluateArgs) -> Result<()> {
    let client = &cx.client;
    let token = args.project_token.ok_or_else(|| {
        BosshoggError::BadRequest(
            "flag evaluate requires --project-token (phc_...) — distinct from personal phx_ key"
                .into(),
        )
    })?;

    let mut body = serde_json::Map::new();
    body.insert("api_key".into(), Value::String(token));
    body.insert("distinct_id".into(), Value::String(args.distinct_id));

    // --groups "type=key" pairs → {type: key}
    if !args.groups.is_empty() {
        let mut groups = serde_json::Map::new();
        for g in &args.groups {
            let (k, v) = g.split_once('=').ok_or_else(|| {
                BosshoggError::BadRequest(format!("--groups expects type=key (got '{g}')"))
            })?;
            groups.insert(k.into(), Value::String(v.into()));
        }
        body.insert("groups".into(), Value::Object(groups));
    }

    if let Some(p) = args.person_props.as_deref() {
        body.insert("person_properties".into(), read_json_file(p).await?);
    }

    // POST to /flags?v=2 on the private host of the active context.
    // This path bypasses /api/... prefix; Client::post uses the host as-is.
    let resp: Value = client.post("/flags?v=2", &Value::Object(body)).await?;
    if cx.is_json() {
        output::print_json(&resp);
    } else if let Some(flags) = resp.get("featureFlags").and_then(Value::as_object) {
        for (k, v) in flags {
            println!("{:<40} {}", k, v);
        }
    } else {
        println!("{}", resp);
    }
    Ok(())
}

async fn dependents(cx: &CommandContext, key: String) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;
    let id = resolve_key_to_id(client, pid, &key).await?;
    let v: Value = client
        .get(&format!(
            "/api/projects/{pid}/feature_flags/{id}/dependent_flags/"
        ))
        .await?;
    if cx.is_json() {
        output::print_json(&v);
    } else if let Some(arr) = v.as_array() {
        for d in arr {
            let k = d.get("key").and_then(Value::as_str).unwrap_or("-");
            let id = d.get("id").map(|x| x.to_string()).unwrap_or_default();
            println!("{id:<6} {k}");
        }
    }
    Ok(())
}

async fn activity(cx: &CommandContext, key: String) -> Result<()> {
    let client = &cx.client;
    let pid = client
        .project_id()
        .ok_or_else(|| BosshoggError::Config("no project_id".into()))?;
    let id = resolve_key_to_id(client, pid, &key).await?;
    let v: Value = client
        .get(&format!("/api/projects/{pid}/feature_flags/{id}/activity/"))
        .await?;
    if cx.is_json() {
        output::print_json(&v);
    } else if let Some(results) = v.get("results").and_then(Value::as_array) {
        for e in results {
            let a = e.get("activity").and_then(Value::as_str).unwrap_or("-");
            let t = e.get("created_at").and_then(Value::as_str).unwrap_or("-");
            let u = e
                .pointer("/user/email")
                .and_then(Value::as_str)
                .unwrap_or("-");
            println!("{t}  {a:<10}  {u}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_roundtrip_preserves_unknown_fields() {
        let raw = r#"{
            "id": 1, "key": "k", "name": "n", "active": true,
            "filters": { "groups": [] }, "rollout_percentage": 25,
            "vibe": "unknown-future-field"
        }"#;
        let f: Flag = serde_json::from_str(raw).unwrap();
        assert_eq!(f.id, 1);
        assert_eq!(f.rollout_percentage, Some(25));
        let back = serde_json::to_value(&f).unwrap();
        assert_eq!(
            back.get("vibe").and_then(|v| v.as_str()),
            Some("unknown-future-field")
        );
    }
}
