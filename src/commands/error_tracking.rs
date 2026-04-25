// src/commands/error_tracking.rs
//! `bosshogg error-tracking` — fingerprints / assignment-rules / grouping-rules /
//! issues / resolve-github / resolve-gitlab / releases / symbol-sets.
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

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ErrorIssue {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
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
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ErrorTrackingRelease {
    pub id: String,
    pub hash_id: String,
    pub team_id: i64,
    pub created_at: String,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ErrorTrackingSymbolSet {
    pub id: String,
    #[serde(rename = "ref")]
    pub ref_: String,
    pub team_id: i64,
    pub created_at: String,
    #[serde(default)]
    pub last_used: Option<String>,
    #[serde(default)]
    pub storage_ptr: Option<String>,
    #[serde(default)]
    pub failure_reason: Option<String>,
    #[serde(default)]
    pub release: Option<Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct SymbolSetDownloadResponse {
    pub url: String,
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
    /// Manage error tracking issues (list, get, assign, merge, split, etc.).
    #[command(subcommand)]
    Issues(IssuesCommand),
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
    /// Look up releases linked to source-map uploads.
    #[command(name = "releases", subcommand)]
    Releases(ReleasesCommand),
    /// Manage source-map symbol sets (upload bracket + download).
    #[command(name = "symbol-sets", subcommand)]
    SymbolSets(SymbolSetsCommand),
}

#[derive(Subcommand, Debug)]
pub enum IssuesCommand {
    /// List error tracking issues (paginated).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single error tracking issue by ID.
    Get { id: String },
    /// Get the activity log for a specific issue.
    Activity { id: String },
    /// Get the project-level activity log across all issues.
    #[command(name = "activity-list")]
    ActivityList,
    /// Assign an issue to a user.
    Assign {
        id: String,
        /// Numeric user ID to assign to.
        #[arg(long)]
        assignee_id: i64,
    },
    /// Create a cohort of users who encountered this issue.
    Cohort { id: String },
    /// Merge this issue into another issue (destructive).
    Merge {
        id: String,
        /// ID of the issue to merge into.
        #[arg(long)]
        into: String,
    },
    /// Split this issue by fingerprints into separate issues (destructive).
    Split {
        id: String,
        /// Path to a JSON file containing an array of fingerprint strings.
        #[arg(long)]
        fingerprints_file: PathBuf,
    },
    /// Bulk action on multiple issues.
    Bulk {
        /// Path to a JSON file containing an array of issue IDs.
        #[arg(long)]
        ids_file: PathBuf,
        /// Action to perform (e.g. "resolve", "archive").
        #[arg(long)]
        action: String,
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

#[derive(Subcommand, Debug)]
pub enum ReleasesCommand {
    /// List releases (paginated).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single release by ID (UUID).
    Get { id: String },
    /// Look up a release by its source-map hash.
    #[command(name = "by-hash")]
    ByHash {
        /// The source-map hash identifying the release.
        hash: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum SymbolSetsCommand {
    /// List symbol sets (paginated).
    List {
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single symbol set by ID (UUID).
    Get { id: String },
    /// Get a presigned download URL for a symbol set's source map.
    ///
    /// Returns a JSON object with a `url` field (presigned URL). Pipe through
    /// `jq -r .url` and then `curl -o out.map <url>` to retrieve the file.
    Download {
        /// Symbol set UUID.
        id: String,
        /// Write the JSON response to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Start a source-map upload. Returns a presigned URL.
    ///
    /// UPLOAD FLOW:
    ///   1. bosshogg error-tracking symbol-sets start-upload --name <name> --kind <chunk|sourcemap>
    ///      (prints presigned URL)
    ///   2. curl -T <your-file> '<presigned_url>'
    ///      (PUT the source-map file directly to the presigned URL — S3/GCS, not PostHog)
    ///   3. bosshogg error-tracking symbol-sets finish-upload <id>
    ///      (marks upload complete in PostHog)
    ///
    /// Use global --yes to skip the confirmation prompt.
    #[command(name = "start-upload")]
    StartUpload {
        /// Human-readable name / ref for this symbol set.
        #[arg(long)]
        name: String,
        /// Symbol set kind: `chunk` or `sourcemap`.
        #[arg(long)]
        kind: Option<String>,
    },
    /// Finish a source-map upload (marks upload complete in PostHog).
    ///
    /// Call this after uploading the file to the presigned URL returned by
    /// `start-upload`. See `start-upload --help` for the full upload flow.
    ///
    /// Use global --yes to skip the confirmation prompt.
    #[command(name = "finish-upload")]
    FinishUpload {
        /// Symbol set UUID returned by `start-upload`.
        id: String,
    },
    /// Bulk-delete symbol sets by ID list. Use global --yes to skip confirmation.
    #[command(name = "bulk-delete")]
    BulkDelete {
        /// Path to a JSON file containing an array of symbol-set UUIDs.
        #[arg(long)]
        ids_file: PathBuf,
    },
    /// Bulk start-upload for multiple symbol sets.
    ///
    /// See `start-upload --help` for the full upload flow. Provide a JSON file
    /// whose shape matches the PostHog `bulk_start_upload` body (array of name objects).
    ///
    /// Use global --yes to skip the confirmation prompt.
    #[command(name = "bulk-start-upload")]
    BulkStartUpload {
        /// Path to a JSON file containing the bulk start-upload body.
        #[arg(long)]
        names_file: PathBuf,
    },
    /// Bulk finish-upload for multiple symbol sets.
    ///
    /// Call after uploading each file to its presigned URL. Provide a JSON file
    /// containing an array of symbol-set UUIDs to mark complete.
    ///
    /// Use global --yes to skip the confirmation prompt.
    #[command(name = "bulk-finish-upload")]
    BulkFinishUpload {
        /// Path to a JSON file containing an array of symbol-set UUIDs.
        #[arg(long)]
        ids_file: PathBuf,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: ErrorTrackingArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        ErrorTrackingCommand::Fingerprints(cmd) => dispatch_fingerprints(cx, cmd).await,
        ErrorTrackingCommand::AssignmentRules(cmd) => dispatch_assignment_rules(cx, cmd).await,
        ErrorTrackingCommand::GroupingRules(cmd) => dispatch_grouping_rules(cx, cmd).await,
        ErrorTrackingCommand::Issues(cmd) => dispatch_issues(cx, cmd).await,
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
        ErrorTrackingCommand::Releases(cmd) => dispatch_releases(cx, cmd).await,
        ErrorTrackingCommand::SymbolSets(cmd) => dispatch_symbol_sets(cx, cmd).await,
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

// ── issues ────────────────────────────────────────────────────────────────────

async fn dispatch_issues(cx: &CommandContext, cmd: IssuesCommand) -> Result<()> {
    match cmd {
        IssuesCommand::List { limit } => list_issues(cx, limit).await,
        IssuesCommand::Get { id } => get_issue(cx, id).await,
        IssuesCommand::Activity { id } => issue_activity(cx, id).await,
        IssuesCommand::ActivityList => issues_activity_list(cx).await,
        IssuesCommand::Assign { id, assignee_id } => assign_issue(cx, id, assignee_id).await,
        IssuesCommand::Cohort { id } => issue_cohort(cx, id).await,
        IssuesCommand::Merge { id, into } => merge_issue(cx, id, into).await,
        IssuesCommand::Split {
            id,
            fingerprints_file,
        } => split_issue(cx, id, fingerprints_file).await,
        IssuesCommand::Bulk { ids_file, action } => bulk_issues(cx, ids_file, action).await,
    }
}

#[derive(Serialize)]
struct IssuesListOutput {
    count: usize,
    results: Vec<ErrorIssue>,
}

async fn list_issues(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/error_tracking/issues/");
    let results: Vec<ErrorIssue> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&IssuesListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "STATUS", "OCCURRENCES", "LAST_SEEN"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|i| {
                vec![
                    i.id.clone(),
                    i.name.clone().unwrap_or_else(|| "-".into()),
                    i.status.clone().unwrap_or_else(|| "-".into()),
                    i.occurrences
                        .map(|n| n.to_string())
                        .unwrap_or_else(|| "-".into()),
                    i.last_seen.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn get_issue(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let issue: ErrorIssue = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/issues/{id}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&issue);
    } else {
        println!("ID:           {}", issue.id);
        if let Some(n) = issue.name.as_deref() {
            println!("Name:         {n}");
        }
        if let Some(s) = issue.status.as_deref() {
            println!("Status:       {s}");
        }
        if let Some(n) = issue.occurrences {
            println!("Occurrences:  {n}");
        }
        if let Some(u) = issue.affected_users {
            println!("Users:        {u}");
        }
        if let Some(ls) = issue.last_seen.as_deref() {
            println!("Last seen:    {ls}");
        }
        if let Some(fs) = issue.first_seen.as_deref() {
            println!("First seen:   {fs}");
        }
        if let Some(d) = issue.description.as_deref() {
            println!("Description:  {d}");
        }
    }
    Ok(())
}

async fn issue_activity(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/issues/{id}/activity/"
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
    } else {
        output::print_json(&v);
    }
    Ok(())
}

async fn issues_activity_list(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/issues/activity/"
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
    } else {
        output::print_json(&v);
    }
    Ok(())
}

async fn assign_issue(cx: &CommandContext, id: String, assignee_id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "assign issue `{id}` to user {assignee_id}; continue?"
    ))?;

    let body = json!({ "assignee_id": assignee_id });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/issues/{id}/assign/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Assigned issue {id} to user {assignee_id}");
    }
    Ok(())
}

async fn issue_cohort(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "create cohort from users who hit issue `{id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/issues/{id}/cohort/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Created cohort for issue {id}");
    }
    Ok(())
}

async fn merge_issue(cx: &CommandContext, id: String, into_issue_id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "merge issue `{id}` into `{into_issue_id}` (destructive); continue?"
    ))?;

    let body = json!({ "into_issue_id": into_issue_id });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/issues/{id}/merge/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Merged issue {id} into {into_issue_id}");
    }
    Ok(())
}

async fn split_issue(cx: &CommandContext, id: String, fingerprints_file: PathBuf) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let fingerprints = read_json_file(&fingerprints_file).await?;

    cx.confirm(&format!(
        "split issue `{id}` by fingerprints (destructive); continue?"
    ))?;

    let body = json!({ "fingerprints": fingerprints });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/issues/{id}/split/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Split issue {id} by fingerprints");
    }
    Ok(())
}

