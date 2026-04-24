// src/commands/role.rs
//! `bosshogg role` — list / get / create / update / delete / members /
//! add-member / remove-member.
//!
//! Roles are organization-scoped (not project/env scoped).
//! Uses `client.org_id()` — errors if not configured.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Role {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub feature_flags_access_level: Option<i32>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub members: Option<Vec<Value>>,
    #[serde(default)]
    pub associated_flags: Option<Vec<Value>>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct RoleArgs {
    #[command(subcommand)]
    pub command: RoleCommand,
}

#[derive(Subcommand, Debug)]
pub enum RoleCommand {
    /// List all roles.
    List,
    /// Get a single role by ID.
    Get { id: String },
    /// Create a new role.
    Create {
        #[arg(long)]
        name: String,
        /// Feature flags access level (integer; check PostHog docs for values).
        #[arg(long)]
        feature_flags_access_level: Option<i32>,
    },
    /// Update a role.
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
    },
    /// Delete a role (hard DELETE).
    Delete { id: String },
    /// List members of a role.
    Members { id: String },
    /// Add a user to a role.
    #[command(name = "add-member")]
    AddMember {
        id: String,
        /// User UUID to add.
        #[arg(long)]
        user_id: String,
    },
    /// Remove a user from a role by membership ID.
    #[command(name = "remove-member")]
    RemoveMember {
        id: String,
        /// Role membership ID (from `bosshogg role members`).
        #[arg(long)]
        membership_id: String,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: RoleArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        RoleCommand::List => list_roles(cx).await,
        RoleCommand::Get { id } => get_role(cx, id).await,
        RoleCommand::Create {
            name,
            feature_flags_access_level,
        } => create_role(cx, name, feature_flags_access_level).await,
        RoleCommand::Update { id, name } => update_role(cx, id, name).await,
        RoleCommand::Delete { id } => delete_role(cx, id).await,
        RoleCommand::Members { id } => list_members(cx, id).await,
        RoleCommand::AddMember { id, user_id } => add_member(cx, id, user_id).await,
        RoleCommand::RemoveMember { id, membership_id } => {
            remove_member(cx, id, membership_id).await
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn org_id_required(client: &Client) -> Result<&str> {
    client.org_id().ok_or_else(|| {
        BosshoggError::Config(
            "no org_id configured; run `bosshogg configure` or set POSTHOG_CLI_ORG_ID".into(),
        )
    })
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Role>,
}

async fn list_roles(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;
    let path = format!("/api/organizations/{org_id}/roles/");
    let results: Vec<Role> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "FF_ACCESS_LEVEL", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.name.clone(),
                    r.feature_flags_access_level
                        .map(|l| l.to_string())
                        .unwrap_or_else(|| "-".into()),
                    r.created_at.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_role(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;
    let role: Role = client
        .get(&format!("/api/organizations/{org_id}/roles/{id}/"))
        .await?;
    print_role(&role, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_role(
    cx: &CommandContext,
    name: String,
    feature_flags_access_level: Option<i32>,
) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;

    let mut body = serde_json::Map::new();
    body.insert("name".into(), Value::String(name));
    if let Some(level) = feature_flags_access_level {
        body.insert(
            "feature_flags_access_level".into(),
            Value::Number(level.into()),
        );
    }

    let created: Role = client
        .post(
            &format!("/api/organizations/{org_id}/roles/"),
            &Value::Object(body),
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
        println!("Created role '{}' (id {})", created.name, created.id);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_role(cx: &CommandContext, id: String, name: Option<String>) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name)".into(),
        ));
    }

    cx.confirm(&format!("update role `{id}`; continue?"))?;

    let updated: Role = client
        .patch(
            &format!("/api/organizations/{org_id}/roles/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated role '{}' (id {})", updated.name, updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_role(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;

    cx.confirm(&format!("hard-delete role `{id}`; continue?"))?;

    client
        .delete(&format!("/api/organizations/{org_id}/roles/{id}/"))
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
        println!("Deleted role {id}");
    }
    Ok(())
}

// ── members ───────────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
struct RoleMembership {
    pub id: String,
    #[serde(default)]
    pub user: Option<Value>,
    #[serde(default)]
    pub joined_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Serialize)]
struct MembersListOutput {
    count: usize,
    results: Vec<RoleMembership>,
}

async fn list_members(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;
    let path = format!("/api/organizations/{org_id}/roles/{id}/role_memberships/");
    let results: Vec<RoleMembership> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&MembersListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["MEMBERSHIP_ID", "USER_EMAIL", "JOINED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|m| {
                let email = m
                    .user
                    .as_ref()
                    .and_then(|u| u.get("email"))
                    .and_then(Value::as_str)
                    .unwrap_or("-")
                    .to_string();
                vec![
                    m.id.clone(),
                    email,
                    m.joined_at.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── add-member ────────────────────────────────────────────────────────────────

async fn add_member(cx: &CommandContext, id: String, user_id: String) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;

    cx.confirm(&format!("add user `{user_id}` to role `{id}`; continue?"))?;

    let body = json!({ "user_uuid": user_id });
    let membership: RoleMembership = client
        .post(
            &format!("/api/organizations/{org_id}/roles/{id}/role_memberships/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&membership);
    } else {
        println!(
            "Added user {user_id} to role {id} (membership {})",
            membership.id
        );
    }
    Ok(())
}

// ── remove-member ─────────────────────────────────────────────────────────────

async fn remove_member(cx: &CommandContext, id: String, membership_id: String) -> Result<()> {
    let client = &cx.client;
    let org_id = org_id_required(client)?;

    cx.confirm(&format!(
        "remove membership `{membership_id}` from role `{id}`; continue?"
    ))?;

    client
        .delete(&format!(
            "/api/organizations/{org_id}/roles/{id}/role_memberships/{membership_id}/"
        ))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            membership_id: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "remove-member",
            membership_id,
        });
    } else {
        println!("Removed membership {membership_id} from role {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_role(role: &Role, json_mode: bool) {
    if json_mode {
        output::print_json(role);
    } else {
        println!("ID:                       {}", role.id);
        println!("Name:                     {}", role.name);
        if let Some(level) = role.feature_flags_access_level {
            println!("Feature Flags Access:     {level}");
        }
        if let Some(ca) = role.created_at.as_deref() {
            println!("Created:                  {ca}");
        }
        if let Some(members) = role.members.as_ref() {
            println!("Members:                  {}", members.len());
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_roundtrip_minimal() {
        let raw = r#"{"id": "role-1", "name": "Engineers"}"#;
        let r: Role = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "role-1");
        assert_eq!(r.name, "Engineers");
        assert!(r.feature_flags_access_level.is_none());
    }

    #[test]
    fn role_roundtrip_full() {
        let raw = r#"{
            "id": "role-full",
            "name": "Admins",
            "feature_flags_access_level": 37,
            "created_at": "2026-01-01T00:00:00Z",
            "created_by": {"email": "admin@example.com"},
            "members": [{"id": "m1"}, {"id": "m2"}],
            "associated_flags": []
        }"#;
        let r: Role = serde_json::from_str(raw).unwrap();
        assert_eq!(r.feature_flags_access_level, Some(37));
        assert_eq!(r.members.as_ref().map(|m| m.len()), Some(2));
    }

    #[test]
    fn role_membership_roundtrip() {
        let raw = r#"{
            "id": "mem-1",
            "user": {"id": "user-1", "email": "dev@example.com"},
            "joined_at": "2026-04-01T00:00:00Z"
        }"#;
        let m: RoleMembership = serde_json::from_str(raw).unwrap();
        assert_eq!(m.id, "mem-1");
        let email = m
            .user
            .as_ref()
            .and_then(|u| u.get("email"))
            .and_then(Value::as_str)
            .unwrap();
        assert_eq!(email, "dev@example.com");
    }
}
