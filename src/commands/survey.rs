// src/commands/survey.rs
//! `bosshogg survey` — list / get / create / update / delete / activity /
//! duplicate / archive-response.
//!
//! Surveys are project-scoped. Deletion is a HARD DELETE (DELETE HTTP verb).

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
pub struct Survey {
    pub id: String, // UUID
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub survey_type: String, // popover, api, widget, etc.
    pub questions: Value, // fluid
    #[serde(default)]
    pub appearance: Option<Value>,
    #[serde(default)]
    pub conditions: Option<Value>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub end_date: Option<String>,
    #[serde(default)]
    pub linked_flag: Option<Value>,
    #[serde(default)]
    pub targeting_flag: Option<Value>,
    #[serde(default)]
    pub internal_targeting_flag: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub archived: Option<bool>,
    #[serde(default)]
    pub enable_partial_responses: Option<bool>,
    #[serde(default)]
    pub responses_limit: Option<i64>,
    #[serde(default)]
    pub iteration_count: Option<i32>,
    #[serde(default)]
    pub current_iteration: Option<i32>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct SurveyArgs {
    #[command(subcommand)]
    pub command: SurveyCommand,
}

#[derive(Subcommand, Debug)]
pub enum SurveyCommand {
    /// List surveys with optional search filter.
    List {
        #[arg(long)]
        search: Option<String>,
        /// Include archived surveys.
        #[arg(long)]
        archived: bool,
    },
    /// Get a single survey by UUID.
    Get { id: String },
    /// Create a new survey.
    Create {
        #[arg(long)]
        name: String,
        /// Survey type (popover, api, widget, etc.).
        #[arg(long = "type")]
        survey_type: String,
        /// Path to a JSON file containing the questions array.
        #[arg(long)]
        questions_file: PathBuf,
    },
    /// Update survey fields.
    Update {
        id: String,
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        description: Option<String>,
        /// Path to a JSON file with updated questions.
        #[arg(long)]
        questions_file: Option<PathBuf>,
    },
    /// Hard-delete a survey (DELETE HTTP verb).
    Delete { id: String },
    /// Get the activity log for a survey.
    Activity { id: String },
    /// Duplicate a survey to one or more projects.
    Duplicate {
        id: String,
        /// Comma-separated list of target project IDs.
        #[arg(long)]
        target_project_ids: String,
    },
    /// Archive a survey response by UUID.
    #[command(name = "archive-response")]
    ArchiveResponse {
        id: String,
        /// Response UUID to archive.
        #[arg(long)]
        response_uuid: String,
    },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: SurveyArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        SurveyCommand::List { search, archived } => list_surveys(cx, search, archived).await,
        SurveyCommand::Get { id } => get_survey(cx, id).await,
        SurveyCommand::Create {
            name,
            survey_type,
            questions_file,
        } => create_survey(cx, name, survey_type, questions_file).await,
        SurveyCommand::Update {
            id,
            name,
            description,
            questions_file,
        } => update_survey(cx, id, name, description, questions_file).await,
        SurveyCommand::Delete { id } => delete_survey(cx, id).await,
        SurveyCommand::Activity { id } => activity_survey(cx, id).await,
        SurveyCommand::Duplicate {
            id,
            target_project_ids,
        } => duplicate_survey(cx, id, target_project_ids).await,
        SurveyCommand::ArchiveResponse { id, response_uuid } => {
            archive_response(cx, id, response_uuid).await
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
    results: Vec<Survey>,
}

async fn list_surveys(cx: &CommandContext, search: Option<String>, archived: bool) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    let mut params: Vec<String> = Vec::new();
    if let Some(s) = search {
        params.push(format!("search={}", urlencoding::encode(&s)));
    }
    if archived {
        params.push("archived=true".into());
    }
    let query = if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    };