async fn bulk_issues(cx: &CommandContext, ids_file: PathBuf, action: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let ids = read_json_file(&ids_file).await?;

    cx.confirm(&format!(
        "bulk action `{action}` on issues from file; continue?"
    ))?;

    let body = json!({ "ids": ids, "action": action });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/issues/bulk/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Bulk action `{action}` applied to issues");
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

// ── releases ──────────────────────────────────────────────────────────────────

async fn dispatch_releases(cx: &CommandContext, cmd: ReleasesCommand) -> Result<()> {
    match cmd {
        ReleasesCommand::List { limit } => list_releases(cx, limit).await,
        ReleasesCommand::Get { id } => get_release(cx, id).await,
        ReleasesCommand::ByHash { hash } => get_release_by_hash(cx, hash).await,
    }
}

#[derive(Serialize)]
struct ReleasesListOutput {
    count: usize,
    results: Vec<ErrorTrackingRelease>,
}

async fn list_releases(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/error_tracking/releases/");
    let results: Vec<ErrorTrackingRelease> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ReleasesListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "HASH_ID", "VERSION", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.id.clone(),
                    r.hash_id.clone(),
                    r.version.clone().unwrap_or_else(|| "-".into()),
                    r.created_at.clone(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn get_release(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let r: ErrorTrackingRelease = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/releases/{id}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&r);
    } else {
        println!("ID:          {}", r.id);
        println!("Hash ID:     {}", r.hash_id);
        if let Some(v) = r.version.as_deref() {
            println!("Version:     {v}");
        }
        println!("Created at:  {}", r.created_at);
    }
    Ok(())
}

async fn get_release_by_hash(cx: &CommandContext, hash: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    // The response shape for the hash lookup is unspecified in the schema (no body),
    // so we decode as raw Value to be forward-compatible.
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/releases/hash/{hash}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
    }
    Ok(())
}

