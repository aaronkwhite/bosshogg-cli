// src/commands/event.rs
//! `bosshogg event` — list (HogQL), get, values, tail.
//!
//! POST /events/ is deprecated. All listing goes through HogQL via
//! `Client::query`. `get` and `values` use the legacy REST endpoints which
//! still work.

use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::Value;

use crate::client::QueryKind;
use crate::commands::context::CommandContext;
use crate::commands::util::env_id_required;
use crate::error::Result;
use crate::output;

// ── Clap tree ────────────────────────────────────────────────────────────────

#[derive(Args, Debug)]
pub struct EventArgs {
    #[command(subcommand)]
    pub command: EventCommand,
}

#[derive(Subcommand, Debug)]
pub enum EventCommand {
    /// List events via HogQL query.
    List {
        /// Filter by event name.
        #[arg(long)]
        event: Option<String>,
        /// Filter by distinct_id.
        #[arg(long)]
        distinct_id: Option<String>,
        /// Filter by person UUID.
        #[arg(long)]
        person_id: Option<String>,
        /// Only events before this ISO timestamp.
        #[arg(long)]
        before: Option<String>,
        /// Only events after this ISO timestamp.
        #[arg(long)]
        after: Option<String>,
        /// Extra WHERE clause fragment (raw HogQL).
        #[arg(long)]
        properties: Option<String>,
        /// Max rows to return (default 50).
        #[arg(long)]
        limit: Option<u64>,
    },
    /// Get a single event by UUID (legacy REST endpoint).
    Get { uuid: String },
    /// List distinct values for an event property.
    Values {
        /// Property name to enumerate.
        #[arg(long)]
        prop: String,
        /// Optional event name to scope the property.
        #[arg(long)]
        event: Option<String>,
    },
    /// Poll for new events in a loop (Ctrl-C to stop).
    Tail {
        /// Filter by event name.
        #[arg(long)]
        event: Option<String>,
        /// Rows per poll iteration (default 20).
        #[arg(long)]
        limit: Option<u64>,
    },
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

pub async fn execute(args: EventArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        EventCommand::List {
            event,
            distinct_id,
            person_id,
            before,
            after,
            properties,
            limit,
        } => {
            list_events(
                cx,
                ListFlags {
                    event,
                    distinct_id,
                    person_id,
                    before,
                    after,
                    properties,
                    limit,
                },
            )
            .await
        }
        EventCommand::Get { uuid } => get_event(cx, &uuid).await,
        EventCommand::Values { prop, event } => values(cx, &prop, event.as_deref()).await,
        EventCommand::Tail { event, limit } => tail_events(cx, event, limit).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

struct ListFlags {
    event: Option<String>,
    distinct_id: Option<String>,
    person_id: Option<String>,
    before: Option<String>,
    after: Option<String>,
    properties: Option<String>,
    limit: Option<u64>,
}

fn build_hogql(flags: &ListFlags) -> String {
    let mut where_clauses: Vec<String> = Vec::new();

    if let Some(name) = &flags.event {
        where_clauses.push(format!("event = '{}'", name.replace('\'', "''")));
    }
    if let Some(did) = &flags.distinct_id {
        where_clauses.push(format!("distinct_id = '{}'", did.replace('\'', "''")));
    }
    if let Some(pid) = &flags.person_id {
        where_clauses.push(format!("person_id = '{}'", pid.replace('\'', "''")));
    }
    if let Some(ts) = &flags.after {
        where_clauses.push(format!("timestamp >= '{}'", ts.replace('\'', "''")));
    }
    if let Some(ts) = &flags.before {
        where_clauses.push(format!("timestamp <= '{}'", ts.replace('\'', "''")));
    }
    if let Some(extra) = &flags.properties {
        where_clauses.push(extra.clone());
    }

    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_clauses.join(" AND "))
    };

    let limit = flags.limit.unwrap_or(50);
    format!(
        "SELECT event, distinct_id, timestamp, properties FROM events{where_sql} ORDER BY timestamp DESC LIMIT {limit}"
    )
}

// ── list ──────────────────────────────────────────────────────────────────────

async fn list_events(cx: &CommandContext, flags: ListFlags) -> Result<()> {
    let client = &cx.client;
    let sql = build_hogql(&flags);
    let resp = client.query(&sql, QueryKind::HogQL, false).await?;

    if cx.json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            columns: &'a [String],
            types: &'a [Value],
            results: &'a [Vec<Value>],
            #[serde(skip_serializing_if = "Option::is_none")]
            hogql: Option<&'a String>,
        }
        output::print_json(&Out {
            columns: &resp.columns,
            types: &resp.types,
            results: &resp.results,
            hogql: resp.hogql.as_ref(),
        });
    } else {
        let headers: Vec<&str> = resp.columns.iter().map(String::as_str).collect();
        let rows: Vec<Vec<String>> = resp
            .results
            .iter()
            .map(|r| r.iter().map(render_cell).collect())
            .collect();
        output::table::print(&headers, &rows);
    }
    Ok(())
}

