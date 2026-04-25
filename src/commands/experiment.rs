// src/commands/experiment.rs
//! `bosshogg experiment` — list / get / create / update / delete / archive /
//! duplicate / copy-to-project / create-exposure-cohort /
//! launch / end / pause / resume / reset / ship-variant /
//! recalculate-timeseries / results.
//!
//! Experiments are project-scoped. Deletion is soft (PATCH {"deleted": true}).

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
pub struct Experiment {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    pub feature_flag_key: String,
    #[serde(default)]
    pub feature_flag: Option<Value>,
    #[serde(default)]
    pub exposure_cohort: Option<i64>,
    pub parameters: Value, // fluid
    #[serde(default)]
    pub secondary_metrics: Option<Value>,
    #[serde(default)]
    pub metrics: Option<Value>,
    #[serde(default)]
    pub saved_metrics: Option<Value>,
    #[serde(default)]
    pub archived: Option<bool>,
    #[serde(default)]
    pub deleted: Option<bool>,
    #[serde(default)]
    pub filters: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct ExperimentArgs {
    #[command(subcommand)]
    pub command: ExperimentCommand,
}

#[derive(Subcommand, Debug)]
pub enum ExperimentCommand {
    /// List experiments with optional search filter.
    List {
        #[arg(long)]
        search: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single experiment by numeric id.
    Get { id: i64 },
    /// Create a new experiment.
    Create {
        #[arg(long)]
        name: String,
        /// Feature flag key to associate with the experiment.
        #[arg(long)]
        feature_flag_key: String,
        /// Path to a JSON file containing the parameters object.
        #[arg(long)]
        parameters_file: PathBuf,
    },
    /// Update experiment fields.
    Update {
        id: i64,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Path to a JSON file with updated parameters.
        #[arg(long)]
        parameters_file: Option<PathBuf>,
    },
    /// Soft-delete an experiment (PATCH with deleted=true).
    Delete { id: i64 },
    /// Archive an experiment.
    Archive { id: i64 },
    /// Duplicate an experiment.
    Duplicate { id: i64 },
    /// Copy an experiment to another project.
    #[command(name = "copy-to-project")]
    CopyToProject {
        id: i64,
        /// Target project ID to copy the experiment into.
        #[arg(long)]
        target_project_id: String,
    },
    /// Create an exposure cohort for an experiment.
    #[command(name = "create-exposure-cohort")]
    CreateExposureCohort { id: i64 },
    /// Launch an experiment (transitions it to running state).
    Launch { id: i64 },
    /// End a running experiment.
    End { id: i64 },
    /// Pause a running experiment.
    Pause { id: i64 },
    /// Resume a paused experiment.
    Resume { id: i64 },
    /// Reset an experiment's results (clears collected data).
    Reset { id: i64 },
    /// Declare a winning variant; sets that variant's rollout to 100% on the
    /// underlying feature flag.
    #[command(name = "ship-variant")]
    ShipVariant {
        id: i64,
        /// Key of the variant to ship (e.g. "test" or "control").
        #[arg(long)]
        variant: String,
    },
    /// Recalculate timeseries results for an experiment.
    #[command(name = "recalculate-timeseries")]
    RecalculateTimeseries { id: i64 },
    /// Fetch timeseries results for a specific metric on an experiment.
    Results {
        id: i64,
        /// UUID of the metric to fetch results for (required by PostHog).
        #[arg(long)]
        metric_uuid: String,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: ExperimentArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        ExperimentCommand::List { search, limit } => list_experiments(cx, search, limit).await,
        ExperimentCommand::Get { id } => get_experiment(cx, id).await,
        ExperimentCommand::Create {
            name,
            feature_flag_key,
            parameters_file,
        } => create_experiment(cx, name, feature_flag_key, parameters_file).await,
        ExperimentCommand::Update {
            id,
            name,
            description,
            parameters_file,
        } => update_experiment(cx, id, name, description, parameters_file).await,
        ExperimentCommand::Delete { id } => delete_experiment(cx, id).await,
        ExperimentCommand::Archive { id } => archive_experiment(cx, id).await,
        ExperimentCommand::Duplicate { id } => duplicate_experiment(cx, id).await,
        ExperimentCommand::CopyToProject {
            id,
            target_project_id,
        } => copy_to_project(cx, id, target_project_id).await,
        ExperimentCommand::CreateExposureCohort { id } => create_exposure_cohort(cx, id).await,
        ExperimentCommand::Launch { id } => launch_experiment(cx, id).await,
        ExperimentCommand::End { id } => end_experiment(cx, id).await,
        ExperimentCommand::Pause { id } => pause_experiment(cx, id).await,
        ExperimentCommand::Resume { id } => resume_experiment(cx, id).await,
        ExperimentCommand::Reset { id } => reset_experiment(cx, id).await,
        ExperimentCommand::ShipVariant { id, variant } => ship_variant(cx, id, variant).await,
        ExperimentCommand::RecalculateTimeseries { id } => recalculate_timeseries(cx, id).await,
        ExperimentCommand::Results { id, metric_uuid } => {
            experiment_results(cx, id, metric_uuid).await
        }
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
    results: Vec<Experiment>,
}

async fn list_experiments(
    cx: &CommandContext,
    search: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let query = if let Some(s) = search {
        format!("?search={}", urlencoding::encode(&s))
    } else {
        String::new()
    };

    let path = format!("/api/projects/{project_id}/experiments/{query}");
    let results: Vec<Experiment> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "FEATURE_FLAG_KEY", "ARCHIVED", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|e| {
                vec![
                    e.id.to_string(),
                    e.name.clone(),
                    e.feature_flag_key.clone(),
                    e.archived
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| "-".into()),
                    e.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let experiment: Experiment = client
        .get(&format!("/api/projects/{project_id}/experiments/{id}/"))
        .await?;
    print_experiment(&experiment, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_experiment(
    cx: &CommandContext,
    name: String,
    feature_flag_key: String,
    parameters_file: PathBuf,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let parameters = read_json_file(&parameters_file).await?;

    let body = json!({
        "name": name,
        "feature_flag_key": feature_flag_key,
        "parameters": parameters,
    });

    let created: Experiment = client
        .post(&format!("/api/projects/{project_id}/experiments/"), &body)
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
        println!("Created experiment '{}' (id {})", created.name, created.id);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_experiment(
    cx: &CommandContext,
    id: i64,
    name: Option<String>,
    description: Option<String>,
    parameters_file: Option<PathBuf>,
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
    if let Some(p) = parameters_file.as_deref() {
        body.insert("parameters".into(), read_json_file(p).await?);
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --description, --parameters-file)".into(),
        ));
    }

    cx.confirm(&format!("update experiment `{id}`; continue?"))?;

    let updated: Experiment = client
        .patch(
            &format!("/api/projects/{project_id}/experiments/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated experiment '{}' (id {})", updated.name, updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("delete experiment `{id}`; continue?"))?;

    client
        .patch(
            &format!("/api/projects/{project_id}/experiments/{id}/"),
            &serde_json::json!({"deleted": true}),
        )
        .await
        .map(|_: Value| ())?;

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
        println!("Deleted experiment {id}");
    }
    Ok(())
}

// ── archive ───────────────────────────────────────────────────────────────────

async fn archive_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("archive experiment `{id}`; continue?"))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/archive/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Archived experiment {id}");
    }
    Ok(())
}

// ── duplicate ─────────────────────────────────────────────────────────────────

async fn duplicate_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("duplicate experiment `{id}`; continue?"))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/duplicate/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        let new_id = v.get("id").and_then(Value::as_i64).unwrap_or(0);
        println!("Duplicated experiment {id} → new id {new_id}");
    }
    Ok(())
}

