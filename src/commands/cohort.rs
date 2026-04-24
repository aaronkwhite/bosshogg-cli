// src/commands/cohort.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::read_json_file;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Cohort {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub groups: Option<Value>, // fluid — legacy
    #[serde(default)]
    pub deleted: bool,
    #[serde(default)]
    pub filters: Value, // fluid
    #[serde(default)]
    pub query: Option<Value>,
    #[serde(default)]
    pub is_calculating: Option<bool>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub last_calculation: Option<String>,
    #[serde(default)]
    pub errors_calculating: Option<i32>,
    #[serde(default)]
    pub count: Option<i64>, // member count
    #[serde(default)]
    pub is_static: Option<bool>,
    #[serde(default)]
    pub experiment_set: Option<Vec<i64>>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct CohortArgs {
    #[command(subcommand)]
    pub command: CohortCommand,
}

#[derive(Subcommand, Debug)]
pub enum CohortCommand {
    /// List cohorts with optional search filter.
    List {
        #[arg(long)]
        search: Option<String>,
    },
    /// Get a single cohort by numeric id or exact name.
    Get {
        /// Cohort numeric id or exact name string.
        identifier: String,
    },
    /// Create a new cohort.
    Create {
        #[arg(long)]
        name: String,
        /// Path to a JSON file containing the filters object.
        #[arg(long)]
        filters_file: Option<PathBuf>,
        /// Create a static cohort (mutually exclusive with --dynamic).
        #[arg(long, conflicts_with = "dynamic")]
        r#static: bool,
        /// Create a dynamic cohort (default).
        #[arg(long, conflicts_with = "static")]
        dynamic: bool,
    },
    /// Update cohort fields.
    Update {
        id: i64,
        #[arg(long)]
        name: Option<String>,
        /// Path to a JSON file containing updated filters.
        #[arg(long)]
        filters_file: Option<PathBuf>,
    },
    /// Soft-delete a cohort (PATCH deleted=true).
    Delete { id: i64 },
    /// List members (persons) of a cohort (paginated).
    Members { id: i64 },
    /// Add a person to a static cohort by person UUID.
    AddPerson {
        id: i64,
        /// Person UUID (obtain from `bosshogg person` in M4).
        #[arg(long, conflicts_with = "distinct_id")]
        person_id: Option<String>,
        /// Distinct ID (not yet supported — use --person-id with a UUID instead).
        #[arg(long, conflicts_with = "person_id")]
        distinct_id: Option<String>,
    },
    /// Remove a person from a static cohort by person UUID.
    RemovePerson {
        id: i64,
        /// Person UUID (obtain from `bosshogg person` in M4).
        #[arg(long, conflicts_with = "distinct_id")]
        person_id: Option<String>,
        /// Distinct ID (not yet supported — use --person-id with a UUID instead).
        #[arg(long, conflicts_with = "person_id")]
        distinct_id: Option<String>,
    },
    /// View calculation history for a cohort.
    CalculationHistory { id: i64 },
    /// View the activity log for a cohort.
    Activity { id: i64 },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: CohortArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        CohortCommand::List { search } => list_cohorts(cx, search).await,
        CohortCommand::Get { identifier } => get_cohort_by_identifier(cx, &identifier).await,
        CohortCommand::Create {
            name,
            filters_file,
            r#static: is_static,
            dynamic: _,
        } => create_cohort(cx, name, filters_file, is_static).await,
        CohortCommand::Update {
            id,
            name,
            filters_file,
        } => update_cohort(cx, id, name, filters_file).await,
        CohortCommand::Delete { id } => delete_cohort(cx, id).await,
        CohortCommand::Members { id } => members_cohort(cx, id).await,
        CohortCommand::AddPerson {
            id,
            person_id,
            distinct_id,
        } => add_person(cx, id, person_id, distinct_id).await,
        CohortCommand::RemovePerson {
            id,
            person_id,
            distinct_id,
        } => remove_person(cx, id, person_id, distinct_id).await,
        CohortCommand::CalculationHistory { id } => calculation_history(cx, id).await,
        CohortCommand::Activity { id } => activity_cohort(cx, id).await,
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
    results: Vec<Cohort>,
}

