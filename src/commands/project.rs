// src/commands/project.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;
use crate::{config, config::Config};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Project {
    pub id: i64,
    #[serde(default)]
    pub uuid: Option<String>,
    pub name: String,
    #[serde(default)]
    pub organization: Option<String>,
    #[serde(default)]
    pub api_token: Option<String>, // phc_... project token
    #[serde(default)]
    pub app_urls: Option<Vec<String>>,
    #[serde(default)]
    pub data_attributes: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub session_recording_opt_in: Option<bool>,
    #[serde(default)]
    pub has_completed_onboarding_for: Option<serde_json::Value>,
    #[serde(default)]
    pub timezone: Option<String>,
    #[serde(default)]
    pub week_start_day: Option<u8>,
}

#[derive(Args, Debug)]
pub struct ProjectArgs {
    #[command(subcommand)]
    pub command: ProjectCommand,
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// List projects in the active org.
    List,
    /// Get a project by numeric ID or name.
    Get {
        /// Project numeric ID or name string.
        identifier: String,
    },
    /// Show the currently active project (from config).
    Current,
    /// Switch the active project in config (no API call).
    Switch {
        /// Project numeric ID to make active.
        identifier: String,
    },
    /// Rotate the project token. Existing clients using the old token stop working immediately.
    ResetToken {
        /// Project numeric ID.
        id: i64,
    },
}

pub async fn execute(args: &ProjectArgs, cx: &CommandContext) -> Result<()> {
    match &args.command {
        // switch is config-local — just mutates cx.context_name's config entry
        ProjectCommand::Switch { identifier } => {
            switch_project(identifier, cx.json_mode, cx.context_name.as_deref()).await
        }
        ProjectCommand::List => list_projects(cx).await,
        ProjectCommand::Get { identifier } => get_project(cx, identifier).await,
        ProjectCommand::Current => current_project(cx).await,
        ProjectCommand::ResetToken { id } => reset_token(cx, *id).await,
    }
}