// ── copy-to-project ───────────────────────────────────────────────────────────

async fn copy_to_project(cx: &CommandContext, id: i64, target_project_id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "copy experiment `{id}` to project `{target_project_id}`; continue?"
    ))?;

    let body = json!({ "team_id": target_project_id });
    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/copy_to_project/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Copied experiment {id} to project {target_project_id}");
    }
    Ok(())
}

// ── create-exposure-cohort ────────────────────────────────────────────────────

async fn create_exposure_cohort(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "create exposure cohort for experiment `{id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!(
                "/api/projects/{project_id}/experiments/{id}/create_exposure_cohort_for_experiment/"
            ),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        let cohort_id = v.get("cohort_id").and_then(Value::as_i64).unwrap_or(0);
        println!("Created exposure cohort {cohort_id} for experiment {id}");
    }
    Ok(())
}

// ── launch ────────────────────────────────────────────────────────────────────

async fn launch_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("launch experiment `{id}`; continue?"))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/launch/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Launched experiment {id}");
    }
    Ok(())
}

// ── end ───────────────────────────────────────────────────────────────────────

async fn end_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("end experiment `{id}`; continue?"))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/end/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Ended experiment {id}");
    }
    Ok(())
}

// ── pause ─────────────────────────────────────────────────────────────────────

