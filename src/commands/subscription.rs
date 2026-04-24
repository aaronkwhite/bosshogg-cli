// src/commands/subscription.rs
//! `bosshogg subscription` — list / get / create / update / delete /
//! test-delivery / deliveries.
//!
//! Subscriptions are environment-scoped.
//! `subscriptions` IS in SOFT_DELETE_RESOURCES — `client.delete()` rewrites to
//! PATCH {"deleted": true}.

use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::commands::context::CommandContext;
use crate::commands::util::env_id_required;
use crate::error::{BosshoggError, Result};
use crate::output;

// ── Typed struct ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Subscription {
    pub id: i64,
    pub title: String,
    pub target_type: String,
    pub target_value: String,
    pub frequency: String,
    #[serde(default)]
    pub interval: Option<i32>,
    #[serde(default)]
    pub byweekday: Option<Vec<String>>,
    #[serde(default)]
    pub bysetpos: Option<i32>,
    #[serde(default)]
    pub start_date: Option<String>,
    #[serde(default)]
    pub until_date: Option<String>,
    #[serde(default)]
    pub insight: Option<i64>,
    #[serde(default)]
    pub dashboard: Option<i64>,
    #[serde(default)]
    pub next_delivery_date: Option<String>,
    pub deleted: bool,
    #[serde(default)]
    pub created_by: Option<Value>,
    #[serde(default)]
    pub created_at: Option<String>,
}

// ── Clap tree ─────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct SubscriptionArgs {
    #[command(subcommand)]
    pub command: SubscriptionCommand,
}

#[derive(Subcommand, Debug)]
pub enum SubscriptionCommand {
    /// List all subscriptions.
    List,
    /// Get a single subscription by numeric id.
    Get { id: i64 },
    /// Create a new subscription.
    Create {
        #[arg(long)]
        title: String,
        /// Target type: "email", "slack", or "webhook".
        #[arg(long)]
        target_type: String,
        /// Target value: email address, slack channel, or webhook URL.
        #[arg(long)]
        target_value: String,
        /// Frequency: "daily", "weekly", or "monthly".
        #[arg(long)]
        frequency: String,
        /// Attach to an insight by numeric id.
        #[arg(long, conflicts_with = "dashboard_id")]
        insight_id: Option<i64>,
        /// Attach to a dashboard by numeric id.
        #[arg(long, conflicts_with = "insight_id")]
        dashboard_id: Option<i64>,
    },
    /// Update a subscription's title or frequency.
    Update {
        id: i64,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        frequency: Option<String>,
    },
    /// Soft-delete a subscription.
    Delete { id: i64 },
    /// Send a test delivery for a subscription (sends a real notification).
    #[command(name = "test-delivery")]
    TestDelivery { id: i64 },
    /// List deliveries for a subscription.
    Deliveries { id: i64 },
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

pub async fn execute(args: SubscriptionArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        SubscriptionCommand::List => list_subscriptions(cx).await,
        SubscriptionCommand::Get { id } => get_subscription(cx, id).await,
        SubscriptionCommand::Create {
            title,
            target_type,
            target_value,
            frequency,
            insight_id,
            dashboard_id,
        } => {
            create_subscription(
                cx,
                title,
                target_type,
                target_value,
                frequency,
                insight_id,
                dashboard_id,
            )
            .await
        }
        SubscriptionCommand::Update {
            id,
            title,
            frequency,
        } => update_subscription(cx, id, title, frequency).await,
        SubscriptionCommand::Delete { id } => delete_subscription(cx, id).await,
        SubscriptionCommand::TestDelivery { id } => test_delivery(cx, id).await,
        SubscriptionCommand::Deliveries { id } => deliveries(cx, id).await,
    }
}

// ── list ──────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct ListOutput {
    count: usize,
    results: Vec<Subscription>,
}

