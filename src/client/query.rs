//! HogQL + Query API wrapper.
//!
//! - Endpoint: `POST /api/environments/:env_id/query/`
//! - Rate-limited at 2400/hr (docs/api-notes.md § HogQL).
//! - Auto-injects `LIMIT 100` via `output::safe::inject_hogql_limit`.
//! - `is_async = true` uses `query_status.id` and polls `GET /query/:id/`
//!   with exponential backoff (500ms → 10s cap).
//! - `HogQLQuery` is the default kind; `Events`/`Trends`/`Funnel` map to
//!   the corresponding PostHog query kinds.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::client::Client;
use crate::error::{BosshoggError, Result};
use crate::output::safe::inject_hogql_limit;

#[derive(Debug, Clone, Copy)]
pub enum QueryKind {
    HogQL,
    Events,
    Trends,
    Funnel,
}

impl QueryKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            QueryKind::HogQL => "HogQLQuery",
            QueryKind::Events => "EventsQuery",
            QueryKind::Trends => "TrendsQuery",
            QueryKind::Funnel => "FunnelsQuery",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            QueryKind::HogQL => "HogQL",
            QueryKind::Events => "events",
            QueryKind::Trends => "trends",
            QueryKind::Funnel => "funnel",
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct QueryResponse {
    #[serde(default)]
    pub results: Vec<Vec<Value>>,
    #[serde(default)]
    pub columns: Vec<String>,
    // PostHog returns per-column [name, ch_type] pairs; older servers returned
    // bare type strings. Accept either by keeping the element opaque.
    #[serde(default)]
    pub types: Vec<Value>,
    #[serde(default)]
    pub hogql: Option<String>,
    #[serde(default)]
    pub timings: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct AsyncEnqueueEnvelope {
    query_status: QueryStatus,
}

#[derive(Debug, Deserialize)]
struct PollEnvelope {
    query_status: QueryStatus,
}

#[derive(Debug, Deserialize)]
struct QueryStatus {
    id: String,
    #[serde(default)]
    complete: bool,
    #[serde(default, rename = "error")]
    err: Option<String>,
    #[serde(default)]
    results: Vec<Vec<Value>>,
    #[serde(default)]
    columns: Vec<String>,
    #[serde(default)]
    types: Vec<Value>,
    #[serde(default)]
    hogql: Option<String>,
    #[serde(default)]
    timings: Option<Value>,
}

impl Client {
    /// Run a HogQL query. Builds `{kind: "HogQLQuery", query: <sql>}` and
    /// parses the response into the tabular `QueryResponse` shape. Appends
    /// `LIMIT 100` to the SQL if no LIMIT clause is present.
    pub async fn query(&self, sql: &str, kind: QueryKind, is_async: bool) -> Result<QueryResponse> {
        let safe_sql = inject_hogql_limit(sql);
        let body = json!({ "kind": kind.as_str(), "query": safe_sql });
        let raw = self.query_body(body, is_async).await?;
        Ok(serde_json::from_value(raw)?)
    }

    /// POST an arbitrary query body (the inner object — must include its own
    /// `kind` field) to `/api/environments/:env/query/` and return the raw
    /// JSON response. Handles both sync and async (poll-until-complete) paths.
    ///
    /// Returns `Value` because different query kinds have different response
    /// shapes: HogQL/Events return `{columns, types, results}`, but Trends
    /// returns `{series, ...}`, Funnel returns `{funnel, steps}`, etc.
    pub async fn query_body(&self, query: Value, is_async: bool) -> Result<Value> {
        let env_id = self.env_id().ok_or_else(|| {
            BosshoggError::Config("no env_id set — run `bosshogg configure` or pass --env".into())
        })?;

        let path = format!("/api/environments/{env_id}/query/");
        let body = json!({
            "query": query,
            "async": is_async,
        });

        if !is_async {
            return self.post(&path, &body).await;
        }

        // Async path: enqueue, then poll. Only the poll envelope shape is
        // structurally constrained — the inner results are still opaque.
        let enqueue: AsyncEnqueueEnvelope = self.post(&path, &body).await?;
        let id = enqueue.query_status.id;
        let poll_path = format!("/api/environments/{env_id}/query/{id}/");

        let mut delay_ms: u64 = 500;
        let cap_ms: u64 = 10_000;
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(60);

        loop {
            if start.elapsed() >= timeout {
                let _ = self.delete(&poll_path).await;
                return Err(BosshoggError::ServerError {
                    status: 504,
                    message: format!("query {id} timed out"),
                });
            }

            let poll: PollEnvelope = self.get(&poll_path).await?;
            let qs = poll.query_status;

            if let Some(e) = qs.err {
                return Err(BosshoggError::HogQL(e));
            }

            if qs.complete {
                return Ok(json!({
                    "results": qs.results,
                    "columns": qs.columns,
                    "types": qs.types,
                    "hogql": qs.hogql,
                    "timings": qs.timings,
                }));
            }

            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            delay_ms = (delay_ms * 2).min(cap_ms);
        }
    }
}
