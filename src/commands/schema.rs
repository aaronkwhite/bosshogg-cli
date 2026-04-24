use std::collections::BTreeMap;

use chrono::Utc;
use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::{Value, json};

use crate::commands::context::CommandContext;
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Args, Debug)]
pub struct SchemaArgs {
    #[command(subcommand)]
    pub command: SchemaCommand,
}

#[derive(Subcommand, Debug)]
pub enum SchemaCommand {
    /// Dump the HogQL schema for the active project.
    Hogql,
}

#[derive(Serialize, Debug)]
pub struct ColumnInfo {
    pub name: String,
    pub r#type: String,
    pub schema_valid: bool,
}

#[derive(Serialize, Debug)]
pub struct TableInfo {
    pub name: String,
    pub kind: String,
    pub columns: Vec<ColumnInfo>,
}

#[derive(Serialize, Debug)]
pub struct SchemaOutput {
    pub tables: Vec<TableInfo>,
    pub warehouse_tables: Vec<TableInfo>,
    pub captured_at: String,
}

pub async fn execute(args: SchemaArgs, cx: &CommandContext) -> Result<()> {
    match args.command {
        SchemaCommand::Hogql => dump_hogql(cx).await,
    }
}

async fn dump_hogql(cx: &CommandContext) -> Result<()> {
    let client = &cx.client;
    let env_id = client.env_id().ok_or_else(|| {
        BosshoggError::Config("no env_id set — run `bosshogg configure` or pass --env".into())
    })?;

    let path = format!("/api/environments/{env_id}/query/");
    let body = json!({ "query": { "kind": "DatabaseSchemaQuery" } });
    let resp: Value = client.post(&path, &body).await?;

    // Response shape: { tables: { <table_name>: { type, id, name, fields: { <col>: {name, type, schema_valid, ...} }, ... } }, joins: [...] }
    let tables_obj = resp
        .get("tables")
        .and_then(Value::as_object)
        .ok_or_else(|| BosshoggError::Config("DatabaseSchemaQuery response missing 'tables'".into()))?;

    let mut tables: Vec<TableInfo> = Vec::new();
    let mut warehouse_tables: Vec<TableInfo> = Vec::new();

    // BTreeMap iteration is alphabetical — keeps output stable.
    let sorted: BTreeMap<&String, &Value> = tables_obj.iter().collect();
    for (name, t) in sorted {
        let kind = t
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();

        // Keep the surface narrow: only emit tables a user would usefully
        // inspect. Views, managed_views, materialized_views, batch_exports,
        // endpoints, and system tables are noise in schema dumps.
        let bucket = match kind.as_str() {
            "posthog" => &mut tables,
            "data_warehouse" => &mut warehouse_tables,
            _ => continue,
        };

        let mut columns: Vec<ColumnInfo> = t
            .get("fields")
            .and_then(Value::as_object)
            .map(|fields| {
                fields
                    .iter()
                    .map(|(col_name, f)| ColumnInfo {
                        name: f
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or(col_name)
                            .to_string(),
                        r#type: f
                            .get("type")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        schema_valid: f
                            .get("schema_valid")
                            .and_then(Value::as_bool)
                            .unwrap_or(true),
                    })
                    .collect()
            })
            .unwrap_or_default();
        columns.sort_by(|a, b| a.name.cmp(&b.name));

        bucket.push(TableInfo {
            name: name.clone(),
            kind,
            columns,
        });
    }

    let out = SchemaOutput {
        tables,
        warehouse_tables,
        captured_at: Utc::now().to_rfc3339(),
    };

    if cx.json_mode {
        output::print_json(&out);
    } else {
        for t in &out.tables {
            println!("{} ({})", t.name, t.kind);
            for c in &t.columns {
                println!("  {:<40} {}", c.name, c.r#type);
            }
            println!();
        }
        if !out.warehouse_tables.is_empty() {
            println!("# warehouse tables");
            for t in &out.warehouse_tables {
                println!("{} ({})", t.name, t.kind);
                for c in &t.columns {
                    println!("  {:<40} {}", c.name, c.r#type);
                }
                println!();
            }
        }
        println!("captured_at: {}", out.captured_at);
    }
    Ok(())
}