async fn list_cohorts(cx: &CommandContext, search: Option<String>) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let query = if let Some(s) = search {
        format!("?search={}", urlencoding::encode(&s))
    } else {
        String::new()
    };

    let path = format!("/api/projects/{project_id}/cohorts/{query}");
    let results: Vec<Cohort> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "COUNT", "IS_STATIC", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|c| {
                vec![
                    c.id.to_string(),
                    c.name.clone(),
                    c.count.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
                    c.is_static
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "-".into()),
                    c.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

/// Resolve a cohort by numeric id or by exact name (list-then-filter fallback),
/// mirroring `bosshogg project get` name-resolution UX.
async fn resolve_cohort_by_identifier(client: &Client, identifier: &str) -> Result<Cohort> {
    let project_id = project_id_required(client)?;

    // Numeric id → direct GET
    if let Ok(id) = identifier.parse::<i64>() {
        let cohort: Cohort = client
            .get(&format!("/api/projects/{project_id}/cohorts/{id}/"))
            .await?;
        return Ok(cohort);
    }

    // Name → list with ?search=<name> and filter by exact name
    let query = format!("?search={}", urlencoding::encode(identifier));
    let path = format!("/api/projects/{project_id}/cohorts/{query}");
    let candidates: Vec<Cohort> = client.get_paginated(&path, None).await?;
    candidates
        .into_iter()
        .find(|c| c.name == identifier)
        .ok_or_else(|| BosshoggError::NotFound(format!("cohort '{identifier}'")))
}

async fn get_cohort_by_identifier(cx: &CommandContext, identifier: &str) -> Result<()> {
    let cohort = resolve_cohort_by_identifier(&cx.client, identifier).await?;
    print_cohort(&cohort, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_cohort(
    cx: &CommandContext,
    name: String,
    filters_file: Option<PathBuf>,
    is_static: bool,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = json!({ "name": name });

    if let Some(p) = filters_file.as_deref() {
        body["filters"] = read_json_file(p).await?;
    }

    if is_static {
        body["is_static"] = Value::Bool(true);
    }

    let created: Cohort = client
        .post(&format!("/api/projects/{project_id}/cohorts/"), &body)
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
        println!("Created cohort '{}' (id {})", created.name, created.id);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_cohort(
    cx: &CommandContext,
    id: i64,
    name: Option<String>,
    filters_file: Option<PathBuf>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(n) = name {
        body.insert("name".into(), Value::String(n));
    }
    if let Some(p) = filters_file.as_deref() {
        body.insert("filters".into(), read_json_file(p).await?);
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --filters-file)".into(),
        ));
    }

    cx.confirm(&format!("update cohort `{id}`; continue?"))?;

    let updated: Cohort = client
        .patch(
            &format!("/api/projects/{project_id}/cohorts/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated cohort '{}' (id {})", updated.name, updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_cohort(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("soft-delete cohort `{id}`; continue?"))?;

    client
        .delete(&format!("/api/projects/{project_id}/cohorts/{id}/"))
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
        println!("Deleted cohort {id}");
    }
    Ok(())
}

// ── members ───────────────────────────────────────────────────────────────────

async fn members_cohort(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let path = format!("/api/projects/{project_id}/cohorts/{id}/persons/");
    let results: Vec<Value> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            count: usize,
            results: Vec<Value>,
        }
        output::print_json(&Out {
            count: results.len(),
            results,
        });
    } else {
        println!("Members of cohort {id}: {}", results.len());
        for person in &results {
            let did = person
                .get("distinct_ids")
                .and_then(Value::as_array)
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .unwrap_or("-");
            let pid = person
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| person.get("uuid").and_then(Value::as_str).unwrap_or("-"));
            println!("  {pid}  {did}");
        }
    }
    Ok(())
}

// ── add-person ────────────────────────────────────────────────────────────────

