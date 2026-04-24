// src/commands/group.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::context::CommandContext;
use crate::commands::util::env_id_required;
use crate::error::Result;
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Group {
    pub group_type_index: i32,
    pub group_key: String,
    #[serde(default)]
    pub group_properties: Option<Value>, // fluid
    #[serde(default)]
    pub created_at: Option<String>,
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct GroupArgs {
    #[command(subcommand)]
    pub command: GroupCommand,
}

#[derive(Subcommand, Debug)]
pub enum GroupCommand {
    /// List groups of a given type.
    List {
        /// Group type index (required by PostHog API — use `/api/:project_id/groups_types/` to discover valid indices).
        #[arg(long)]
        group_type_index: i32,
    },
    /// Find a single group by type index and key.
    Find {
        #[arg(long)]
        group_type_index: i32,
        #[arg(long)]
        group_key: String,
    },
    /// List property definitions for groups.
    PropertyDefinitions {
        #[arg(long)]
        group_type_index: Option<i32>,
    },
    /// List property values for a specific property key.
    PropertyValues {
        #[arg(long)]
        key: String,
        /// Group type index (required by PostHog API).
        #[arg(long)]
        group_type_index: i32,
    },
    /// List related persons and groups for a group.
    Related {
        #[arg(long)]
        group_type_index: i32,
        #[arg(long)]
        group_key: String,
    },
    /// View the activity log for groups.
    Activity {
        /// Group type index (required by PostHog API).
        #[arg(long)]
        group_type_index: i32,
    },
    /// Set a property value on a group.
    UpdateProperty {
        #[arg(long)]
        group_type_index: i32,
        #[arg(long)]
        group_key: String,
        #[arg(long)]
        prop_key: String,
        #[arg(long)]
        prop_value: String,
    },
    /// Delete a property from a group.
    DeleteProperty {
        #[arg(long)]
        group_type_index: i32,
        #[arg(long)]
        group_key: String,
        #[arg(long)]
        prop_key: String,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: GroupArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        GroupCommand::List { group_type_index } => list_groups(cx, group_type_index).await,
        GroupCommand::Find {
            group_type_index,
            group_key,
        } => find_group(cx, group_type_index, group_key).await,
        GroupCommand::PropertyDefinitions { group_type_index } => {
            property_definitions(cx, group_type_index).await
        }
        GroupCommand::PropertyValues {
            key,
            group_type_index,
        } => property_values(cx, key, group_type_index).await,
        GroupCommand::Related {
            group_type_index,
            group_key,
        } => related_groups(cx, group_type_index, group_key).await,
        GroupCommand::Activity { group_type_index } => activity_groups(cx, group_type_index).await,
        GroupCommand::UpdateProperty {
            group_type_index,
            group_key,
            prop_key,
            prop_value,
        } => update_property(cx, group_type_index, group_key, prop_key, prop_value).await,
        GroupCommand::DeleteProperty {
            group_type_index,
            group_key,
            prop_key,
        } => delete_property(cx, group_type_index, group_key, prop_key).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Group>,
}

async fn list_groups(cx: &CommandContext, group_type_index: i32) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let path = format!("/api/environments/{env_id}/groups/?group_type_index={group_type_index}");
    let results: Vec<Group> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["TYPE_INDEX", "GROUP_KEY", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|g| {
                vec![
                    g.group_type_index.to_string(),
                    g.group_key.clone(),
                    g.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── find ──────────────────────────────────────────────────────────────────────

async fn find_group(cx: &CommandContext, group_type_index: i32, group_key: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!(
        "/api/environments/{env_id}/groups/find/?group_type_index={group_type_index}&group_key={}",
        urlencoding::encode(&group_key)
    );
    let group: Group = client.get(&path).await?;
    print_group(&group, cx.json_mode);
    Ok(())
}

// ── property-definitions ──────────────────────────────────────────────────────

async fn property_definitions(cx: &CommandContext, group_type_index: Option<i32>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let qs = match group_type_index {
        Some(idx) => format!("?group_type_index={idx}"),
        None => String::new(),
    };

    let path = format!("/api/environments/{env_id}/groups/property_definitions/{qs}");
    let v: Value = client.get(&path).await?;

    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(results) = v.get("results").and_then(Value::as_array) {
        let headers = &["NAME", "TYPE", "PROPERTY_TYPE"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|r| {
                vec![
                    r.get("name").and_then(Value::as_str).unwrap_or("-").into(),
                    r.get("type").and_then(Value::as_str).unwrap_or("-").into(),
                    r.get("property_type")
                        .and_then(Value::as_str)
                        .unwrap_or("-")
                        .into(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    } else {
        output::print_json(&v);
    }
    Ok(())
}

// ── property-values ───────────────────────────────────────────────────────────

async fn property_values(cx: &CommandContext, key: String, group_type_index: i32) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let path = format!(
        "/api/environments/{env_id}/groups/property_values/?key={}&group_type_index={group_type_index}",
        urlencoding::encode(&key)
    );
    let v: Value = client.get(&path).await?;

    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(arr) = v.as_array() {
        for val in arr {
            let s = val
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| val.as_str().unwrap_or("-"));
            println!("{s}");
        }
    } else {
        output::print_json(&v);
    }
    Ok(())
}

// ── related ───────────────────────────────────────────────────────────────────

async fn related_groups(
    cx: &CommandContext,
    group_type_index: i32,
    group_key: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!(
        "/api/environments/{env_id}/groups/related/?group_type_index={group_type_index}&group_key={}",
        urlencoding::encode(&group_key)
    );
    let v: Value = client.get(&path).await?;

    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(arr) = v.as_array() {
        println!("Related for group type={group_type_index} key={group_key}:");
        for item in arr {
            let kind = item.get("type").and_then(Value::as_str).unwrap_or("-");
            let id = item
                .get("id")
                .and_then(Value::as_str)
                .or_else(|| item.get("distinct_id").and_then(Value::as_str))
                .unwrap_or("-");
            println!("  {kind}  {id}");
        }
    } else {
        output::print_json(&v);
    }
    Ok(())
}

// ── activity ──────────────────────────────────────────────────────────────────

async fn activity_groups(cx: &CommandContext, group_type_index: i32) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let path =
        format!("/api/environments/{env_id}/groups/activity/?group_type_index={group_type_index}");
    let v: Value = client.get(&path).await?;

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

// ── update-property ───────────────────────────────────────────────────────────

async fn update_property(
    cx: &CommandContext,
    group_type_index: i32,
    group_key: String,
    prop_key: String,
    prop_value: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "update property '{prop_key}' on group type={group_type_index} key={group_key}; continue?"
    ))?;

    let body = json!({
        "group_type_index": group_type_index,
        "group_key": group_key,
        "$set": { &prop_key: prop_value }
    });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/groups/update_property/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Updated property '{prop_key}' on group type={group_type_index} key={group_key}");
    }
    Ok(())
}

// ── delete-property ───────────────────────────────────────────────────────────

async fn delete_property(
    cx: &CommandContext,
    group_type_index: i32,
    group_key: String,
    prop_key: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "delete property '{prop_key}' from group type={group_type_index} key={group_key}; continue?"
    ))?;

    let body = json!({
        "group_type_index": group_type_index,
        "group_key": group_key,
        "$unset": [&prop_key]
    });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/groups/delete_property/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!(
            "Deleted property '{prop_key}' from group type={group_type_index} key={group_key}"
        );
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_group(group: &Group, json_mode: bool) {
    if json_mode {
        output::print_json(group);
    } else {
        println!("Type Index:  {}", group.group_type_index);
        println!("Group Key:   {}", group.group_key);
        if let Some(ca) = group.created_at.as_deref() {
            println!("Created:     {ca}");
        }
        if let Some(props) = group.group_properties.as_ref() {
            println!("Properties:  {props}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn group_roundtrip_minimal() {
        let raw = r#"{
            "group_type_index": 0,
            "group_key": "org_123"
        }"#;
        let g: Group = serde_json::from_str(raw).unwrap();
        assert_eq!(g.group_type_index, 0);
        assert_eq!(g.group_key, "org_123");
        assert!(g.group_properties.is_none());
    }

    #[test]
    fn group_roundtrip_full() {
        let raw = r#"{
            "group_type_index": 2,
            "group_key": "acme_corp",
            "group_properties": {"plan": "enterprise", "seats": 100},
            "created_at": "2026-01-01T00:00:00Z"
        }"#;
        let g: Group = serde_json::from_str(raw).unwrap();
        assert_eq!(g.group_type_index, 2);
        assert_eq!(g.group_key, "acme_corp");
        assert!(g.group_properties.is_some());
        assert_eq!(g.created_at, Some("2026-01-01T00:00:00Z".into()));
    }

    #[test]
    fn group_null_properties_ok() {
        let raw = r#"{
            "group_type_index": 1,
            "group_key": "test",
            "group_properties": null
        }"#;
        let g: Group = serde_json::from_str(raw).unwrap();
        assert!(g.group_properties.is_none());
    }
}
