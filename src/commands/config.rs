use clap::{Args, Subcommand};
use serde::Serialize;

use crate::config::{self, Context};
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Upsert a context.
    SetContext(SetContextArgs),
    /// List all contexts.
    GetContexts,
    /// Print the current context name.
    CurrentContext,
    /// Switch the current context.
    UseContext { name: String },
    /// Delete a context.
    DeleteContext { name: String },
    /// Manage anonymous self-tracking telemetry.
    Analytics(AnalyticsArgs),
}

#[derive(Args, Debug)]
pub struct AnalyticsArgs {
    #[command(subcommand)]
    pub command: AnalyticsCommand,
}

#[derive(Subcommand, Debug)]
pub enum AnalyticsCommand {
    /// Enable anonymous usage stats (the default).
    On,
    /// Disable anonymous usage stats.
    Off,
    /// Show whether telemetry is currently enabled.
    Status,
}

#[derive(Args, Debug)]
pub struct SetContextArgs {
    pub name: String,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub region: Option<String>,
    #[arg(long)]
    pub project: Option<String>,
    #[arg(long)]
    pub env: Option<String>,
    #[arg(long)]
    pub org: Option<String>,
    /// Read key from named env var.
    #[arg(long, conflicts_with = "key_from_stdin")]
    pub key_from_env: Option<String>,
    /// Read key from stdin (one line).
    #[arg(long, conflicts_with = "key_from_env")]
    pub key_from_stdin: bool,
    /// Project public token (phc_) used by `bosshogg capture` and `bosshogg flag evaluate`.
    #[arg(long = "project-token")]
    pub project_token: Option<String>,
}

#[derive(Serialize)]
struct ContextListItem<'a> {
    name: &'a str,
    host: &'a str,
    project_id: Option<&'a str>,
    env_id: Option<&'a str>,
    has_key: bool,
}

#[derive(Serialize)]
struct ContextListOutput<'a> {
    current: Option<&'a str>,
    contexts: Vec<ContextListItem<'a>>,
}

pub async fn run(args: ConfigArgs, json_mode: bool) -> Result<()> {
    match args.command {
        ConfigCommand::SetContext(a) => set_context(a, json_mode).await,
        ConfigCommand::GetContexts => get_contexts(json_mode),
        ConfigCommand::CurrentContext => current_context(json_mode),
        ConfigCommand::UseContext { name } => use_context(name, json_mode),
        ConfigCommand::DeleteContext { name } => delete_context(name, json_mode),
        ConfigCommand::Analytics(a) => analytics(a, json_mode),
    }
}

fn analytics(args: AnalyticsArgs, json_mode: bool) -> Result<()> {
    match args.command {
        AnalyticsCommand::On => {
            config::set_analytics_enabled(Some(true))?;
            print_analytics_status(true, "set", json_mode);
        }
        AnalyticsCommand::Off => {
            config::set_analytics_enabled(Some(false))?;
            print_analytics_status(false, "set", json_mode);
        }
        AnalyticsCommand::Status => {
            let enabled = config::is_analytics_enabled();
            print_analytics_status(enabled, "status", json_mode);
        }
    }
    Ok(())
}

fn print_analytics_status(enabled: bool, action: &str, json_mode: bool) {
    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            action: &'a str,
            analytics_enabled: bool,
        }
        output::print_json(&Out {
            ok: true,
            action,
            analytics_enabled: enabled,
        });
    } else if enabled {
        println!("analytics: enabled");
    } else {
        println!("analytics: disabled");
    }
}