async fn list_projects(cx: &CommandContext) -> Result<()> {
    let org_id = cx.client.org_id().ok_or_else(|| {
        BosshoggError::Config(
            "no org_id set; run `bosshogg configure` or `bosshogg org switch <id>`".into(),
        )
    })?;

    let projects: Vec<Project> = cx.client
        .get_paginated(&format!("/api/organizations/{org_id}/projects/"), None)
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            count: usize,
            results: Vec<Project>,
        }
        output::print_json(&Out {
            count: projects.len(),
            results: projects,
        });
    } else {
        let headers = &["ID", "NAME", "TIMEZONE", "CREATED"];
        let rows: Vec<Vec<String>> = projects
            .iter()
            .map(|p| {
                vec![
                    p.id.to_string(),
                    p.name.clone(),
                    p.timezone.clone().unwrap_or_default(),
                    p.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn resolve_project_by_identifier(
    client: &Client,
    identifier: &str,
    org_id: &str,
) -> Result<Project> {
    // Numeric → direct GET /api/projects/:id/
    if let Ok(id) = identifier.parse::<i64>() {
        let p: Project = client.get(&format!("/api/projects/{id}/")).await?;
        return Ok(p);
    }
    // Name → list all in org and filter
    let all: Vec<Project> = client
        .get_paginated(&format!("/api/organizations/{org_id}/projects/"), None)
        .await?;
    all.into_iter()
        .find(|p| p.name == identifier)
        .ok_or_else(|| BosshoggError::NotFound(format!("project '{identifier}'")))
}

async fn get_project(cx: &CommandContext, identifier: &str) -> Result<()> {
    let project = if identifier.parse::<i64>().is_ok() {
        // Numeric: direct fetch
        let p: Project = cx.client.get(&format!("/api/projects/{identifier}/")).await?;
        p
    } else {
        // Name: need org_id for list endpoint
        let org_id = cx.client.org_id().ok_or_else(|| {
            BosshoggError::Config(
                "no org_id set; needed to list projects by name. Run `bosshogg org switch <id>`"
                    .into(),
            )
        })?;
        resolve_project_by_identifier(&cx.client, identifier, org_id).await?
    };

    print_project(&project, cx.json_mode);
    Ok(())
}

async fn current_project(cx: &CommandContext) -> Result<()> {
    let project_id = cx.client.project_id().ok_or_else(|| {
        BosshoggError::Config(
            "no project_id set; run `bosshogg configure` or `bosshogg project switch <id>`".into(),
        )
    })?;
    let project: Project = cx.client.get(&format!("/api/projects/{project_id}/")).await?;
    print_project(&project, cx.json_mode);
    Ok(())
}

async fn switch_project(identifier: &str, json_mode: bool, context: Option<&str>) -> Result<()> {
    let mut cfg: Config = config::load()?;
    let current = cfg.current_context.clone().ok_or_else(|| {
        BosshoggError::Config("no current context; run `bosshogg configure` first".into())
    })?;
    let ctx_name = context.unwrap_or(&current).to_string();
    let ctx = cfg
        .contexts
        .get_mut(&ctx_name)
        .ok_or_else(|| BosshoggError::Config(format!("context `{ctx_name}` not found in config")))?;
    ctx.project_id = Some(identifier.to_string());
    config::save(&cfg)?;

    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            context: &'a str,
            project_id: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            context: &ctx_name,
            project_id: identifier,
        });
    } else {
        println!("Switched project to `{identifier}` in context `{ctx_name}`.");
    }
    Ok(())
}

async fn reset_token(cx: &CommandContext, id: i64) -> Result<()> {
    cx.confirm(&format!(
        "DESTRUCTIVE: rotate project token for project {id}? \
        All SDK clients using the current token will STOP WORKING immediately. Continue?"
    ))?;

    #[derive(Deserialize)]
    struct ResetTokenResp {
        api_token: String,
    }

    let resp: ResetTokenResp = cx
        .client
        .patch(
            &format!("/api/projects/{id}/reset_token/"),
            &serde_json::json!({}),
        )
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            project_id: i64,
            new_api_token: String,
        }
        output::print_json(&Out {
            ok: true,
            project_id: id,
            new_api_token: resp.api_token,
        });
    } else {
        println!(
            "WARNING: Project token rotated for project {id}.\n\
            OLD token is DEAD — any SDK clients or integrations using it are now broken.\n\
            New project token: {}\n\
            Update POSTHOG_API_KEY / project_token in every env/SDK that used the old value.",
            resp.api_token
        );
    }
    Ok(())
}

fn print_project(project: &Project, json_mode: bool) {
    if json_mode {
        output::print_json(project);
    } else {
        println!("ID:         {}", project.id);
        println!("Name:       {}", project.name);
        if let Some(org) = project.organization.as_deref() {
            println!("Org:        {org}");
        }
        if let Some(tz) = project.timezone.as_deref() {
            println!("Timezone:   {tz}");
        }
        if let Some(token) = project.api_token.as_deref() {
            println!("Token:      {token}");
        }
        if let Some(c) = project.created_at.as_deref() {
            println!("Created:    {c}");
        }
        if let Some(rec) = project.session_recording_opt_in {
            println!("Recording:  {rec}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_roundtrip() {
        let raw = r#"{"id":1,"name":"My Project","timezone":"UTC","api_token":"phc_test"}"#;
        let p: Project = serde_json::from_str(raw).unwrap();
        assert_eq!(p.id, 1);
        assert_eq!(p.api_token.as_deref(), Some("phc_test"));
    }

    #[test]
    fn project_accepts_extra_fields() {
        let raw = r#"{"id":2,"name":"X","extra_future_field":true}"#;
        let p: Project = serde_json::from_str(raw).unwrap();
        assert_eq!(p.id, 2);
    }
}
