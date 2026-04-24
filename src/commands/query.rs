//! Query command — HogQL and Query API wrappers.
//!
//! Supports:
//! - `query run` — inline SQL or `--file` input
//! - `query hogql` — alias for `run --kind hogql`
//! - `query events` / `trends` / `funnel` — QueryKind wrappers
//! - `query status` — check async query status
//! - `query cancel` — cancel async query
//! - `query log` — fetch 24h execution log
//! - `query draft-sql` — server-side SQL draft helper

use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::path::PathBuf;

use crate::client::QueryKind;
use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Args, Debug)]
pub struct QueryArgs {
    #[command(subcommand)]
    pub command: QueryCommand,
}

#[derive(Subcommand, Debug)]
pub enum QueryCommand {
    /// Run a HogQL query (inline SQL, --file, or stdin).
    Run(RunArgs),
    /// Alias for `run --kind hogql`.
    Hogql(RunArgs),
    /// Run an EventsQuery. Input is a JSON body (inline, --file, or stdin);
    /// bosshogg injects `"kind": "EventsQuery"` if omitted. Example:
    /// `query events '{"select": ["event", "timestamp"], "limit": 5}'`.
    Events(RunArgs),
    /// Run a TrendsQuery. Input is a JSON body.
    Trends(RunArgs),
    /// Run a FunnelsQuery. Input is a JSON body.
    Funnel(RunArgs),
    /// Check async query status.
    Status { id: String },
    /// Cancel an async query.
    Cancel { id: String },
    /// Fetch the 24h execution log for a query.
    Log { id: String },
    /// Server-side SQL draft helper.
    DraftSql {
        #[arg(long)]
        prompt: String,
    },
}

