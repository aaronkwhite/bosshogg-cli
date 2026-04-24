// src/commands/dashboard.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::commands::context::CommandContext;
use crate::commands::util::{env_id_required, read_json_file, read_text_file};
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Dashboard {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub is_shared: Option<bool>,
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub creation_mode: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tiles: Option<Value>,
    #[serde(default)]
    pub filters: Value,
    #[serde(default)]
    pub variables: Option<Value>,
    #[serde(default)]
    pub restriction_level: Option<i32>,
    #[serde(default)]
    pub effective_privilege_level: Option<i32>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct DashboardArgs {
    #[command(subcommand)]
    pub command: DashboardCommand,
}

#[derive(Subcommand, Debug)]
pub enum DashboardCommand {
    /// List dashboards with optional filters.
    List {
        #[arg(long)]
        tag: Option<String>,
        #[arg(long)]
        search: Option<String>,
    },
    /// Get a single dashboard by numeric id.
    Get { id: i64 },
    /// Trigger insight refresh for all tiles on a dashboard.
    Refresh { id: i64 },
    /// Create a new dashboard.
    Create {
        #[arg(long)]
        name: String,
        /// Path to a text file containing the description.
        #[arg(long)]
        description_file: Option<PathBuf>,
        /// Path to a JSON file containing the filters object.
        #[arg(long)]
        filters_file: Option<PathBuf>,
    },
    /// Update dashboard fields (name, description, tags, etc.).
    Update {
        id: i64,
        #[arg(long)]
        name: Option<String>,
        /// Path to a text file containing the new description.
        #[arg(long)]
        description_file: Option<PathBuf>,
        /// Add a tag (can be repeated).
        #[arg(long)]
        tag: Vec<String>,
    },
    /// Soft-delete a dashboard (PATCH deleted=true).
    Delete { id: i64 },
    /// Manage tiles on a dashboard.
    #[command(subcommand)]
    Tiles(TilesCommand),
    /// View sharing settings for a dashboard (read-only in M3).
    Share { id: i64 },
}

#[derive(Subcommand, Debug)]
pub enum TilesCommand {
    /// Add an insight as a tile to a dashboard.
    Add {
        dashboard_id: i64,
        /// Insight id to add as a tile.
        #[arg(long)]
        insight: i64,
    },
    /// Remove a tile from a dashboard.
    Remove {
        dashboard_id: i64,
        /// Tile id to remove.
        #[arg(long)]
        tile: i64,
    },
    /// Move (reposition) a tile on a dashboard.
    Move {
        dashboard_id: i64,
        /// Tile id to move.
        #[arg(long)]
        tile: i64,
        /// New position as a JSON object (e.g. '{"x":0,"y":0,"w":6,"h":5}').
        #[arg(long)]
        position: String,
    },
    /// Copy a tile to another dashboard.
    Copy {
        dashboard_id: i64,
        /// Tile id to copy.
        #[arg(long)]
        tile: i64,
        /// Destination dashboard id.
        #[arg(long, name = "to-dashboard")]
        to_dashboard: i64,
    },
    /// Reorder all tiles on a dashboard from a JSON file.
    Reorder {
        dashboard_id: i64,
        /// Path to a JSON file containing the ordered tile id list.
        #[arg(long, name = "order-file")]
        order_file: PathBuf,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: DashboardArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        DashboardCommand::List { tag, search } => list_dashboards(cx, tag, search).await,
        DashboardCommand::Get { id } => get_dashboard(cx, id).await,
        DashboardCommand::Refresh { id } => refresh_dashboard(cx, id).await,
        DashboardCommand::Create {
            name,
            description_file,
            filters_file,
        } => create_dashboard(cx, name, description_file, filters_file).await,
        DashboardCommand::Update {
            id,
            name,
            description_file,
            tag,
        } => update_dashboard(cx, id, name, description_file, tag).await,
        DashboardCommand::Delete { id } => delete_dashboard(cx, id).await,
        DashboardCommand::Tiles(tiles_cmd) => dispatch_tiles(cx, tiles_cmd).await,
        DashboardCommand::Share { id } => share_dashboard(cx, id).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Dashboard>,
}

async fn list_dashboards(
    cx: &CommandContext,
    tag: Option<String>,
    search: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut qs: Vec<(String, String)> = Vec::new();
    if let Some(t) = tag {
        qs.push(("tags".into(), t));
    }
    if let Some(s) = search {
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

    let path = format!("/api/environments/{env_id}/dashboards/{query}");
    let results: Vec<Dashboard> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "PINNED", "TAGS", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|d| {
                vec![
                    d.id.to_string(),
                    d.name.clone(),
                    d.pinned
                        .map(|p| p.to_string())
                        .unwrap_or_else(|| "-".into()),
                    d.tags.join(","),
                    d.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_dashboard(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let dashboard: Dashboard = client
        .get(&format!("/api/environments/{env_id}/dashboards/{id}/"))
        .await?;
    print_dashboard(&dashboard, cx.json_mode);
    Ok(())
}

// ── refresh ───────────────────────────────────────────────────────────────────

async fn refresh_dashboard(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/dashboards/{id}/run_insights/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Triggered insight refresh for dashboard {id}");
    }
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_dashboard(
    cx: &CommandContext,
    name: String,
    description_file: Option<PathBuf>,
    filters_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = json!({ "name": name });

    if let Some(p) = description_file.as_deref() {
        let desc = read_text_file(p).await?;
        body["description"] = Value::String(desc.trim().to_string());
    }
    if let Some(p) = filters_file.as_deref() {
        body["filters"] = read_json_file(p).await?;
    }

    let created: Dashboard = client
        .post(&format!("/api/environments/{env_id}/dashboards/"), &body)
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
        println!("Created dashboard '{}' (id {})", created.name, created.id);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_dashboard(
    cx: &CommandContext,
    id: i64,
    name: Option<String>,
    description_file: Option<PathBuf>,
    tag: Vec<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(p) = description_file.as_deref() {
        let desc = read_text_file(p).await?;
        body.insert("description".into(), Value::String(desc.trim().to_string()));
    }
    if !tag.is_empty() {
        let tag_vals: Vec<Value> = tag.iter().map(|t| Value::String(t.clone())).collect();
        body.insert("tags".into(), Value::Array(tag_vals));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --description-file, --tag)".into(),
        ));
    }

    cx.confirm(&format!("update dashboard `{id}`; continue?"))?;

    let updated: Dashboard = client
        .patch(
            &format!("/api/environments/{env_id}/dashboards/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated dashboard '{}' (id {})", updated.name, updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_dashboard(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("soft-delete dashboard `{id}`; continue?"))?;

    client
        .delete(&format!("/api/environments/{env_id}/dashboards/{id}/"))
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
        println!("Deleted dashboard {id}");
    }
    Ok(())
}

// ── tiles dispatch ────────────────────────────────────────────────────────────

async fn dispatch_tiles(cx: &CommandContext, cmd: TilesCommand) -> Result<()> {
    match cmd {
        TilesCommand::Add {
            dashboard_id,
            insight,
        } => {
            cx.confirm(&format!(
                "add insight {insight} to dashboard {dashboard_id}; continue?"
            ))?;
            tiles_add(cx, dashboard_id, insight).await
        }
        TilesCommand::Remove { dashboard_id, tile } => {
            cx.confirm(&format!(
                "remove tile {tile} from dashboard {dashboard_id}; continue?"
            ))?;
            tiles_remove(cx, dashboard_id, tile).await
        }
        TilesCommand::Move {
            dashboard_id,
            tile,
            position,
        } => {
            cx.confirm(&format!(
                "move tile {tile} on dashboard {dashboard_id}; continue?"
            ))?;
            tiles_move(cx, dashboard_id, tile, position).await
        }
        TilesCommand::Copy {
            dashboard_id,
            tile,
            to_dashboard,
        } => {
            cx.confirm(&format!(
                "copy tile {tile} from dashboard {dashboard_id} to dashboard {to_dashboard}; continue?"
            ))?;
            tiles_copy(cx, dashboard_id, tile, to_dashboard).await
        }
        TilesCommand::Reorder {
            dashboard_id,
            order_file,
        } => {
            cx.confirm(&format!(
                "reorder tiles on dashboard {dashboard_id}; continue?"
            ))?;
            tiles_reorder(cx, dashboard_id, &order_file).await
        }
    }
}

// ── tiles add ─────────────────────────────────────────────────────────────────

async fn tiles_add(cx: &CommandContext, dashboard_id: i64, insight: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    // Modern PostHog attaches insights to dashboards by PATCHing the
    // INSIGHT with an updated `dashboards` array. The legacy
    // `PATCH /dashboards/{id}/ {"tiles": [...]}` path is silently dropped
    // on current accounts — the response returns 200 but `tiles` stays empty.
    let insight_path = format!("/api/environments/{env_id}/insights/{insight}/");

    let current: Value = client.get(&insight_path).await?;
    let mut dashboards: Vec<i64> = current
        .get("dashboards")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
        .unwrap_or_default();

    if !dashboards.contains(&dashboard_id) {
        dashboards.push(dashboard_id);
    }

    let v: Value = client
        .patch(&insight_path, &json!({ "dashboards": dashboards }))
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Added insight {insight} to dashboard {dashboard_id}");
    }
    Ok(())
}

// ── tiles remove ──────────────────────────────────────────────────────────────

async fn tiles_remove(cx: &CommandContext, dashboard_id: i64, tile: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    // Symmetric with tiles_add: modern PostHog detaches an insight from a
    // dashboard by PATCHing the INSIGHT with `dashboards` minus this
    // dashboard_id. The legacy `PATCH /dashboards/{id}/ {"tiles": [...]}`
    // path is silently dropped on current accounts.
    //
    // Since the input is a tile_id (not insight_id), we first GET the
    // dashboard to find the tile's insight.
    let dash_path = format!("/api/environments/{env_id}/dashboards/{dashboard_id}/");
    let dashboard: Dashboard = client.get(&dash_path).await?;
    let insight_id = dashboard
        .tiles
        .as_ref()
        .and_then(|t| t.as_array())
        .and_then(|arr| {
            arr.iter()
                .find(|t| t.get("id").and_then(Value::as_i64) == Some(tile))
                .and_then(|t| t.get("insight"))
                .and_then(|i| i.get("id"))
                .and_then(Value::as_i64)
        })
        .ok_or_else(|| {
            BosshoggError::NotFound(format!("tile {tile} on dashboard {dashboard_id}"))
        })?;

    let insight_path = format!("/api/environments/{env_id}/insights/{insight_id}/");
    let current: Value = client.get(&insight_path).await?;
    let dashboards: Vec<i64> = current
        .get("dashboards")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_i64())
                .filter(|id| *id != dashboard_id)
                .collect()
        })
        .unwrap_or_default();

    let v: Value = client
        .patch(&insight_path, &json!({ "dashboards": dashboards }))
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Removed tile {tile} from dashboard {dashboard_id}");
    }
    Ok(())
}

