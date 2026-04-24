// src/commands/person.rs
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::context::CommandContext;
use crate::commands::util::env_id_required;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Person {
    // PostHog's /persons/ LIST returns id as the UUID string; /persons/:uuid/ GET
    // returns id as the integer DB PK. Accept either shape and stringify.
    #[serde(deserialize_with = "deserialize_string_or_int")]
    pub id: String,
    #[serde(default)]
    pub uuid: Option<String>,
    #[serde(default)]
    pub distinct_ids: Vec<String>,
    #[serde(default)]
    pub properties: Value, // fluid
    #[serde(default)]
    pub is_identified: Option<bool>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

fn deserialize_string_or_int<'de, D>(d: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Either {
        S(String),
        I(i64),
    }
    Ok(match Either::deserialize(d)? {
        Either::S(s) => s,
        Either::I(i) => i.to_string(),
    })
}

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct PersonArgs {
    #[command(subcommand)]
    pub command: PersonCommand,
}

#[derive(Subcommand, Debug)]
pub enum PersonCommand {
    /// List persons with optional filters.
    List {
        #[arg(long)]
        distinct_id: Option<String>,
        #[arg(long)]
        email: Option<String>,
        #[arg(long)]
        search: Option<String>,
        /// Filter by properties (JSON object).
        #[arg(long)]
        properties: Option<String>,
        /// Cap results at N rows (default: fetch all pages).
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Get a single person by distinct_id.
    Get {
        /// The person's distinct_id.
        distinct_id: String,
    },
    /// Hard-delete a person by distinct_id (GDPR; irreversible).
    Delete {
        /// The person's distinct_id.
        distinct_id: String,
    },
    /// Set a property value on a person.
    UpdateProperty {
        /// The person's distinct_id.
        distinct_id: String,
        #[arg(long)]
        key: String,
        #[arg(long)]
        value: String,
    },
    /// Delete a property from a person.
    DeleteProperty {
        /// The person's distinct_id.
        distinct_id: String,
        #[arg(long)]
        key: String,
    },
    /// View the activity log for a person.
    Activity {
        /// The person's distinct_id.
        distinct_id: String,
    },
    /// Split a person's distinct_ids by moving all but main to a new person.
    Split {
        /// The person's distinct_id.
        distinct_id: String,
        /// The distinct_id to keep on the original person.
        #[arg(long)]
        main_distinct_id: String,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: PersonArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        PersonCommand::List {
            distinct_id,
            email,
            search,
            properties,
            limit,
        } => list_persons(cx, distinct_id, email, search, properties, limit).await,
        PersonCommand::Get { distinct_id } => get_person(cx, &distinct_id).await,
        PersonCommand::Delete { distinct_id } => delete_person(cx, &distinct_id).await,
        PersonCommand::UpdateProperty {
            distinct_id,
            key,
            value,
        } => update_property(cx, &distinct_id, key, value).await,
        PersonCommand::DeleteProperty { distinct_id, key } => {
            delete_property(cx, &distinct_id, key).await
        }
        PersonCommand::Activity { distinct_id } => activity_person(cx, &distinct_id).await,
        PersonCommand::Split {
            distinct_id,
            main_distinct_id,
        } => split_person(cx, &distinct_id, main_distinct_id).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Resolve a distinct_id to a person UUID via GET /persons/?distinct_id=...
async fn resolve_uuid(client: &Client, env_id: &str, distinct_id: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Page {
        results: Vec<Person>,
    }

    let path = format!(
        "/api/environments/{env_id}/persons/?distinct_id={}",
        urlencoding::encode(distinct_id)
    );
    let page: Page = client.get(&path).await?;
    page.results
        .into_iter()
        .next()
        .map(|p| p.id)
        .ok_or_else(|| {
            BosshoggError::NotFound(format!("no person found with distinct_id={distinct_id}"))
        })
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Person>,
}

async fn list_persons(
    cx: &CommandContext,
    distinct_id: Option<String>,
    email: Option<String>,
    search: Option<String>,
    properties: Option<String>,
    limit: Option<usize>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut qs_parts: Vec<String> = Vec::new();
    if let Some(d) = &distinct_id {
        qs_parts.push(format!("distinct_id={}", urlencoding::encode(d)));
    }
    if let Some(e) = &email {
        qs_parts.push(format!("email={}", urlencoding::encode(e)));
    }
    if let Some(s) = &search {
        qs_parts.push(format!("search={}", urlencoding::encode(s)));
    }
    if let Some(p) = &properties {
        qs_parts.push(format!("properties={}", urlencoding::encode(p)));
    }

    let qs = if qs_parts.is_empty() {
        String::new()
    } else {
        format!("?{}", qs_parts.join("&"))
    };

    let path = format!("/api/environments/{env_id}/persons/{qs}");
    let results: Vec<Person> = client.get_paginated(&path, limit).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["UUID", "DISTINCT_IDS", "NAME", "IDENTIFIED", "CREATED_AT"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|p| {
                vec![
                    p.id.clone(),
                    p.distinct_ids.join(", "),
                    p.name.clone().unwrap_or_else(|| "-".into()),
                    p.is_identified
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    p.created_at.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_person(cx: &CommandContext, distinct_id: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let uuid = resolve_uuid(client, env_id, distinct_id).await?;
    let person: Person = client
        .get(&format!("/api/environments/{env_id}/persons/{uuid}/"))
        .await?;
    print_person(&person, cx.json_mode);
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_person(cx: &CommandContext, distinct_id: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "HARD-delete person with distinct_id={distinct_id}? This is irreversible (GDPR). Continue?"
    ))?;

    let uuid = resolve_uuid(client, env_id, distinct_id).await?;
    client
        .delete(&format!("/api/environments/{env_id}/persons/{uuid}/"))
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            uuid: String,
            distinct_id: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "delete",
            uuid,
            distinct_id: distinct_id.to_string(),
        });
    } else {
        println!("Deleted person (distinct_id={distinct_id})");
    }
    Ok(())
}

// ── update-property ───────────────────────────────────────────────────────────

async fn update_property(
    cx: &CommandContext,
    distinct_id: &str,
    key: String,
    value: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "update property '{key}' on person with distinct_id={distinct_id}; continue?"
    ))?;

    let uuid = resolve_uuid(client, env_id, distinct_id).await?;
    let body = json!({ "$set": { &key: value } });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/persons/{uuid}/update_property/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Updated property '{key}' on person (distinct_id={distinct_id})");
    }
    Ok(())
}