// ── symbol-sets ───────────────────────────────────────────────────────────────

async fn dispatch_symbol_sets(cx: &CommandContext, cmd: SymbolSetsCommand) -> Result<()> {
    match cmd {
        SymbolSetsCommand::List { limit } => list_symbol_sets(cx, limit).await,
        SymbolSetsCommand::Get { id } => get_symbol_set(cx, id).await,
        SymbolSetsCommand::Download { id, out } => download_symbol_set(cx, id, out).await,
        SymbolSetsCommand::StartUpload { name, kind } => start_upload(cx, name, kind).await,
        SymbolSetsCommand::FinishUpload { id } => finish_upload(cx, id).await,
        SymbolSetsCommand::BulkDelete { ids_file } => bulk_delete_symbol_sets(cx, ids_file).await,
        SymbolSetsCommand::BulkStartUpload { names_file } => {
            bulk_start_upload(cx, names_file).await
        }
        SymbolSetsCommand::BulkFinishUpload { ids_file } => bulk_finish_upload(cx, ids_file).await,
    }
}

#[derive(Serialize)]
struct SymbolSetsListOutput {
    count: usize,
    results: Vec<ErrorTrackingSymbolSet>,
}

async fn list_symbol_sets(cx: &CommandContext, limit: Option<usize>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/error_tracking/symbol_sets/");
    let results: Vec<ErrorTrackingSymbolSet> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&SymbolSetsListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "REF", "CREATED_AT", "LAST_USED", "FAILURE"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|s| {
                vec![
                    s.id.clone(),
                    s.ref_.clone(),
                    s.created_at.clone(),
                    s.last_used.clone().unwrap_or_else(|| "-".into()),
                    s.failure_reason.clone().unwrap_or_else(|| "-".into()),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn get_symbol_set(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let s: ErrorTrackingSymbolSet = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/symbol_sets/{id}/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&s);
    } else {
        println!("ID:          {}", s.id);
        println!("Ref:         {}", s.ref_);
        println!("Created at:  {}", s.created_at);
        if let Some(lu) = s.last_used.as_deref() {
            println!("Last used:   {lu}");
        }
        if let Some(fr) = s.failure_reason.as_deref() {
            println!("Failure:     {fr}");
        }
    }
    Ok(())
}

async fn download_symbol_set(cx: &CommandContext, id: String, out: Option<PathBuf>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    // The download endpoint returns a JSON object with a `url` field containing
    // a presigned URL. Print the JSON; users can pipe through `jq -r .url | xargs curl -o file`.
    let resp: SymbolSetDownloadResponse = client
        .get(&format!(
            "/api/environments/{env_id}/error_tracking/symbol_sets/{id}/download/"
        ))
        .await?;

    if let Some(path) = out {
        let json_bytes = serde_json::to_vec_pretty(&resp)
            .map_err(|e| BosshoggError::Config(format!("serialize error: {e}")))?;
        tokio::fs::write(&path, &json_bytes)
            .await
            .map_err(|e| BosshoggError::Config(format!("write {}: {e}", path.display())))?;
        if cx.json_mode {
            output::print_json(
                &serde_json::json!({"ok": true, "path": path.display().to_string()}),
            );
        } else {
            println!("Written to {}", path.display());
        }
    } else if cx.json_mode {
        output::print_json(&resp);
    } else {
        println!("Presigned URL: {}", resp.url);
        println!();
        println!("To download the source map:");
        println!("  curl -o <output-file> '{}'", resp.url);
    }
    Ok(())
}

async fn start_upload(cx: &CommandContext, name: String, kind: Option<String>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("start upload for symbol set `{name}`; continue?"))?;

    let mut body = serde_json::Map::new();
    body.insert("ref".into(), serde_json::Value::String(name));
    if let Some(k) = kind {
        body.insert("kind".into(), serde_json::Value::String(k));
    }

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/symbol_sets/start_upload/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        println!();
        println!("NEXT STEP: Upload the source-map file to the presigned URL above:");
        println!("  curl -T <your-file> '<presigned_url>'");
        println!("Then run: bosshogg error-tracking symbol-sets finish-upload <id>");
    }
    Ok(())
}