// ── get ───────────────────────────────────────────────────────────────────────

async fn get_event(cx: &CommandContext, uuid: &str) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let v: Value = client
        .get(&format!("/api/environments/{env_id}/events/{uuid}/"))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        let event = v.get("event").and_then(Value::as_str).unwrap_or("-");
        let did = v.get("distinct_id").and_then(Value::as_str).unwrap_or("-");
        let ts = v.get("timestamp").and_then(Value::as_str).unwrap_or("-");
        println!("Event:       {event}");
        println!("Distinct ID: {did}");
        println!("Timestamp:   {ts}");
        if let Some(props) = v.get("properties") {
            println!("Properties:  {props}");
        }
    }
    Ok(())
}

// ── values ────────────────────────────────────────────────────────────────────

async fn values(cx: &CommandContext, prop: &str, event: Option<&str>) -> Result<()> {
    let client = &cx.client;
    let env_id = env_id_required(client)?;
    let mut qs = format!("?key={}", urlencoding::encode(prop));
    if let Some(e) = event {
        qs.push_str(&format!("&event_name={}", urlencoding::encode(e)));
    }
    let v: Value = client
        .get(&format!("/api/environments/{env_id}/events/values/{qs}"))
        .await?;

    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(arr) = v.as_array() {
        for item in arr {
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_else(|| item.as_str().unwrap_or("-"));
            println!("{name}");
        }
    } else {
        output::print_json(&v);
    }
    Ok(())
}

// ── tail ──────────────────────────────────────────────────────────────────────

async fn tail_events(cx: &CommandContext, event: Option<String>, limit: Option<u64>) -> Result<()> {
    let client = &cx.client;
    let flags = ListFlags {
        event,
        distinct_id: None,
        person_id: None,
        before: None,
        after: None,
        properties: None,
        limit: Some(limit.unwrap_or(20)),
    };

    loop {
        let sql = build_hogql(&flags);
        let resp = client.query(&sql, QueryKind::HogQL, false).await?;

        if cx.json_mode {
            #[derive(Serialize)]
            struct Out<'a> {
                columns: &'a [String],
                results: &'a [Vec<Value>],
            }
            output::print_json(&Out {
                columns: &resp.columns,
                results: &resp.results,
            });
        } else {
            let headers: Vec<&str> = resp.columns.iter().map(String::as_str).collect();
            let rows: Vec<Vec<String>> = resp
                .results
                .iter()
                .map(|r| r.iter().map(render_cell).collect())
                .collect();
            output::table::print(&headers, &rows);
        }

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

fn render_cell(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

// ── unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn flags(
        event: Option<&str>,
        distinct_id: Option<&str>,
        person_id: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
        limit: Option<u64>,
    ) -> ListFlags {
        ListFlags {
            event: event.map(str::to_owned),
            distinct_id: distinct_id.map(str::to_owned),
            person_id: person_id.map(str::to_owned),
            after: after.map(str::to_owned),
            before: before.map(str::to_owned),
            properties: None,
            limit,
        }
    }

    #[test]
    fn hogql_no_filters() {
        let sql = build_hogql(&flags(None, None, None, None, None, None));
        assert!(sql.contains("FROM events"), "missing FROM clause");
        assert!(!sql.contains("WHERE"), "unexpected WHERE");
        assert!(sql.contains("LIMIT 50"), "wrong default limit");
    }

    #[test]
    fn hogql_with_event_filter() {
        let sql = build_hogql(&flags(Some("pageview"), None, None, None, None, Some(10)));
        assert!(sql.contains("event = 'pageview'"), "event filter missing");
        assert!(sql.contains("WHERE"), "WHERE missing");
        assert!(sql.contains("LIMIT 10"), "custom limit missing");
    }

    #[test]
    fn hogql_with_all_filters() {
        let sql = build_hogql(&flags(
            Some("$pageview"),
            Some("user@example.com"),
            Some("uuid-123"),
            Some("2026-01-01T00:00:00Z"),
            Some("2026-12-31T23:59:59Z"),
            Some(100),
        ));
        assert!(sql.contains("event = '$pageview'"));
        assert!(sql.contains("distinct_id = 'user@example.com'"));
        assert!(sql.contains("person_id = 'uuid-123'"));
        assert!(sql.contains("timestamp >= '2026-01-01T00:00:00Z'"));
        assert!(sql.contains("timestamp <= '2026-12-31T23:59:59Z'"));
        assert!(sql.contains("LIMIT 100"));
    }

    #[test]
    fn hogql_single_quote_escaped() {
        let sql = build_hogql(&flags(Some("it's an event"), None, None, None, None, None));
        assert!(
            sql.contains("event = 'it''s an event'"),
            "escaping broken: {sql}"
        );
    }

    #[test]
    fn hogql_order_is_desc() {
        let sql = build_hogql(&flags(None, None, None, None, None, None));
        assert!(sql.contains("ORDER BY timestamp DESC"), "wrong order");
    }
}