async fn add_person(
    cx: &CommandContext,
    id: i64,
    person_id: Option<String>,
    distinct_id: Option<String>,
) -> Result<()> {
    if distinct_id.is_some() {
        return Err(BosshoggError::BadRequest(
            "--distinct-id is not yet supported for add-person; use --person-id <uuid> instead. \
             You can obtain person UUIDs with `bosshogg person list` (available in M4)."
                .into(),
        ));
    }

    let uuid =
        person_id.ok_or_else(|| BosshoggError::BadRequest("provide --person-id <uuid>".into()))?;

    let client = &cx.client;
    let project_id = project_id_required(client)?;

    // Validate that the cohort is static before mutating.
    let get_path = format!("/api/projects/{project_id}/cohorts/{id}/");
    let cohort: Cohort = client.get::<Cohort>(&get_path).await?;
    if cohort.is_static != Some(true) {
        return Err(BosshoggError::BadRequest(format!(
            "cohort {id} is dynamic; add-person / remove-person only work on static cohorts"
        )));
    }

    cx.confirm(&format!(
        "add person {uuid} to static cohort `{id}`; continue?"
    ))?;

    let body = json!({ "person_uuids": [uuid] });
    let v: Value = client
        .patch(
            &format!("/api/projects/{project_id}/cohorts/{id}/add_persons_to_static_cohort/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Added person {uuid} to cohort {id}");
    }
    Ok(())
}

// ── remove-person ─────────────────────────────────────────────────────────────

async fn remove_person(
    cx: &CommandContext,
    id: i64,
    person_id: Option<String>,
    distinct_id: Option<String>,
) -> Result<()> {
    if distinct_id.is_some() {
        return Err(BosshoggError::BadRequest(
            "--distinct-id is not yet supported for remove-person; use --person-id <uuid> instead. \
             You can obtain person UUIDs with `bosshogg person list` (available in M4)."
                .into(),
        ));
    }

    let uuid =
        person_id.ok_or_else(|| BosshoggError::BadRequest("provide --person-id <uuid>".into()))?;

    let client = &cx.client;
    let project_id = project_id_required(client)?;

    // Validate that the cohort is static before mutating.
    let get_path = format!("/api/projects/{project_id}/cohorts/{id}/");
    let cohort: Cohort = client.get::<Cohort>(&get_path).await?;
    if cohort.is_static != Some(true) {
        return Err(BosshoggError::BadRequest(format!(
            "cohort {id} is dynamic; add-person / remove-person only work on static cohorts"
        )));
    }

    cx.confirm(&format!(
        "remove person {uuid} from static cohort `{id}`; continue?"
    ))?;

    let body = json!({ "person_uuid": uuid });
    let v: Value = client
        .patch(
            &format!("/api/projects/{project_id}/cohorts/{id}/remove_person_from_static_cohort/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Removed person {uuid} from cohort {id}");
    }
    Ok(())
}

// ── calculation-history ───────────────────────────────────────────────────────

async fn calculation_history(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/projects/{project_id}/cohorts/{id}/calculation_history/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(results) = v.get("results").and_then(Value::as_array) {
        println!("Calculation history for cohort {id}:");
        for entry in results {
            let completed = entry
                .get("last_calculation")
                .and_then(Value::as_str)
                .unwrap_or("-");
            let count = entry
                .get("count")
                .and_then(Value::as_i64)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".into());
            let errors = entry
                .get("errors_calculating")
                .and_then(Value::as_i64)
                .map(|n| n.to_string())
                .unwrap_or_else(|| "0".into());
            println!("  {completed}  count={count}  errors={errors}");
        }
    } else {
        output::print_json(&v);
    }
    Ok(())
}

// ── activity ──────────────────────────────────────────────────────────────────

