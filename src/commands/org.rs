// src/commands/org.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};

use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;
use crate::{config, config::Config};

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Org {
    pub id: String, // UUID
    pub name: String,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub membership_level: Option<u8>,
    #[serde(default)]
    pub available_product_features: Option<serde_json::Value>,
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Args, Debug)]
pub struct OrgArgs {
    #[command(subcommand)]
    pub command: OrgCommand,
}

#[derive(Subcommand, Debug)]
pub enum OrgCommand {
    /// List all organizations the API key has access to.
    List,
    /// Get a single organization by ID or slug.
    Get {
        /// Organization UUID or slug.
        identifier: String,
    },
    /// Show the currently active organization (from config).
    Current,
    /// Switch the active organization in config (no API call).
    Switch {
        /// Organization UUID or slug to make active.
        identifier: String,
    },
}

pub async fn execute(args: &OrgArgs, cx: &CommandContext) -> Result<()> {
    match &args.command {
        // switch is config-local — no API call, just mutates cx.context_name's config entry
        OrgCommand::Switch { identifier } => {
            switch_org(identifier, cx.json_mode, cx.context_name.as_deref()).await
        }
        OrgCommand::List => list_orgs(cx).await,
        OrgCommand::Get { identifier } => get_org(cx, identifier).await,
        OrgCommand::Current => current_org(cx).await,
    }
}

async fn list_orgs(cx: &CommandContext) -> Result<()> {
    #[derive(Serialize, Deserialize)]
    struct OrgListPage {
        #[serde(default)]
        results: Vec<Org>,
    }

    let page: OrgListPage = cx.client.get("/api/organizations/").await?;
    let orgs = page.results;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            count: usize,
            results: Vec<Org>,
        }
        output::print_json(&Out {
            count: orgs.len(),
            results: orgs,
        });
    } else {
        let headers = &["ID", "NAME", "SLUG", "CREATED"];
        let rows: Vec<Vec<String>> = orgs
            .iter()
            .map(|o| {
                vec![
                    o.id.clone(),
                    o.name.clone(),
                    o.slug.clone().unwrap_or_default(),
                    o.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

async fn get_org(cx: &CommandContext, identifier: &str) -> Result<()> {
    let org: Org = cx
        .client
        .get(&format!("/api/organizations/{identifier}/"))
        .await?;
    print_org(&org, cx.json_mode);
    Ok(())
}

async fn current_org(cx: &CommandContext) -> Result<()> {
    let org_id = cx.client.org_id().ok_or_else(|| {
        BosshoggError::Config(
            "no org_id set; run `bosshogg configure` or `bosshogg org switch <id>`".into(),
        )
    })?;
    let org: Org = cx
        .client
        .get(&format!("/api/organizations/{org_id}/"))
        .await?;
    print_org(&org, cx.json_mode);
    Ok(())
}

async fn switch_org(identifier: &str, json_mode: bool, context: Option<&str>) -> Result<()> {
    let mut cfg: Config = config::load()?;
    let current = cfg.current_context.clone().ok_or_else(|| {
        BosshoggError::Config("no current context; run `bosshogg configure` first".into())
    })?;
    let ctx_name = context.unwrap_or(&current).to_string();
    let ctx = cfg.contexts.get_mut(&ctx_name).ok_or_else(|| {
        BosshoggError::Config(format!("context `{ctx_name}` not found in config"))
    })?;
    ctx.org_id = Some(identifier.to_string());
    config::save(&cfg)?;

    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            context: &'a str,
            org_id: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            context: &ctx_name,
            org_id: identifier,
        });
    } else {
        println!("Switched org to `{identifier}` in context `{ctx_name}`.");
    }
    Ok(())
}

fn print_org(org: &Org, json_mode: bool) {
    if json_mode {
        output::print_json(org);
    } else {
        println!("ID:      {}", org.id);
        println!("Name:    {}", org.name);
        if let Some(s) = org.slug.as_deref() {
            println!("Slug:    {s}");
        }
        if let Some(l) = org.membership_level {
            println!("Level:   {l}");
        }
        if let Some(c) = org.created_at.as_deref() {
            println!("Created: {c}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn org_roundtrip() {
        let raw = r#"{"id":"org-uuid","name":"Acme","slug":"acme","membership_level":15}"#;
        let o: Org = serde_json::from_str(raw).unwrap();
        assert_eq!(o.id, "org-uuid");
        assert_eq!(o.membership_level, Some(15));
    }
}
