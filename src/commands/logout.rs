//! `bosshogg logout` — remove saved credentials from local config.
//!
//! Local-only operation. Does NOT revoke the personal API key on PostHog —
//! that requires visiting account settings. The help text says so explicitly.
//!
//! Default behavior: deletes the current context. `--context <name>` targets
//! a specific context. `--all` wipes every context and clears current.

use clap::Args;
use serde::Serialize;

use crate::config;
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Args, Debug)]
pub struct LogoutArgs {
    /// Remove a specific context by name (default: current context).
    #[arg(long, conflicts_with = "all")]
    pub context: Option<String>,

    /// Remove every saved context and clear the current selection.
    #[arg(long, conflicts_with = "context")]
    pub all: bool,
}

#[derive(Serialize)]
struct LogoutOutput<'a> {
    ok: bool,
    deleted: Vec<&'a str>,
    note: &'static str,
}

const REVOKE_NOTE: &str =
    "Local credentials removed. To revoke the API key on PostHog, visit account settings.";

pub fn execute(args: LogoutArgs, json_mode: bool) -> Result<()> {
    let mut cfg = config::load().unwrap_or_default();

    let to_delete: Vec<String> = if args.all {
        cfg.contexts.keys().cloned().collect()
    } else if let Some(name) = args.context {
        if !cfg.contexts.contains_key(&name) {
            return Err(BosshoggError::NotFound(format!("context '{name}'")));
        }
        vec![name]
    } else {
        match cfg.current_context.clone() {
            Some(name) if cfg.contexts.contains_key(&name) => vec![name],
            Some(name) => {
                return Err(BosshoggError::NotFound(format!(
                    "current context '{name}' has no saved credentials"
                )));
            }
            None => {
                return Err(BosshoggError::Config(
                    "no current context to log out of. \
                     Use --context <name> to target one, or --all to remove every context."
                        .into(),
                ));
            }
        }
    };

    for name in &to_delete {
        cfg.contexts.remove(name);
    }
    if args.all
        || cfg
            .current_context
            .as_deref()
            .map(|c| to_delete.iter().any(|d| d == c))
            .unwrap_or(false)
    {
        cfg.current_context = None;
    }
    config::save(&cfg)?;

    if json_mode {
        output::print_json(&LogoutOutput {
            ok: true,
            deleted: to_delete.iter().map(String::as_str).collect(),
            note: REVOKE_NOTE,
        });
    } else if to_delete.is_empty() {
        println!("No contexts to remove.");
        println!("{REVOKE_NOTE}");
    } else {
        for name in &to_delete {
            println!("Logged out of context '{name}'.");
        }
        println!("{REVOKE_NOTE}");
    }

    Ok(())
}
