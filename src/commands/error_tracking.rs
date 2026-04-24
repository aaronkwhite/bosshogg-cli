// src/commands/error_tracking.rs
//! `bosshogg error-tracking` — fingerprints / assignment-rules / grouping-rules /
//! resolve-github / resolve-gitlab.
//!
//! All endpoints are environment-scoped (PostHog moved these from the legacy
//! `/api/projects/:project_id/error_tracking/...` routes to
//! `/api/environments/:team_id/error_tracking/...`).
//!
//! Soft-delete notes:
//!   - `error_tracking/fingerprints` IS in SOFT_DELETE_RESOURCES — client.delete()
//!     rewrites to PATCH {"deleted": true}.
//!   - `assignment_rules` and `grouping_rules` are NOT — hard DELETE.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::commands::context::CommandContext;
use crate::commands::util::{env_id_required, read_json_file};
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed structs ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ErrorFingerprint {
    pub id: String,
    pub fingerprint: String,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub last_seen: Option<String>,
    #[serde(default)]
    pub first_seen: Option<String>,
    #[serde(default)]
    pub occurrences: Option<i64>,
    #[serde(default)]
    pub affected_users: Option<i64>,
    #[serde(default)]
    pub assignee: Option<Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AssignmentRule {
    pub id: String,
    pub filters: Value,
    pub assignee: Value,
    #[serde(default)]
    pub order_key: Option<i32>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct GroupingRule {
    pub id: String,
    pub filters: Value,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub assignee: Option<Value>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ErrorTrackingArgs {
    #[command(subcommand)]
    pub command: ErrorTrackingCommand,
}

#[derive(Subcommand, Debug)]
pub enum ErrorTrackingCommand {
    /// Manage error fingerprints.
    #[command(subcommand)]
    Fingerprints(FingerprintsCommand),
    /// Manage assignment rules.
    #[command(name = "assignment-rules", subcommand)]
    AssignmentRules(AssignmentRulesCommand),
    /// Manage grouping rules.
    #[command(name = "grouping-rules", subcommand)]
    GroupingRules(GroupingRulesCommand),
    /// Resolve a GitHub source location to an error fingerprint.
    #[command(name = "resolve-github")]
    ResolveGithub {
        #[arg(long)]
        organization: String,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        line: u32,
    },
    /// Resolve a GitLab source location to an error fingerprint.
    #[command(name = "resolve-gitlab")]
    ResolveGitlab {
        #[arg(long)]
        organization: String,
        #[arg(long)]
        repo: String,
        #[arg(long)]
        file: String,
        #[arg(long)]
        line: u32,
    },
}

#[derive(Subcommand, Debug)]
pub enum FingerprintsCommand {
    /// List error fingerprints.
    List {
        /// Filter by distinct ID.
        #[arg(long)]
        distinct_id: Option<String>,
        /// Search by string.
        #[arg(long)]
        search: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single error fingerprint by ID.
    Get { id: String },
}

#[derive(Subcommand, Debug)]
pub enum AssignmentRulesCommand {
    /// List all assignment rules.
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Create a new assignment rule.
    Create {
        /// Path to a JSON file containing the filters object.
        #[arg(long)]
        filters_file: PathBuf,
        /// Assignee ID (user UUID).
        #[arg(long)]
        assignee_id: String,
    },
    /// Get a single assignment rule by ID.
    Get { id: String },
    /// Update an assignment rule.
    Update {
        id: String,
        /// Path to a JSON file with updated filters.
        #[arg(long)]
        filters_file: Option<PathBuf>,
    },
    /// Hard-delete an assignment rule.
    Delete { id: String },
    /// Reorder assignment rules.
    Reorder {
        /// Path to a JSON file with ordered rule IDs: `[id1, id2, ...]`.
        #[arg(long)]
        order_file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
pub enum GroupingRulesCommand {
    /// List all grouping rules.
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Create a new grouping rule.
    Create {
        /// Path to a JSON file containing the filters object.
        #[arg(long)]
        filters_file: PathBuf,
        /// Human-readable description.
        #[arg(long)]
        description: String,
    },
    /// Get a single grouping rule by ID.
    Get { id: String },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: ErrorTrackingArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        ErrorTrackingCommand::Fingerprints(cmd) => dispatch_fingerprints(cx, cmd).await,
        ErrorTrackingCommand::AssignmentRules(cmd) => dispatch_assignment_rules(cx, cmd).await,
        ErrorTrackingCommand::GroupingRules(cmd) => dispatch_grouping_rules(cx, cmd).await,
        ErrorTrackingCommand::ResolveGithub {
            organization,
            repo,
            file,
            line,
        } => resolve_source(cx, "resolve_github", organization, repo, file, line).await,
        ErrorTrackingCommand::ResolveGitlab {
            organization,
            repo,
            file,
            line,
        } => resolve_source(cx, "resolve_gitlab", organization, repo, file, line).await,
    }
}

// ── fingerprints ──────────────────────────────────────────────────────────────

async fn dispatch_fingerprints(cx: &CommandContext, cmd: FingerprintsCommand) -> Result<()> {
    match cmd {
        FingerprintsCommand::List {
            distinct_id,
            search,
            limit,
        } => list_fingerprints(cx, distinct_id, search, limit).await,
        FingerprintsCommand::Get { id } => get_fingerprint(cx, id).await,
    }
}

#[derive(Serialize)]
struct FingerprintsListOutput {
    count: usize,
    results: Vec<ErrorFingerprint>,
}

async fn list_fingerprints(
    cx: &CommandContext,
    distinct_id: Option<String>,
    search: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut params: Vec<String> = Vec::new();
    if let Some(d) = distinct_id {
        params.push(format!("distinct_id={}", urlencoding::encode(&d)));
    }
    if let Some(s) = search {
        params.push(format!("search={}", urlencoding::encode(&s)));
    }
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };

    let path = format!("/api/environments/{env_id}/error_tracking/fingerprints/{query}");
    let results: Vec<ErrorFingerprint> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&FingerprintsListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "FINGERPRINT", "STATUS", "OCCURRENCES", "LAST_SEEN"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|f| {
                vec![
                    f.id.clone(),
                    f.fingerprint.clone(),
                    f.status.clone().unwrap_or_else(|| "-".into()),
                    f.occurrences
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "-".into()),
                    f.last_seen.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn get_fingerprint(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let f: ErrorFingerprint = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/fingerprints/{id}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&f);
    } else {
        println!("ID:           {}", f.id);
        println!("Fingerprint:  {}", f.fingerprint);
        if let Some(s) = f.status.as_deref() {
            println!("Status:       {s}");
        }
        if let Some(n) = f.occurrences {
            println!("Occurrences:  {n}");
        }
        if let Some(u) = f.affected_users {
            println!("Users:        {u}");
        }
        if let Some(ls) = f.last_seen.as_deref() {
            println!("Last seen:    {ls}");
        }
        if let Some(fs) = f.first_seen.as_deref() {
            println!("First seen:   {fs}");
        }
    }
    Ok(())
}

// ── assignment-rules ──────────────────────────────────────────────────────────

async fn dispatch_assignment_rules(cx: &CommandContext, cmd: AssignmentRulesCommand) -> Result<()> {
    match cmd {
        AssignmentRulesCommand::List { limit } => list_assignment_rules(cx, limit).await,
        AssignmentRulesCommand::Create {
            filters_file,
            assignee_id,
        } => create_assignment_rule(cx, filters_file, assignee_id).await,
        AssignmentRulesCommand::Get { id } => get_assignment_rule(cx, id).await,
        AssignmentRulesCommand::Update { id, filters_file } => {
            update_assignment_rule(cx, id, filters_file).await
        }
        AssignmentRulesCommand::Delete { id } => delete_assignment_rule(cx, id).await,
        AssignmentRulesCommand::Reorder { order_file } => {
            reorder_assignment_rules(cx, order_file).await
        }
    }
}

#[derive(Serialize)]
struct AssignmentRulesListOutput {
    count: usize,
    results: Vec<AssignmentRule>,
}

async fn list_assignment_rules(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/error_tracking/assignment_rules/");
    let results: Vec<AssignmentRule> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&AssignmentRulesListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "ORDER_KEY", "ASSIGNEE"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.order_key
                        .map(|k| k.to_string())
                        .unwrap_or_else(|| "-".into()),
                    r.assignee
                        .get("email")
                        .or_else(|| r.assignee.get("id"))
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .to_string(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn create_assignment_rule(
    cx: &CommandContext,
    filters_file: PathBuf,
    assignee_id: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let filters = read_json_file(&filters_file).await?;

    cx.confirm("create assignment rule; continue?")?;

    let body = json!({
        "filters": filters,
        "assignee": { "id": assignee_id },
    });

    let created: AssignmentRule = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/assignment_rules/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&created);
    } else {
        println!("Created assignment rule {}", created.id);
    }
    Ok(())
}

async fn get_assignment_rule(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let r: AssignmentRule = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/assignment_rules/{id}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&r);
    } else {
        println!("ID:        {}", r.id);
        println!(
            "Order:     {}",
            r.order_key
                .map(|k| k.to_string())
                .unwrap_or_else(|| "-".into())
        );
    }
    Ok(())
}

async fn update_assignment_rule(
    cx: &CommandContext,
    id: String,
    filters_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(f) = filters_file.as_deref() {
        body.insert("filters".into(), read_json_file(f).await?);
    }
    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --filters-file)".into(),
        ));
    }

    cx.confirm(&format!("update assignment rule `{id}`; continue?"))?;

    let updated: AssignmentRule = client
        .patch(
            &format!("/api/environments/{env_id}/error_tracking/assignment_rules/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated assignment rule {}", updated.id);
    }
    Ok(())
}