// ── delete-property ───────────────────────────────────────────────────────────

async fn delete_property(cx: &CommandContext, distinct_id: &str, key: String) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "delete property '{key}' from person with distinct_id={distinct_id}; continue?"
    ))?;

    let uuid = resolve_uuid(client, env_id, distinct_id).await?;
    let body = json!({ "$unset": [&key] });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/persons/{uuid}/delete_property/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Deleted property '{key}' from person (distinct_id={distinct_id})");
    }
    Ok(())
}

// ── activity ──────────────────────────────────────────────────────────────────

async fn activity_person(cx: &CommandContext, distinct_id: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let uuid = resolve_uuid(client, env_id, distinct_id).await?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/persons/{uuid}/activity/"
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

// ── split ─────────────────────────────────────────────────────────────────────

async fn split_person(
    cx: &CommandContext,
    distinct_id: &str,
    main_distinct_id: String,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "split person with distinct_id={distinct_id}, keeping {main_distinct_id} as main; continue?"
    ))?;

    let uuid = resolve_uuid(client, env_id, distinct_id).await?;
    let body = json!({ "main_distinct_id": main_distinct_id });
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/persons/{uuid}/split/"),
            &body,
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Split person (distinct_id={distinct_id}), main={main_distinct_id}");
    }
    Ok(())
}

// ── print helper ──────────────────────────────────────────────────────────────

fn print_person(person: &Person, json_mode: bool) {
    if json_mode {
        output::print_json(person);
    } else {
        // Prefer the dedicated `uuid` field; fall back to `id` (which is the
        // UUID on list responses and the DB PK on get-by-uuid responses).
        let uuid_display = person.uuid.as_deref().unwrap_or(&person.id);
        println!("UUID:        {uuid_display}");
        println!("Distinct IDs: {}", person.distinct_ids.join(", "));
        if let Some(n) = person.name.as_deref() {
            println!("Name:        {n}");
        }
        println!(
            "Identified:  {}",
            person
                .is_identified
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".into())
        );
        if let Some(ca) = person.created_at.as_deref() {
            println!("Created:     {ca}");
        }
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn person_roundtrip_minimal() {
        let raw = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "distinct_ids": ["user@example.com"],
            "properties": {}
        }"#;
        let p: Person = serde_json::from_str(raw).unwrap();
        assert_eq!(p.id, "550e8400-e29b-41d4-a716-446655440000");
        assert_eq!(p.distinct_ids, vec!["user@example.com"]);
    }

    #[test]
    fn person_roundtrip_full() {
        let raw = r#"{
            "id": "550e8400-e29b-41d4-a716-446655440000",
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "distinct_ids": ["user@example.com", "user123"],
            "properties": {"email": "user@example.com", "plan": "pro"},
            "is_identified": true,
            "created_at": "2026-01-01T00:00:00Z",
            "name": "Test User"
        }"#;
        let p: Person = serde_json::from_str(raw).unwrap();
        assert_eq!(p.is_identified, Some(true));
        assert_eq!(p.name, Some("Test User".into()));
        assert_eq!(p.distinct_ids.len(), 2);
    }

    #[test]
    fn person_missing_optional_fields_ok() {
        let raw = r#"{
            "id": "abc-123",
            "distinct_ids": [],
            "properties": null
        }"#;
        let p: Person = serde_json::from_str(raw).unwrap();
        assert_eq!(p.id, "abc-123");
        assert!(p.distinct_ids.is_empty());
        assert_eq!(p.created_at, None);
    }
}