// ── tiles move ────────────────────────────────────────────────────────────────

async fn tiles_move(
    cx: &CommandContext,
    dashboard_id: i64,
    tile: i64,
    position: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    // Modern PostHog exposes a dedicated `PATCH /dashboards/{id}/move_tile/`
    // endpoint. The legacy `PATCH /dashboards/{id}/ {"tiles": [...]}` path
    // is silently dropped on current accounts.
    //
    // NOTE: `move_tile` currently requires session-cookie auth and will
    // return 403 "does not support Personal API Key access" on PATCH via
    // personal API key. Hitting the correct endpoint surfaces this
    // limitation cleanly rather than pretending a silent-drop succeeded.
    let pos: Value = serde_json::from_str(&position)
        .map_err(|e| BosshoggError::BadRequest(format!("position is not valid JSON: {e}")))?;

    let path = format!("/api/environments/{env_id}/dashboards/{dashboard_id}/move_tile/");
    let body = json!({
        "tile": { "id": tile, "layouts": { "sm": pos } }
    });
    let v: Value = client.patch(&path, &body).await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Moved tile {tile} on dashboard {dashboard_id}");
    }
    Ok(())
}

// ── tiles copy ────────────────────────────────────────────────────────────────

async fn tiles_copy(
    cx: &CommandContext,
    dashboard_id: i64,
    tile: i64,
    to_dashboard: i64,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    // Modern PostHog: `POST /dashboards/{id}/copy_tile/` with
    // `{fromDashboardId, tileId}`. The `id` in the path is the destination.
    //
    // NOTE: like `move_tile`, `copy_tile` currently requires session-cookie
    // auth and returns 403 for Personal API Key access.
    let path = format!("/api/environments/{env_id}/dashboards/{to_dashboard}/copy_tile/");
    let body = json!({
        "fromDashboardId": dashboard_id,
        "tileId": tile
    });
    let v: Value = client.post(&path, &body).await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Copied tile {tile} from dashboard {dashboard_id} to dashboard {to_dashboard}");
    }
    Ok(())
}