fn get_contexts(json_mode: bool) -> Result<()> {
    let cfg = config::load()?;
    let mut items: Vec<ContextListItem> = cfg
        .contexts
        .iter()
        .map(|(name, c)| ContextListItem {
            name,
            host: &c.host,
            project_id: c.project_id.as_deref(),
            env_id: c.env_id.as_deref(),
            has_key: c.api_key.is_some(),
        })
        .collect();
    items.sort_by(|a, b| a.name.cmp(b.name));

    if json_mode {
        output::print_json(&ContextListOutput {
            current: cfg.current_context.as_deref(),
            contexts: items,
        });
    } else {
        let headers = &["CURRENT", "NAME", "HOST", "PROJECT", "ENV", "KEY"];
        let rows: Vec<Vec<String>> = items
            .iter()
            .map(|i| {
                let marker = if cfg.current_context.as_deref() == Some(i.name) {
                    "*".to_string()
                } else {
                    "".to_string()
                };
                vec![
                    marker,
                    i.name.to_string(),
                    i.host.to_string(),
                    i.project_id.unwrap_or("-").to_string(),
                    i.env_id.unwrap_or("-").to_string(),
                    if i.has_key { "yes".into() } else { "no".into() },
                ]
            })
            .collect();
        output::table::print(headers, &rows);
    }
    Ok(())
}

fn current_context(json_mode: bool) -> Result<()> {
    let cfg = config::load()?;
    let current = cfg
        .current_context
        .clone()
        .ok_or_else(|| BosshoggError::Config("no current context set".into()))?;
    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            current: &'a str,
        }
        output::print_json(&Out { current: &current });
    } else {
        println!("{current}");
    }
    Ok(())
}

fn use_context(name: String, json_mode: bool) -> Result<()> {
    let mut cfg = config::load()?;
    if !cfg.contexts.contains_key(&name) {
        return Err(BosshoggError::NotFound(format!("context '{name}'")));
    }
    cfg.current_context = Some(name.clone());
    config::save(&cfg)?;
    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            current: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            current: &name,
        });
    } else {
        println!("Switched to context '{name}'");
    }
    Ok(())
}

fn delete_context(name: String, json_mode: bool) -> Result<()> {
    let mut cfg = config::load()?;
    if cfg.contexts.remove(&name).is_none() {
        return Err(BosshoggError::NotFound(format!("context '{name}'")));
    }
    if cfg.current_context.as_deref() == Some(&name) {
        cfg.current_context = None;
    }
    config::save(&cfg)?;
    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            deleted: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            deleted: &name,
        });
    } else {
        println!("Deleted context '{name}'");
    }
    Ok(())
}

async fn set_context(args: SetContextArgs, json_mode: bool) -> Result<()> {
    let mut cfg = config::load().unwrap_or_default();

    let api_key = if let Some(var) = args.key_from_env.as_deref() {
        Some(
            std::env::var(var)
                .map_err(|_| BosshoggError::Config(format!("env var '{var}' not set")))?,
        )
    } else if args.key_from_stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_line(&mut buf)
            .map_err(|e| BosshoggError::Config(format!("stdin: {e}")))?;
        Some(buf.trim().to_string())
    } else {
        None
    };

    let host = args
        .host
        .clone()
        .or_else(|| region_to_host(args.region.as_deref()))
        .unwrap_or_else(|| "https://us.posthog.com".to_string());

    let existing = cfg.contexts.get(&args.name).cloned();
    let ctx = Context {
        host,
        region: args
            .region
            .or_else(|| existing.as_ref().and_then(|e| e.region.clone())),
        api_key: api_key.or_else(|| existing.as_ref().and_then(|e| e.api_key.clone())),
        project_token: args
            .project_token
            .or_else(|| existing.as_ref().and_then(|e| e.project_token.clone())),
        project_id: args
            .project
            .or_else(|| existing.as_ref().and_then(|e| e.project_id.clone())),
        env_id: args
            .env
            .or_else(|| existing.as_ref().and_then(|e| e.env_id.clone())),
        org_id: args
            .org
            .or_else(|| existing.as_ref().and_then(|e| e.org_id.clone())),
    };

    let name = args.name.clone();
    cfg.contexts.insert(name.clone(), ctx);
    if cfg.current_context.is_none() {
        cfg.current_context = Some(name.clone());
    }
    config::save(&cfg)?;

    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            action: &'a str,
            name: &'a str,
        }
        output::print_json(&Out {
            ok: true,
            action: "set-context",
            name: &name,
        });
    } else {
        println!("Wrote context '{name}'");
    }
    Ok(())
}

fn region_to_host(region: Option<&str>) -> Option<String> {
    match region {
        Some("us") => Some("https://us.posthog.com".into()),
        Some("eu") => Some("https://eu.posthog.com".into()),
        _ => None,
    }
}