    let path = format!("/api/projects/{project_id}/surveys/{query}");
    let results: Vec<Survey> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "NAME", "TYPE", "ARCHIVED", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|s| {
                vec![
                    s.id.clone(),
                    s.name.clone(),
                    s.survey_type.clone(),
                    s.archived
                        .map(|a| a.to_string())
                        .unwrap_or_else(|| "-".into()),
                    s.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_survey(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let survey: Survey = client
        .get(&format!("/api/projects/{project_id}/surveys/{id}/"))
        .await?;
    print_survey(&survey, cx.json_mode);
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

async fn create_survey(
    cx: &CommandContext,
    name: String,
    survey_type: String,
    questions_file: PathBuf,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let questions = read_json_file(&questions_file).await?;

    let body = json!({
        "name": name,
        "type": survey_type,
        "questions": questions,
    });

    let created: Survey = client
        .post(&format!("/api/projects/{project_id}/surveys/"), &body)
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
        println!("Created survey '{}' (id {})", created.name, created.id);
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_survey(
    cx: &CommandContext,
    id: String,
    name: Option<String>,
    description: Option<String>,
    questions_file: Option<PathBuf>,
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
    if let Some(p) = questions_file.as_deref() {
        body.insert("questions".into(), read_json_file(p).await?);
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --name, --description, --questions-file)".into(),
        ));
    }

    cx.confirm(&format!("update survey `{id}`; continue?"))?;

    let updated: Survey = client
        .patch(
            &format!("/api/projects/{project_id}/surveys/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!("Updated survey '{}' (id {})", updated.name, updated.id);
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_survey(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("hard-delete survey `{id}`; continue?"))?;

    client
        .delete(&format!("/api/projects/{project_id}/surveys/{id}/"))
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
        println!("Deleted survey {id}");
    }
    Ok(())
}

// ── activity ──────────────────────────────────────────────────────────────────

async fn activity_survey(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/projects/{project_id}/surveys/{id}/activity/"
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

// ── duplicate ─────────────────────────────────────────────────────────────────

async fn duplicate_survey(
    cx: &CommandContext,
    id: String,
    target_project_ids: String,
) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!("duplicate survey `{id}`; continue?"))?;

    // Parse comma-separated project IDs into a JSON array of numbers/strings.
    let ids: Vec<Value> = target_project_ids
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<i64>()
                .map(Value::from)
                .unwrap_or_else(|_| Value::String(s.to_owned()))
        })
        .collect();

    let body = json!({ "target_project_ids": ids });
    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/surveys/{id}/duplicate_to_projects/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Duplicated survey {id} to projects: {target_project_ids}");
    }
    Ok(())
}

// ── archive-response ──────────────────────────────────────────────────────────

async fn archive_response(cx: &CommandContext, id: String, response_uuid: String) -> Result<()> {
    let client = &cx.client;
    let project_id = project_id_required(client)?;

    cx.confirm(&format!(
        "archive response `{response_uuid}` for survey `{id}`; continue?"
    ))?;

    let v: Value = client
        .post(
            &format!("/api/projects/{project_id}/surveys/{id}/responses/{response_uuid}/archive/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Archived response {response_uuid} for survey {id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_survey(survey: &Survey, json_mode: bool) {
    if json_mode {
        output::print_json(survey);
    } else {
        println!("ID:          {}", survey.id);
        println!("Name:        {}", survey.name);
        if let Some(d) = survey.description.as_deref() {
            println!("Description: {d}");
        }
        println!("Type:        {}", survey.survey_type);
        println!("Archived:    {}", survey.archived.unwrap_or(false));
        if let Some(ca) = survey.created_at.as_deref() {
            println!("Created:     {ca}");
        }
        if let Some(ua) = survey.updated_at.as_deref() {
            println!("Updated:     {ua}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survey_roundtrip_minimal() {
        let raw = r#"{
            "id": "abc-123",
            "name": "My Survey",
            "type": "popover",
            "questions": []
        }"#;
        let s: Survey = serde_json::from_str(raw).unwrap();
        assert_eq!(s.id, "abc-123");
        assert_eq!(s.name, "My Survey");
        assert_eq!(s.survey_type, "popover");
    }

    #[test]
    fn survey_roundtrip_full() {
        let raw = r##"{
            "id": "uuid-survey-1",
            "name": "Full Survey",
            "description": "A comprehensive survey",
            "type": "api",
            "questions": [{"type": "open", "question": "How are you?"}],
            "appearance": {"background": "#fff"},
            "conditions": null,
            "start_date": "2026-01-01T00:00:00Z",
            "end_date": "2026-06-01T00:00:00Z",
            "linked_flag": null,
            "targeting_flag": null,
            "internal_targeting_flag": null,
            "created_at": "2026-01-01T00:00:00Z",
            "updated_at": "2026-04-01T00:00:00Z",
            "archived": false,
            "enable_partial_responses": true,
            "responses_limit": 500,
            "iteration_count": 1,
            "current_iteration": 1
        }"##;
        let s: Survey = serde_json::from_str(raw).unwrap();
        assert_eq!(s.id, "uuid-survey-1");
        assert_eq!(s.responses_limit, Some(500));
        assert_eq!(s.archived, Some(false));
        assert_eq!(s.enable_partial_responses, Some(true));
    }

    #[test]
    fn survey_type_rename_works() {
        let raw = r#"{"id": "x", "name": "Y", "type": "widget", "questions": []}"#;
        let s: Survey = serde_json::from_str(raw).unwrap();
        assert_eq!(s.survey_type, "widget");
        // Re-serialize should emit "type", not "survey_type"
        let out = serde_json::to_string(&s).unwrap();
        assert!(out.contains("\"type\":\"widget\""));
        assert!(!out.contains("survey_type"));
    }
}
