// src/commands/session_recording_playlist.rs
//! `bosshogg session-recording-playlist` — full CRUD + recordings sub-resource.
//!
//! Playlists are project-scoped.
//! Path: `/api/projects/{project_id}/session_recording_playlists/`
//!
//! The OpenAPI spec uses `{short_id}` (not `{id}`) in detail-path segments.
//! The `recordings` sub-path is a GET that returns no typed body in the spec
//! (the server streams the recording list); we capture the raw Value.
//!
//! DELETE on the playlist itself returns 204 (hard delete).
//! The `deleted` field on the model is a soft-delete flag settable via PATCH;
//! since DELETE is available we use it directly.

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
pub struct SessionRecordingPlaylist {
    pub id: i64,
    pub short_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub derived_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub filters: Option<Value>,
    #[serde(rename = "type", default)]
    pub playlist_type: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub last_modified_at: Option<String>,
    #[serde(default)]
    pub recordings_counts: Option<Value>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct SessionRecordingPlaylistArgs {
    #[command(subcommand)]
    pub command: SessionRecordingPlaylistCommand,
}

#[derive(Subcommand, Debug)]
pub enum SessionRecordingPlaylistCommand {
    /// List session recording playlists.
    List {
        /// Maximum number of results to return.
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single playlist by short_id.
    Get { short_id: String },
    /// Create a new session recording playlist.
    Create {
        /// Human-readable name for the playlist.
        #[arg(long)]
        name: String,
        /// Optional description.
        #[arg(long)]
        description: Option<String>,
        /// Path to a JSON file with recording filter criteria.
        #[arg(long)]
        filters_file: Option<PathBuf>,
    },
    /// Update a session recording playlist.
    Update {
        short_id: String,
        /// New name.
        #[arg(long)]
        name: Option<String>,
        /// New description.
        #[arg(long)]
        description: Option<String>,
        /// Path to a JSON file with updated recording filter criteria.
        #[arg(long)]
        filters_file: Option<PathBuf>,
    },
    /// Delete a session recording playlist (hard DELETE — returns 204).
    Delete { short_id: String },
    /// List recordings in a playlist.
    Recordings { short_id: String },
    /// Add a session recording to a playlist.
    #[command(name = "add-recording")]
    AddRecording {
        short_id: String,
        /// Session recording ID to add.
        #[arg(long)]
        session_id: String,
    },
    /// Remove a session recording from a playlist.
    #[command(name = "remove-recording")]
    RemoveRecording {
        short_id: String,
        /// Session recording ID to remove.
        #[arg(long)]
        session_id: String,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: SessionRecordingPlaylistArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        SessionRecordingPlaylistCommand::List { limit } => list_playlists(cx, limit).await,
        SessionRecordingPlaylistCommand::Get { short_id } => get_playlist(cx, short_id).await,
        SessionRecordingPlaylistCommand::Create {
            name,
            description,
            filters_file,
        } => create_playlist(cx, name, description, filters_file).await,
        SessionRecordingPlaylistCommand::Update {
            short_id,
            name,
            description,
            filters_file,
        } => update_playlist(cx, short_id, name, description, filters_file).await,
        SessionRecordingPlaylistCommand::Delete { short_id } => delete_playlist(cx, short_id).await,
        SessionRecordingPlaylistCommand::Recordings { short_id } => {
            list_recordings(cx, short_id).await
        }
        SessionRecordingPlaylistCommand::AddRecording {
            short_id,
            session_id,
        } => add_recording(cx, short_id, session_id).await,
        SessionRecordingPlaylistCommand::RemoveRecording {
            short_id,
            session_id,
        } => remove_recording(cx, short_id, session_id).await,
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
    results: Vec<SessionRecordingPlaylist>,
}

async fn list_playlists(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path = format!("/api/projects/{project_id}/session_recording_playlists/");
    let results: Vec<SessionRecordingPlaylist> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["SHORT_ID", "NAME", "PINNED", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|p| {
                vec![
                    p.short_id.clone(),
                    p.name
                        .clone()
                        .or_else(|| p.derived_name.clone())
                        .unwrap_or_else(|| "-".into()),
                    p.pinned
                        .map(|v| if v { "yes" } else { "no" })
                        .unwrap_or("-")
                        .to_string(),
                    p.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_playlist(cx: &CommandContext, short_id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let playlist: SessionRecordingPlaylist = client
        .get(&format!(
            "/api/projects/{project_id}/session_recording_playlists/{short_id}/"
        ))
        .await?;
    print_playlist(&playlist, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_playlist(
    cx: &CommandContext,
    name: String,
    description: Option<String>,
    filters_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    body.insert("name".into(), Value::String(name));
    if let Some(d) = description {
        body.insert("description".into(), Value::String(d));
    }
    if let Some(path) = filters_file {
        let filters = read_json_file(&path).await?;
        body.insert("filters".into(), filters);
    }

    cx.confirm("create session recording playlist; continue?")?;

    let created: SessionRecordingPlaylist = client
        .post(
            &format!("/api/projects/{project_id}/session_recording_playlists/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            short_id: String,
            name: Option<String>,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            short_id: created.short_id,
            name: created.name,
        });
    } else {
        println!(
            "Created playlist '{}' (short_id {})",
            created.name.as_deref().unwrap_or("-"),
            created.short_id
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_playlist(
    cx: &CommandContext,
    short_id: String,
    name: Option<String>,
    description: Option<String>,
    filters_file: Option<PathBuf>,
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
    if let Some(path) = filters_file {
        let filters = read_json_file(&path).await?;
        body.insert("filters".into(), filters);
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --description, --filters-file)".into(),
        ));
    }

    cx.confirm(&format!("update playlist `{short_id}`; continue?"))?;

    let updated: SessionRecordingPlaylist = client
        .patch(
            &format!("/api/projects/{project_id}/session_recording_playlists/{short_id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated playlist '{}' (short_id {})",
            updated.name.as_deref().unwrap_or("-"),
            updated.short_id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_playlist(cx: &CommandContext, short_id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("delete playlist `{short_id}`; continue?"))?;

    client
        .delete(&format!(
            "/api/projects/{project_id}/session_recording_playlists/{short_id}/"
        ))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            short_id: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "delete",
            short_id,
        });
    } else {
        println!("Deleted playlist {short_id}");
    }
    Ok(())
}

// ── recordings ────────────────────────────────────────────────────────────────

/// List recordings in a playlist.
///
/// The PostHog spec defines this as a GET with no typed response body
/// (`200: No response body`). We capture the raw JSON Value and emit it.
async fn list_recordings(cx: &CommandContext, short_id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path =
        format!("/api/projects/{project_id}/session_recording_playlists/{short_id}/recordings/");
    let result: Value = client.get(&path).await?;

    if cx.json_mode {
        output::print_json(&result);
    } else {
        // Best-effort table: the body structure is undocumented in the spec.
        println!(
            "{}",
            serde_json::to_string_pretty(&result).unwrap_or_default()
        );
    }
    Ok(())
}

// ── add-recording ─────────────────────────────────────────────────────────────

async fn add_recording(cx: &CommandContext, short_id: String, session_id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "add recording `{session_id}` to playlist `{short_id}`; continue?"
    ))?;

    // POST body is the playlist shape but in practice the server ignores it for
    // this sub-resource. Send an empty object to satisfy Content-Type.
    let _: Value = client
        .post(
            &format!(
                "/api/projects/{project_id}/session_recording_playlists/{short_id}/recordings/{session_id}/"
            ),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            short_id: String,
            session_id: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "add-recording",
            short_id,
            session_id,
        });
    } else {
        println!("Added recording {session_id} to playlist {short_id}");
    }
    Ok(())
}

// ── remove-recording ──────────────────────────────────────────────────────────

async fn remove_recording(cx: &CommandContext, short_id: String, session_id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "remove recording `{session_id}` from playlist `{short_id}`; continue?"
    ))?;