// ── tiles reorder ─────────────────────────────────────────────────────────────

async fn tiles_reorder(
    cx: &CommandContext,
    dashboard_id: i64,
    order_file: &std::path::Path,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    // Modern PostHog: `POST /dashboards/{id}/reorder_tiles/` with
    // `{tile_order: [id1, id2, ...]}`.
    //
    // NOTE: `reorder_tiles` requires session-cookie auth and returns 403
    // for Personal API Key access (same as `move_tile`/`copy_tile`).
    //
    // Accepts either a bare array of ids or an array of objects with `id`
    // fields (back-compat with prior file format).
    let order_value = read_json_file(order_file).await?;
    let ordered_ids: Vec<i64> = order_value
        .as_array()
        .ok_or_else(|| BosshoggError::BadRequest("order-file must contain a JSON array".into()))?
        .iter()
        .map(|v| {
            v.as_i64()
                .or_else(|| v.get("id").and_then(Value::as_i64))
                .ok_or_else(|| {
                    BosshoggError::BadRequest(
                        "each order entry must be an integer tile id or an object with an \"id\" field".into(),
                    )
                })
        })
        .collect::<Result<Vec<_>>>()?;

    let path = format!("/api/environments/{env_id}/dashboards/{dashboard_id}/reorder_tiles/");
    let v: Value = client
        .post(&path, &json!({ "tile_order": ordered_ids }))
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Reordered tiles on dashboard {dashboard_id}");
    }
    Ok(())
}