async fn list_subscriptions(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let path = format!("/api/environments/{env_id}/subscriptions/");
    let results: Vec<Subscription> = client.get_paginated(&path, None).await?;

    if cx.json_mode {
        output::print_json(&ListOutput {
            count: results.len(),
            results,
        });
    } else {
        let headers = &["ID", "TITLE", "TYPE", "FREQUENCY", "NEXT_DELIVERY"];
        let rows: Vec<Vec<String>> = results
            .iter()
            .map(|s| {
                vec![
                    s.id.to_string(),
                    s.title.clone(),
                    s.target_type.clone(),
                    s.frequency.clone(),
                    s.next_delivery_date.clone().unwrap_or_default(),
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_subscription(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let s: Subscription = client
        .get(&format!("/api/environments/{env_id}/subscriptions/{id}/"))
        .await?;
    if cx.json_mode {
        output::print_json(&s);
    } else {
        println!("ID:            {}", s.id);
        println!("Title:         {}", s.title);
        println!("Target type:   {}", s.target_type);
        println!("Target value:  {}", s.target_value);
        println!("Frequency:     {}", s.frequency);
        if let Some(ins) = s.insight {
            println!("Insight:       {ins}");
        }
        if let Some(dash) = s.dashboard {
            println!("Dashboard:     {dash}");
        }
        if let Some(nd) = s.next_delivery_date.as_deref() {
            println!("Next delivery: {nd}");
        }
        println!("Deleted:       {}", s.deleted);
    }
    Ok(())
}

// ── create ────────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn create_subscription(
    cx: &CommandContext,
    title: String,
    target_type: String,
    target_value: String,
    frequency: String,
    insight_id: Option<i64>,
    dashboard_id: Option<i64>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = json!({
        "title": title,
        "target_type": target_type,
        "target_value": target_value,
        "frequency": frequency,
    });
    if let Some(ins) = insight_id {
        body["insight"] = json!(ins);
    }
    if let Some(dash) = dashboard_id {
        body["dashboard"] = json!(dash);
    }

    let created: Subscription = client
        .post(&format!("/api/environments/{env_id}/subscriptions/"), &body)
        .await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out {
            ok: bool,
            action: &'static str,
            id: i64,
            title: String,
        }
        output::print_json(&Out {
            ok: true,
            action: "create",
            id: created.id,
            title: created.title,
        });
    } else {
        println!(
            "Created subscription '{}' (id {})",
            created.title, created.id
        );
    }
    Ok(())
}

// ── update ────────────────────────────────────────────────────────────────────

async fn update_subscription(
    cx: &CommandContext,
    id: i64,
    title: Option<String>,
    frequency: Option<String>,
) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    let mut body = serde_json::Map::new();
    if let Some(t) = title {
        body.insert("title".into(), Value::String(t));
    }
    if let Some(f) = frequency {
        body.insert("frequency".into(), Value::String(f));
    }

    if body.is_empty() {
        return Err(BosshoggError::BadRequest(
            "no update flags provided (try --title, --frequency)".into(),
        ));
    }

    cx.confirm(&format!("update subscription `{id}`; continue?"))?;

    let updated: Subscription = client
        .patch(
            &format!("/api/environments/{env_id}/subscriptions/{id}/"),
            &Value::Object(body),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&updated);
    } else {
        println!(
            "Updated subscription '{}' (id {})",
            updated.title, updated.id
        );
    }
    Ok(())
}

// ── delete ────────────────────────────────────────────────────────────────────

async fn delete_subscription(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!("soft-delete subscription `{id}`; continue?"))?;

    client
        .delete(&format!("/api/environments/{env_id}/subscriptions/{id}/"))
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
        println!("Deleted subscription {id}");
    }
    Ok(())
}

// ── test-delivery ─────────────────────────────────────────────────────────────

async fn test_delivery(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;

    cx.confirm(&format!(
        "send a real test notification for subscription `{id}`; continue?"
    ))?;

    // NOTE: PostHog uses a dash in this endpoint path: test-delivery (not test_delivery)
    let v: Value = client
        .post(
            &format!("/api/environments/{env_id}/subscriptions/{id}/test-delivery/"),
            &json!({}),
        )
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else {
        println!("Sent test delivery for subscription {id}");
    }
    Ok(())
}

// ── deliveries ────────────────────────────────────────────────────────────────

async fn deliveries(cx: &CommandContext, id: i64) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!(
            "/api/environments/{env_id}/subscriptions/{id}/deliveries/"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        if let Some(results) = v.get("results").and_then(Value::as_array) {
            for entry in results {
                let ts = entry
                    .get("created_at")
                    .and_then(Value::as_str)
                    .unwrap_or("-");
                let status = entry.get("status").and_then(Value::as_str).unwrap_or("-");
                println!("{ts}  {status}");
            }
        } else {
            output::print_json(&v);
        }
    }
    Ok(())
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscription_roundtrip_minimal() {
        let raw = r#"{
            "id": 1,
            "title": "Weekly Digest",
            "target_type": "email",
            "target_value": "team@example.com",
            "frequency": "weekly",
            "deleted": false
        }"#;
        let s: Subscription = serde_json::from_str(raw).unwrap();
        assert_eq!(s.id, 1);
        assert_eq!(s.title, "Weekly Digest");
        assert_eq!(s.target_type, "email");
        assert!(!s.deleted);
    }

    #[test]
    fn subscription_roundtrip_full() {
        let raw = "{
            \"id\": 42,
            \"title\": \"Daily Slack Alert\",
            \"target_type\": \"slack\",
            \"target_value\": \"#alerts\",
            \"frequency\": \"daily\",
            \"interval\": 1,
            \"byweekday\": null,
            \"bysetpos\": null,
            \"start_date\": \"2026-01-01\",
            \"until_date\": null,
            \"insight\": 100,
            \"dashboard\": null,
            \"next_delivery_date\": \"2026-04-22T09:00:00Z\",
            \"deleted\": false,
            \"created_by\": {\"id\": 1, \"email\": \"admin@example.com\"},
            \"created_at\": \"2026-01-01T00:00:00Z\"
        }";
        let s: Subscription = serde_json::from_str(raw).unwrap();
        assert_eq!(s.id, 42);
        assert_eq!(s.insight, Some(100));
        assert_eq!(s.dashboard, None);
        assert_eq!(s.interval, Some(1));
    }

    #[test]
    fn subscription_deleted_field_required() {
        // deleted is not optional — missing it should fail
        let raw = r#"{
            "id": 1,
            "title": "Test",
            "target_type": "email",
            "target_value": "a@b.com",
            "frequency": "daily"
        }"#;
        // deleted has no serde(default) so it's required
        let result = serde_json::from_str::<Subscription>(raw);
        assert!(result.is_err(), "deleted field is required");
    }
}