#[derive(Args, Debug)]
pub struct RunArgs {
    /// Inline SQL. Omit to read from --file or stdin.
    pub sql: Option<String>,
    /// Path to a SQL file. '-' reads stdin.
    #[arg(long)]
    pub file: Option<PathBuf>,
    /// Run asynchronously and poll.
    #[arg(long, alias = "async_")]
    pub r#async: bool,
    /// Poll interval cap for --async (seconds).
    #[arg(long, default_value = "10")]
    pub poll_max_s: u64,
    /// Do not inject an auto LIMIT.
    #[arg(long)]
    pub no_limit: bool,
    /// Explicit query kind. Defaults to HogQL.
    #[arg(long, value_enum)]
    pub kind: Option<Kind>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Kind {
    Hogql,
    Events,
    Trends,
    Funnel,
}

impl From<Kind> for QueryKind {
    fn from(k: Kind) -> Self {
        match k {
            Kind::Hogql => QueryKind::HogQL,
            Kind::Events => QueryKind::Events,
            Kind::Trends => QueryKind::Trends,
            Kind::Funnel => QueryKind::Funnel,
        }
    }
}

pub async fn execute(args: &QueryArgs, cx: &CommandContext) -> Result<()> {
    match &args.command {
        QueryCommand::Run(a) => run_query(cx, a, QueryKind::HogQL).await,
        QueryCommand::Hogql(a) => run_query(cx, a, QueryKind::HogQL).await,
        QueryCommand::Events(a) => run_query(cx, a, QueryKind::Events).await,
        QueryCommand::Trends(a) => run_query(cx, a, QueryKind::Trends).await,
        QueryCommand::Funnel(a) => run_query(cx, a, QueryKind::Funnel).await,
        QueryCommand::Status { id } => status(cx, id.clone()).await,
        QueryCommand::Cancel { id } => cancel(cx, id.clone()).await,
        QueryCommand::Log { id } => log_cmd(cx, id.clone()).await,
        QueryCommand::DraftSql { prompt } => draft_sql(cx, prompt.clone()).await,
    }
}

async fn run_query(cx: &CommandContext, args: &RunArgs, default_kind: QueryKind) -> Result<()> {
    let client = &cx.client;
    let kind = args.kind.map(QueryKind::from).unwrap_or(default_kind);
    let input = resolve_sql(args).await?;

    match kind {
        QueryKind::HogQL => {
            // HogQL: input is SQL; the Client wraps + auto-LIMITs and parses
            // the tabular response shape.
            let sql = if args.no_limit {
                input
            } else {
                output::safe::inject_hogql_limit(&input)
            };
            let resp = client.query(&sql, kind, args.r#async).await?;
            render_tabular(cx, &resp);
        }
        QueryKind::Events | QueryKind::Trends | QueryKind::Funnel => {
            // Structured kinds: input is a JSON body. Inject `kind` if missing.
            let mut body: Value = serde_json::from_str(input.trim()).map_err(|e| {
                BosshoggError::BadRequest(format!(
                    "{} expects a JSON body (got: {e}) — example: `query events '{{\"select\":[\"event\"],\"limit\":5}}'`",
                    kind.display_name()
                ))
            })?;
            if let Value::Object(ref mut map) = body {
                map.entry("kind".to_string())
                    .or_insert_with(|| Value::String(kind.as_str().to_string()));
            } else {
                return Err(BosshoggError::BadRequest(
                    "query body must be a JSON object".into(),
                ));
            }
            let raw = client.query_body(body, args.r#async).await?;
            // EventsQuery returns tabular shape; Trends/Funnel return structured
            // outputs that we dump as JSON rather than forcing into rows/cols.
            if matches!(kind, QueryKind::Events)
                && let Ok(tabular) =
                    serde_json::from_value::<crate::client::QueryResponse>(raw.clone())
            {
                render_tabular(cx, &tabular);
            } else {
                output::print_json(&raw);
            }
        }
    }
    Ok(())
}

fn render_tabular(cx: &CommandContext, resp: &crate::client::QueryResponse) {
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
}

fn render_cell(v: &Value) -> String {
    match v {
        Value::Null => "".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

async fn resolve_sql(args: &RunArgs) -> Result<String> {
    if let Some(sql) = args.sql.as_deref() {
        return Ok(sql.to_string());
    }
    if let Some(path) = args.file.as_deref() {
        if path == std::path::Path::new("-") {
            let mut buf = String::new();
            std::io::stdin()
                .read_to_string(&mut buf)
                .map_err(|e| BosshoggError::Config(format!("stdin: {e}")))?;
            return Ok(buf);
        }
        return tokio::fs::read_to_string(path)
            .await
            .map_err(|e| BosshoggError::Config(format!("read {}: {e}", path.display())));
    }
    Err(BosshoggError::BadRequest(
        "provide SQL inline, via --file, or pipe via --file -".into(),
    ))
}

async fn status(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env = client
        .env_id()
        .ok_or_else(|| BosshoggError::Config("no env_id".into()))?;
    let v: Value = client
        .get(&format!("/api/environments/{env}/query/{id}/"))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else {
        let qs = v.pointer("/query_status").unwrap_or(&v);
        println!("{}", qs);
    }
    Ok(())
}

async fn cancel(cx: &CommandContext, id: String) -> Result<()> {
    // Cancelling your own async query is not destructive to stored data; no --yes gate by design.
    let client = &cx.client;
    let env = client
        .env_id()
        .ok_or_else(|| BosshoggError::Config("no env_id".into()))?;
    client
        .delete(&format!("/api/environments/{env}/query/{id}/"))
        .await?;
    if cx.json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            action: &'a str,
            id: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            action: "cancel",
            id: &id,
        });
    } else {
        println!("Cancelled query {id}");
    }
    Ok(())
}

async fn log_cmd(cx: &CommandContext, id: String) -> Result<()> {
    let client = &cx.client;
    let env = client
        .env_id()
        .ok_or_else(|| BosshoggError::Config("no env_id".into()))?;
    let v: Value = client
        .get(&format!("/api/environments/{env}/query/{id}/log/"))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(results) = v.get("results").and_then(Value::as_array) {
        for line in results {
            let ts = line.get("timestamp").and_then(Value::as_str).unwrap_or("-");
            let msg = line.get("line").and_then(Value::as_str).unwrap_or("-");
            println!("{ts}  {msg}");
        }
    }
    Ok(())
}

async fn draft_sql(cx: &CommandContext, prompt: String) -> Result<()> {
    let client = &cx.client;
    let env = client
        .env_id()
        .ok_or_else(|| BosshoggError::Config("no env_id".into()))?;
    let encoded = urlencoding::encode(&prompt);
    let v: Value = client
        .get(&format!(
            "/api/environments/{env}/query/draft_sql/?prompt={encoded}"
        ))
        .await?;
    if cx.json_mode {
        output::print_json(&v);
    } else if let Some(sql) = v.get("sql").and_then(Value::as_str) {
        println!("{sql}");
    }
    Ok(())
}