async fn finish_upload(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "mark upload complete for symbol set `{id}`; continue?"
    ))?;

    // finish_upload is a PUT — body is the symbol set object; send minimal body.
    let v: Value = client
        .put(
            &format!("/api/environments/{env_id}/error_tracking/symbol_sets/{id}/finish_upload/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Upload marked complete for symbol set {id}");
    }
    Ok(())
}

async fn bulk_delete_symbol_sets(cx: &CommandContext, ids_file: PathBuf) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let ids = read_json_file(&ids_file).await?;

    cx.confirm("bulk-delete symbol sets from file; continue?")?;

    let body = json!({ "ids": ids });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/symbol_sets/bulk_delete/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Bulk delete submitted");
    }
    Ok(())
}

async fn bulk_start_upload(cx: &CommandContext, names_file: PathBuf) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let body = read_json_file(&names_file).await?;

    cx.confirm("bulk start-upload for symbol sets from file; continue?")?;

    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/symbol_sets/bulk_start_upload/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        println!();
        println!("NEXT STEP: Upload each file to its presigned URL, then run bulk-finish-upload.");
    }
    Ok(())
}

async fn bulk_finish_upload(cx: &CommandContext, ids_file: PathBuf) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let ids = read_json_file(&ids_file).await?;

    cx.confirm("bulk finish-upload for symbol sets from file; continue?")?;

    let body = json!({ "ids": ids });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/error_tracking/symbol_sets/bulk_finish_upload/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Bulk finish-upload submitted");
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

    #[test]
    fn error_issue_roundtrip_minimal() {
        let raw = r#"{"id": "issue-1"}"#;
        let i: ErrorIssue = serde_json::from_str(raw).unwrap();
        assert_eq!(i.id, "issue-1");
        assert!(i.name.is_none());
        assert!(i.status.is_none());
    }

    #[test]
    fn error_issue_roundtrip_full() {
        let raw = r#"{
            "id": "issue-full",
            "name": "NullPointerException in main",
            "status": "active",
            "last_seen": "2026-04-20T10:00:00Z",
            "first_seen": "2026-03-01T08:00:00Z",
            "occurrences": 100,
            "affected_users": 25,
            "assignee": {"id": "user-1", "email": "dev@example.com"},
            "description": "Crash in request handler"
        }"#;
        let i: ErrorIssue = serde_json::from_str(raw).unwrap();
        assert_eq!(i.id, "issue-full");
        assert_eq!(i.occurrences, Some(100));
        assert_eq!(i.affected_users, Some(25));
        assert_eq!(i.status.as_deref(), Some("active"));
        assert_eq!(i.name.as_deref(), Some("NullPointerException in main"));
    }
}
