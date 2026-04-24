use clap::Args;
use dialoguer::{Input, Password, Select, theme::ColorfulTheme};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

use crate::client::{Client, ResolvedAuth};
use crate::config::{self, Config, Context};
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Args, Debug)]
pub struct ConfigureArgs {
    /// Non-interactive: fail instead of prompting (useful for CI probe).
    #[arg(long)]
    pub non_interactive: bool,
}

#[derive(Debug, PartialEq)]
pub struct WizardInput {
    pub name: String,
    pub region: Region,
    pub host: String,
    pub api_key: String,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Region {
    Us,
    Eu,
    SelfHosted,
}

impl Region {
    pub fn default_host(self) -> &'static str {
        match self {
            Region::Us => "https://us.posthog.com",
            Region::Eu => "https://eu.posthog.com",
            Region::SelfHosted => "",
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Region::Us => "us",
            Region::Eu => "eu",
            Region::SelfHosted => "self-hosted",
        }
    }
}

pub fn validate_host(host: &str) -> Result<()> {
    if !host.starts_with("https://") {
        return Err(BosshoggError::Config(format!(
            "host must start with https:// (got '{host}')"
        )));
    }
    if host.ends_with('/') {
        return Err(BosshoggError::Config("host must not end with '/'".into()));
    }
    Ok(())
}

pub fn validate_key(key: &str) -> Result<()> {
    if !key.starts_with("phx_") {
        return Err(BosshoggError::Config(
            "personal API key must start with 'phx_'".into(),
        ));
    }
    if key.len() < 20 {
        return Err(BosshoggError::Config("API key looks too short".into()));
    }
    Ok(())
}

pub async fn execute(args: ConfigureArgs, json_mode: bool, debug: bool) -> Result<()> {
    if args.non_interactive || !output::is_interactive() {
        return Err(BosshoggError::Config(
            "configure requires a TTY; use 'bosshogg config set-context' for scripts".into(),
        ));
    }

    let theme = ColorfulTheme::default();

    let name: String = Input::with_theme(&theme)
        .with_prompt("Context name")
        .default("default".into())
        .interact_text()
        .map_err(|e| BosshoggError::Config(format!("prompt: {e}")))?;

    let regions = &["us", "eu", "self-hosted"];
    let region_idx = Select::with_theme(&theme)
        .with_prompt("Region")
        .default(0)
        .items(regions)
        .interact()
        .map_err(|e| BosshoggError::Config(format!("prompt: {e}")))?;
    let region = match regions[region_idx] {
        "us" => Region::Us,
        "eu" => Region::Eu,
        _ => Region::SelfHosted,
    };

    let host = if matches!(region, Region::SelfHosted) {
        let h: String = Input::with_theme(&theme)
            .with_prompt("Host URL (https://...)")
            .interact_text()
            .map_err(|e| BosshoggError::Config(format!("prompt: {e}")))?;
        validate_host(&h)?;
        h
    } else {
        region.default_host().to_string()
    };

    let api_key: String = Password::with_theme(&theme)
        .with_prompt("Personal API key (phx_...)")
        .interact()
        .map_err(|e| BosshoggError::Config(format!("prompt: {e}")))?;
    validate_key(&api_key)?;

    // Probe /api/users/@me/ BEFORE saving anything to disk.  Build a temporary
    // client directly from the wizard inputs — nothing is written yet.
    let probe_auth = ResolvedAuth {
        api_key: api_key.clone(),
        host: host.clone(),
        project_id: None,
        env_id: None,
        org_id: None,
        context_name: Some(name.clone()),
    };
    let probe_client = Client::from_resolved(probe_auth, debug)
        .map_err(|e| BosshoggError::Config(format!("failed to build probe client: {e}")))?;
    let me: Value = probe_client.get("/api/users/@me/").await.map_err(|_| {
        BosshoggError::Config(
            "Key rejected by PostHog. Check the key is a valid personal API key (phx_...) \
             and that the host matches the key's region."
                .into(),
        )
    })?;

    // Key validated — extract project/env/org from the response, then save.
    let project_id = me
        .pointer("/team/id")
        .and_then(Value::as_i64)
        .map(|v| v.to_string());
    let env_id = me
        .pointer("/team/id")
        .and_then(Value::as_i64)
        .map(|v| v.to_string());
    let org_id = me
        .pointer("/organization/id")
        .and_then(Value::as_str)
        .map(str::to_string);

    // Optional: project token (phc_) for capture + flag evaluate.
    let project_token = loop {
        let raw: String = Input::with_theme(&theme)
            .with_prompt(
                "Project token (phc_, optional — only needed for `bosshogg capture` + `bosshogg flag evaluate`). Press enter to skip",
            )
            .allow_empty(true)
            .interact_text()
            .map_err(|e| BosshoggError::Config(format!("prompt: {e}")))?;

        if raw.is_empty() {
            break None;
        }
        if raw.starts_with("phc_") {
            break Some(raw);
        }
        eprintln!(
            "Warning: project token should start with 'phc_'. Enter again or press enter to skip."
        );
    };

    let mut cfg = config::load().unwrap_or_else(|_| Config {
        current_context: None,
        contexts: HashMap::new(),
    });
    cfg.contexts.insert(
        name.clone(),
        Context {
            host: host.clone(),
            region: Some(region.as_str().into()),
            api_key: Some(api_key.clone()),
            project_token,
            project_id: project_id.clone(),
            env_id: env_id.clone(),
            org_id: org_id.clone(),
        },
    );
    cfg.current_context = Some(name.clone());
    config::save(&cfg)?;

    if json_mode {
        #[derive(Serialize)]
        struct Out<'a> {
            ok: bool,
            context: &'a str,
            project_id: Option<&'a str>,
            env_id: Option<&'a str>,
            org_id: Option<&'a str>,
        }
        output::print_json(&Out {
            ok: true,
            context: &name,
            project_id: project_id.as_deref(),
            env_id: env_id.as_deref(),
            org_id: org_id.as_deref(),
        });
    } else {
        println!();
        println!("Saved context '{name}'.");
        println!("Next steps:");
        println!("  bosshogg whoami");
        println!("  bosshogg query run \"SELECT 1\"");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_must_be_https() {
        assert!(validate_host("http://example.com").is_err());
        assert!(validate_host("https://us.posthog.com/").is_err());
        assert!(validate_host("https://us.posthog.com").is_ok());
    }

    #[test]
    fn key_requires_phx_prefix() {
        assert!(validate_key("phc_abc123").is_err());
        assert!(validate_key("phx_short").is_err());
        assert!(validate_key("phx_longenoughtokenvalue123").is_ok());
    }

    #[test]
    fn region_default_host_matches_expected_urls() {
        assert_eq!(Region::Us.default_host(), "https://us.posthog.com");
        assert_eq!(Region::Eu.default_host(), "https://eu.posthog.com");
    }
}