async fn pause_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("pause experiment `{id}`; continue?"))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/pause/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Paused experiment {id}");
    }
    Ok(())
}

// ── resume ────────────────────────────────────────────────────────────────────

async fn resume_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("resume experiment `{id}`; continue?"))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/resume/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Resumed experiment {id}");
    }
    Ok(())
}

// ── reset ─────────────────────────────────────────────────────────────────────

async fn reset_experiment(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "reset experiment `{id}` (clears collected data); continue?"
    ))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/reset/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Reset experiment {id}");
    }
    Ok(())
}

// ── ship-variant ──────────────────────────────────────────────────────────────

async fn ship_variant(cx: &CommandContext, id: i64, variant: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "ship variant `{variant}` for experiment `{id}` (sets rollout to 100%); continue?"
    ))?;

    let body = json!({ "variant_key": variant });
    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/ship_variant/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Shipped variant '{variant}' for experiment {id}");
    }
    Ok(())
}

// ── recalculate-timeseries ────────────────────────────────────────────────────

async fn recalculate_timeseries(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "recalculate timeseries for experiment `{id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/experiments/{id}/recalculate_timeseries/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Recalculated timeseries for experiment {id}");
    }
    Ok(())
}

// ── results ───────────────────────────────────────────────────────────────────

async fn experiment_results(cx: &CommandContext, id: i64, metric_uuid: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let encoded = urlencoding::encode(&metric_uuid);
    let path = format!(
        "/api/projects/{project_id}/experiments/{id}/timeseries_results/?metric_uuid={encoded}"
    );
    let v: Value = client.get(&path).await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("{v}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_experiment(experiment: &Experiment, json_mode: bool) {
    if json_mode {
        output::print_json(experiment);
    } else {
        println!("ID:               {}", experiment.id);
        println!("Name:             {}", experiment.name);
        if let Some(d) = experiment.description.as_deref() {
            println!("Description:      {d}");
        }
        println!("Feature Flag Key: {}", experiment.feature_flag_key);
        println!("Archived:         {}", experiment.archived.unwrap_or(false));
        if let Some(sd) = experiment.start_date.as_deref() {
            println!("Start Date:       {sd}");
        }
        if let Some(ed) = experiment.end_date.as_deref() {
            println!("End Date:         {ed}");
        }
        if let Some(ca) = experiment.created_at.as_deref() {
            println!("Created:          {ca}");
        }
        if let Some(ua) = experiment.updated_at.as_deref() {
            println!("Updated:          {ua}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn experiment_roundtrip_minimal() {
        let raw = r#"{
            "id": 1,
            "name": "My Experiment",
            "feature_flag_key": "my-flag",
            "parameters": {}
        }"#;
        let e: Experiment = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, 1);
        assert_eq!(e.name, "My Experiment");
        assert_eq!(e.feature_flag_key, "my-flag");
    }

    #[test]
    fn experiment_roundtrip_full() {
        let raw = r#"{
            "id": 42,
            "name": "Full Experiment",
            "description": "Testing things",
            "start_date": "2026-01-01T00:00:00Z",
            "end_date": "2026-06-01T00:00:00Z",
            "feature_flag_key": "full-flag",
            "feature_flag": {"id": 99, "key": "full-flag"},
            "exposure_cohort": 7,
            "parameters": {"minimum_detectable_effect": 0.05},
            "secondary_metrics": [],
            "metrics": [],
            "saved_metrics": [],
            "archived": false,
            "deleted": false,
            "filters": {},
            "created_at": "2026-01-01T00:00:00Z",
            "created_by": {"id": 1, "email": "test@example.com"},
            "updated_at": "2026-04-01T00:00:00Z"
        }"#;
        let e: Experiment = serde_json::from_str(raw).unwrap();
        assert_eq!(e.id, 42);
        assert_eq!(e.exposure_cohort, Some(7));
        assert_eq!(e.archived, Some(false));
    }

    #[test]
    fn experiment_archived_flag_parsed() {
        let raw = r#"{
            "id": 5,
            "name": "Archived Experiment",
            "feature_flag_key": "arch-flag",
            "parameters": {},
            "archived": true
        }"#;
        let e: Experiment = serde_json::from_str(raw).unwrap();
        assert_eq!(e.archived, Some(true));
    }
}