// ── share ─────────────────────────────────────────────────────────────────────

async fn share_dashboard(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/dashboards/{id}/sharing/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        let enabled = v.get("enabled").and_then(Value::as_bool).unwrap_or(false);
        let token = v
            .get("access_token")
            .and_then(Value::as_str)
            .unwrap_or("<none>");
        println!("Sharing enabled: {enabled}");
        println!("Access token:    {token}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_dashboard(dashboard: &Dashboard, json_mode: bool) {
    if json_mode {
        output::print_json(dashboard);
    } else {
        println!("ID:           {}", dashboard.id);
        println!("Name:         {}", dashboard.name);
        if let Some(d) = dashboard.description.as_deref() {
            println!("Description:  {d}");
        }
        println!("Pinned:       {}", dashboard.pinned.unwrap_or(false));
        if !dashboard.tags.is_empty() {
            println!("Tags:         {}", dashboard.tags.join(", "));
        }
        if let Some(ca) = dashboard.created_at.as_deref() {
            println!("Created:      {ca}");
        }
        println!("Deleted:      {}", dashboard.deleted);
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashboard_roundtrip_minimal() {
        let raw = r#"{
            "id": 1,
            "name": "My Dashboard",
            "deleted": false,
            "tags": [],
            "filters": {}
        }"#;
        let d: Dashboard = serde_json::from_str(raw).unwrap();
        assert_eq!(d.id, 1);
        assert_eq!(d.name, "My Dashboard");
        assert!(!d.deleted);
        assert!(d.tags.is_empty());
    }

    #[test]
    fn dashboard_roundtrip_full() {
        let raw = r#"{
            "id": 42,
            "name": "Analytics",
            "description": "Main analytics dashboard",
            "pinned": true,
            "created_at": "2026-01-01T00:00:00Z",
            "created_by": {"id": 1, "email": "test@example.com"},
            "is_shared": false,
            "deleted": false,
            "creation_mode": "default",
            "tags": ["prod", "analytics"],
            "tiles": [{"id": 1}],
            "filters": {"date_from": "-7d"},
            "variables": null,
            "restriction_level": 21,
            "effective_privilege_level": 21
        }"#;
        let d: Dashboard = serde_json::from_str(raw).unwrap();
        assert_eq!(d.id, 42);
        assert_eq!(d.name, "Analytics");
        assert_eq!(d.tags, vec!["prod", "analytics"]);
        assert_eq!(d.restriction_level, Some(21));
        assert!(d.pinned.unwrap_or(false));
    }
}