async fn delete_assignment_rule(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("hard-delete assignment rule `{id}`; continue?"))?;

    // assignment_rules NOT in SOFT_DELETE_RESOURCES — hard delete.
    client
        .delete(&format!(
            "/api/environments/{env_id}/error_tracking/assignment_rules/{id}/"
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
            id,
        });
    } else {
        println!("Deleted assignment rule {id}");
    }
    Ok(())
}

async fn reorder_assignment_rules(cx: &CommandContext, order_file: PathBuf) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let rule_ids = read_json_file(&order_file).await?;

    cx.confirm("reorder assignment rules; continue?")?;

    let body = json!({ "rule_ids": rule_ids });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/assignment_rules/reorder/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Assignment rules reordered");
    }
    Ok(())
}

// ── grouping-rules ────────────────────────────────────────────────────────────

async fn dispatch_grouping_rules(cx: &CommandContext, cmd: GroupingRulesCommand) -> Result<()> {
    match cmd {
        GroupingRulesCommand::List { limit } => list_grouping_rules(cx, limit).await,
        GroupingRulesCommand::Create {
            filters_file,
            description,
        } => create_grouping_rule(cx, filters_file, description).await,
        GroupingRulesCommand::Get { id } => get_grouping_rule(cx, id).await,
    }
}