    client
        .delete(&format!(
            "/api/projects/{project_id}/session_recording_playlists/{short_id}/recordings/{session_id}/"
        ))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            short_id: String,
            session_id: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "remove-recording",
            short_id,
            session_id,
        });
    } else {
        println!("Removed recording {session_id} from playlist {short_id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_playlist(p: &SessionRecordingPlaylist, json_mode: bool) {
    if json_mode {
        output::print_json(p);
    } else {
        println!("ID:            {}", p.id);
        println!("Short ID:      {}", p.short_id);
        let display_name = p
            .name
            .as_deref()
            .or(p.derived_name.as_deref())
            .unwrap_or("-");
        println!("Name:          {display_name}");
        if let Some(d) = p.description.as_deref() {
            println!("Description:   {d}");
        }
        if let Some(pinned) = p.pinned {
            println!("Pinned:        {pinned}");
        }
        if let Some(ca) = p.created_at.as_deref() {
            println!("Created:       {ca}");
        }
        if let Some(lm) = p.last_modified_at.as_deref() {
            println!("Last modified: {lm}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playlist_roundtrip_minimal() {
        let raw = r#"{
            "id": 42,
            "short_id": "AbCd1234",
            "name": "My Playlist",
            "created_at": "2026-01-01T00:00:00Z",
            "created_by": null,
            "last_modified_at": "2026-01-02T00:00:00Z",
            "last_modified_by": null,
            "recordings_counts": {}
        }"#;
        let p: SessionRecordingPlaylist = serde_json::from_str(raw).unwrap();
        assert_eq!(p.id, 42);
        assert_eq!(p.short_id, "AbCd1234");
        assert_eq!(p.name.as_deref(), Some("My Playlist"));
    }

    #[test]
    fn playlist_roundtrip_full() {
        let raw = r#"{
            "id": 99,
            "short_id": "XyZ9",
            "name": null,
            "derived_name": "Checkout Funnel",
            "description": "Recordings of checkout flow",
            "pinned": true,
            "deleted": false,
            "type": "filters",
            "filters": {"events": [{"id": "$pageview"}]},
            "created_at": "2026-02-01T00:00:00Z",
            "created_by": {"id": 1, "email": "test@example.com"},
            "last_modified_at": "2026-03-01T00:00:00Z",
            "last_modified_by": null,
            "recordings_counts": {"total": 13}
        }"#;
        let p: SessionRecordingPlaylist = serde_json::from_str(raw).unwrap();
        assert_eq!(p.short_id, "XyZ9");
        assert_eq!(p.derived_name.as_deref(), Some("Checkout Funnel"));
        assert_eq!(p.pinned, Some(true));
        assert!(p.filters.is_some());
    }
}