async fn activity_cohort(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/projects/{project_id}/cohorts/{id}/activity/"
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

// ── print helper ──────────────────────────────────────────────────────────────

fn print_cohort(cohort: &Cohort, json_mode: bool) {
    if json_mode {
        output::print_json(cohort);
    } else {
        println!("ID:              {}", cohort.id);
        println!("Name:            {}", cohort.name);
        if let Some(d) = cohort.description.as_deref() {
            println!("Description:     {d}");
        }
        println!("Static:          {}", cohort.is_static.unwrap_or(false));
        println!(
            "Count:           {}",
            cohort
                .count
                .map(|n| n.to_string())
                .unwrap_or_else(|| "-".into())
        );
        println!(
            "Is Calculating:  {}",
            cohort.is_calculating.unwrap_or(false)
        );
        if let Some(ca) = cohort.created_at.as_deref() {
            println!("Created:         {ca}");
        }
        if let Some(lc) = cohort.last_calculation.as_deref() {
            println!("Last Calc:       {lc}");
        }
        println!("Deleted:         {}", cohort.deleted);
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cohort_roundtrip_minimal() {
        let raw = r#"{
            "id": 1,
            "name": "My Cohort",
            "deleted": false,
            "filters": {}
        }"#;
        let c: Cohort = serde_json::from_str(raw).unwrap();
        assert_eq!(c.id, 1);
        assert_eq!(c.name, "My Cohort");
        assert!(!c.deleted);
    }

    #[test]
    fn cohort_roundtrip_full() {
        let raw = r#"{
            "id": 42,
            "name": "Power Users",
            "description": "Users who logged in 10+ times",
            "groups": [{"action_id": 1}],
            "deleted": false,
            "filters": {"properties": []},
            "query": null,
            "is_calculating": false,
            "created_by": {"id": 1, "email": "test@example.com"},
            "created_at": "2026-01-01T00:00:00Z",
            "last_calculation": "2026-04-01T00:00:00Z",
            "errors_calculating": 0,
            "count": 1234,
            "is_static": false,
            "experiment_set": [1, 2]
        }"#;
        let c: Cohort = serde_json::from_str(raw).unwrap();
        assert_eq!(c.id, 42);
        assert_eq!(c.count, Some(1234));
        assert_eq!(c.is_static, Some(false));
        assert_eq!(c.experiment_set, Some(vec![1, 2]));
        assert_eq!(c.errors_calculating, Some(0));
    }

    #[test]
    fn cohort_static_flag_parsed() {
        let raw = r#"{
            "id": 7,
            "name": "Static Cohort",
            "deleted": false,
            "filters": {},
            "is_static": true
        }"#;
        let c: Cohort = serde_json::from_str(raw).unwrap();
        assert_eq!(c.is_static, Some(true));
    }

    #[test]
    fn cohort_identifier_numeric() {
        // parse::<i64>() succeeds for numeric strings
        assert!("42".parse::<i64>().is_ok());
        assert!("power-users".parse::<i64>().is_err());
    }

    #[test]
    fn cohort_name_resolution_finds_exact_match() {
        // Simulates the exact-name filter in resolve_cohort_by_identifier.
        let candidates = vec![
            Cohort {
                id: 1,
                name: "power users".to_string(),
                description: None,
                groups: None,
                deleted: false,
                filters: serde_json::json!({}),
                query: None,
                is_calculating: None,
                created_by: None,
                created_at: None,
                last_calculation: None,
                errors_calculating: None,
                count: None,
                is_static: None,
                experiment_set: None,
            },
            Cohort {
                id: 42,
                name: "power-users".to_string(),
                description: None,
                groups: None,
                deleted: false,
                filters: serde_json::json!({}),
                query: None,
                is_calculating: None,
                created_by: None,
                created_at: None,
                last_calculation: None,
                errors_calculating: None,
                count: None,
                is_static: None,
                experiment_set: None,
            },
        ];

        let found = candidates.into_iter().find(|c| c.name == "power-users");
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, 42);
    }

    #[test]
    fn cohort_name_resolution_not_found() {
        let candidates: Vec<Cohort> = vec![];
        let found = candidates.into_iter().find(|c| c.name == "nonexistent");
        assert!(found.is_none());
    }
}