#[derive(Serialize)]
struct GroupingRulesListOutput {
    count: usize,
    results: Vec<GroupingRule>,
}

async fn list_grouping_rules(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/error_tracking/grouping_rules/");
    let results: Vec<GroupingRule> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&GroupingRulesListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "DESCRIPTION"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.description.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn create_grouping_rule(
    cx: &CommandContext,
    filters_file: PathBuf,
    description: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let filters = read_json_file(&filters_file).await?;

    cx.confirm("create grouping rule; continue?")?;

    let body = json!({
        "filters": filters,
        "description": description,
    });

    let created: GroupingRule = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/grouping_rules/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&created);
    } else {
        println!("Created grouping rule {}", created.id);
    }
    Ok(())
}

async fn get_grouping_rule(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let r: GroupingRule = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/grouping_rules/{id}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&r);
    } else {
        println!("ID:           {}", r.id);
        if let Some(d) = r.description.as_deref() {
            println!("Description:  {d}");
        }
    }
    Ok(())
}

// ── resolve source ────────────────────────────────────────────────────────────

async fn resolve_source(
    cx: &CommandContext,
    endpoint: &str,
    organization: String,
    repo: String,
    file: String,
    line: u32,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!(
        "/api/environments/{env_id}/error_tracking/{endpoint}/?organization={}&repo={}&file={}&line={}",
        urlencoding::encode(&organization),
        urlencoding::encode(&repo),
        urlencoding::encode(&file),
        line
    );
    let v: Value = client.get(&path).await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_fingerprint_roundtrip_minimal() {
        let raw = r#"{"id": "fp-1", "fingerprint": "abc123"}"#;
        let f: ErrorFingerprint = serde_json::from_str(raw).unwrap();
        assert_eq!(f.id, "fp-1");
        assert_eq!(f.fingerprint, "abc123");
        assert!(f.status.is_none());
    }

    #[test]
    fn error_fingerprint_roundtrip_full() {
        let raw = r#"{
            "id": "fp-full",
            "fingerprint": "def456",
            "status": "active",
            "last_seen": "2026-04-01T00:00:00Z",
            "first_seen": "2026-01-01T00:00:00Z",
            "occurrences": 42,
            "affected_users": 7,
            "assignee": {"id": "user-1", "email": "dev@example.com"}
        }"#;
        let f: ErrorFingerprint = serde_json::from_str(raw).unwrap();
        assert_eq!(f.occurrences, Some(42));
        assert_eq!(f.affected_users, Some(7));
        assert_eq!(f.status.as_deref(), Some("active"));
    }

    #[test]
    fn assignment_rule_roundtrip() {
        let raw = r#"{
            "id": "ar-1",
            "filters": {"events": []},
            "assignee": {"id": "user-1"},
            "order_key": 1
        }"#;
        let r: AssignmentRule = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "ar-1");
        assert_eq!(r.order_key, Some(1));
    }

    #[test]
    fn grouping_rule_roundtrip() {
        let raw = r#"{
            "id": "gr-1",
            "filters": {"type": "and", "values": []},
            "description": "Group by module",
            "assignee": null
        }"#;
        let r: GroupingRule = serde_json::from_str(raw).unwrap();
        assert_eq!(r.id, "gr-1");
        assert_eq!(r.description.as_deref(), Some("Group by module"));
    }
}
