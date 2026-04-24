use serde::{Deserialize, Serialize};

use crate::commands::context::CommandContext;
use crate::error::Result;
use crate::output;

#[derive(Deserialize, Serialize, Debug)]
pub struct WhoamiTeam {
    pub id: serde_json::Value,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct WhoamiOrg {
    pub id: serde_json::Value,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct WhoamiResponse {
    pub email: String,
    #[serde(default)]
    pub first_name: Option<String>,
    #[serde(default)]
    pub last_name: Option<String>,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub organization: Option<WhoamiOrg>,
    #[serde(default)]
    pub team: Option<WhoamiTeam>,
    #[serde(default)]
    pub scopes: Vec<String>,
}

pub async fn execute(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let me: WhoamiResponse = client.get("/api/users/@me/").await?;

    if cx.json_mode {
        output::print_json(&me);
    } else {
        println!("Email:   {}", me.email);
        if let Some(name) = me
            .first_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
        {
            println!("Name:    {name}");
        }
        if let Some(org) = me.organization.as_ref() {
            println!(
                "Org:     {} ({})",
                org.name.as_deref().unwrap_or("-"),
                org.id
            );
        }
        if let Some(team) = me.team.as_ref() {
            println!(
                "Team:    {} ({})",
                team.name.as_deref().unwrap_or("-"),
                team.id
            );
        }
        if !me.scopes.is_empty() {
            println!("Scopes:  {}", me.scopes.join(", "));
        }
    }
    Ok(())
}
