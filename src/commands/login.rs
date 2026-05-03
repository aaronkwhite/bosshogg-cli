//! `bosshogg login` — browser device-flow authentication.
//!
//! Obtains a personal API key from PostHog via the OAuth-style device-code
//! flow (/api/cli-auth/device-code/ + /api/cli-auth/poll/) and saves it to
//! the local config. No browser or redirect URI is required — the user opens a
//! short URL printed by this command.

use std::time::Duration;

use clap::Args;
use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use serde_json::json;
use tokio::time::Instant;

use crate::config::{self, Config, Context};
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Args, Debug)]
pub struct LoginArgs {
    /// PostHog host to authenticate against.
    #[arg(long, default_value = "https://us.posthog.com")]
    pub host: String,

    /// Print the authorization URL instead of opening a browser.
    #[arg(long)]
    pub no_browser: bool,

    /// Config context name to save credentials under.
    /// Defaults to "us", "eu", or "login" based on the host.
    #[arg(long)]
    pub context: Option<String>,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct PollResponse {
    personal_api_key: Option<String>,
}

#[derive(Deserialize)]
struct TeamStub {
    id: serde_json::Value,
}

#[derive(Deserialize)]
struct OrgStub {
    id: String,
}

#[derive(Deserialize)]
struct UsersMeResponse {
    team: Option<TeamStub>,
    organization: Option<OrgStub>,
}

fn anon_client() -> reqwest::Result<reqwest::Client> {
    let http_ok = cfg!(feature = "test-harness") && std::env::var("BOSSHOGG_ALLOW_HTTP").is_ok();
    reqwest::Client::builder()
        .https_only(!http_ok)
        .user_agent(concat!("bosshogg/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()
}

fn context_name_for(host: &str, override_name: Option<String>) -> String {
    override_name.unwrap_or_else(|| {
        if host.contains("eu.posthog.com") {
            "eu".into()
        } else if host.contains("us.posthog.com") {
            "us".into()
        } else {
            "login".into()
        }
    })
}

fn region_for(host: &str) -> Option<String> {
    if host.contains("eu.posthog.com") {
        Some("eu".into())
    } else if host.contains("us.posthog.com") {
        Some("us".into())
    } else {
        Some("self-hosted".into())
    }
}

fn open_browser(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

fn extract_project_id(team: TeamStub) -> Option<String> {
    match team.id {
        serde_json::Value::Number(n) => n.as_u64().map(|v| v.to_string()),
        serde_json::Value::String(s) => Some(s),
        _ => None,
    }
}

pub async fn execute(args: LoginArgs, json_mode: bool) -> Result<()> {
    let client = anon_client().map_err(|e| BosshoggError::Config(format!("HTTP client: {e}")))?;
    let host = args.host.trim_end_matches('/');

    // --- Step 1: request device code ---
    let dc_resp = client
        .post(format!("{host}/api/cli-auth/device-code/"))
        .json(&json!({"use_cases": ["schema", "error_tracking", "endpoints"]}))
        .send()
        .await?;

    if dc_resp.status().as_u16() == 404 {
        return Err(BosshoggError::Config(
            "This PostHog instance doesn't support browser login. \
             Run `bosshogg configure` to paste an API key."
                .into(),
        ));
    }
    if !dc_resp.status().is_success() {
        return Err(BosshoggError::ServerError {
            status: dc_resp.status().as_u16(),
            message: "device-code request failed".into(),
        });
    }

    let dc: DeviceCodeResponse = dc_resp.json().await?;
    let verify_url = dc
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&dc.verification_uri);

    let deadline = Instant::now() + Duration::from_secs(dc.expires_in);
    // Server spec: interval=0 means "poll as fast as you like" — floor at 1s to
    // avoid hammering the endpoint and triggering rate limits.
    let poll_interval = Duration::from_secs(dc.interval.max(1));

    // --- Step 2: show URL / open browser ---
    // JSON mode implies headless: emit a pending object with the URL so the
    // caller can present it to the user, then never open a browser.
    if json_mode {
        output::print_json(&json!({
            "status": "pending",
            "user_code": dc.user_code,
            "verification_uri": verify_url,
        }));
    } else {
        println!("Authorize BossHogg at PostHog:\n");
        println!("  Code: {}", dc.user_code);
        println!("  URL:  {verify_url}\n");
        if args.no_browser {
            println!("Open the URL above in your browser, then wait…");
        } else {
            open_browser(verify_url);
        }
    }

    // --- Step 3: poll until authorized or expired ---
    let api_key = loop {
        tokio::time::sleep(poll_interval).await;

        if Instant::now() >= deadline {
            return Err(BosshoggError::Config(
                "Login timed out. Run `bosshogg login` to start again.".into(),
            ));
        }

        let poll = client
            .post(format!("{host}/api/cli-auth/poll/"))
            .json(&json!({"device_code": &dc.device_code}))
            .send()
            .await?;

        let status = poll.status().as_u16();
        if status == 202 {
            // still pending
            continue;
        }
        if poll.status().is_success() {
            let pr: PollResponse = poll.json().await?;
            match pr.personal_api_key {
                Some(k) => break k,
                None => {
                    return Err(BosshoggError::Config(
                        "poll returned success but no personal_api_key".into(),
                    ));
                }
            }
        } else if status == 400 {
            return Err(BosshoggError::Config(
                "Login expired. Open the URL again or run `bosshogg login` to restart.".into(),
            ));
        } else {
            return Err(BosshoggError::ServerError {
                status,
                message: "poll failed".into(),
            });
        }
    };

    // --- Step 4: fetch /api/users/@me/ for project/org info ---
    let (project_id, org_id) = {
        let me = client
            .get(format!("{host}/api/users/@me/"))
            .header(AUTHORIZATION, format!("Bearer {api_key}"))
            .send()
            .await;
        match me {
            Ok(r) if r.status().is_success() => {
                let body: UsersMeResponse = r.json().await.unwrap_or_else(|e| {
                    tracing::warn!("failed to parse /api/users/@me/ response: {e}");
                    UsersMeResponse {
                        team: None,
                        organization: None,
                    }
                });
                let pid = body.team.and_then(extract_project_id);
                let oid = body.organization.map(|o| o.id);
                (pid, oid)
            }
            _ => (None, None),
        }
    };

    // --- Step 5: persist config BEFORE printing (PostHog deletes cache on first read) ---
    let ctx_name = context_name_for(host, args.context);
    let region = region_for(host);
    let mut cfg: Config = config::load().unwrap_or_default();
    cfg.contexts.insert(
        ctx_name.clone(),
        Context {
            host: host.to_string(),
            region,
            api_key: Some(api_key),
            project_token: None,
            project_id: project_id.clone(),
            // env_id = project_id: best-effort default; overridable via `bosshogg configure`
            env_id: project_id.clone(),
            org_id,
        },
    );
    if cfg.current_context.is_none() {
        cfg.current_context = Some(ctx_name.clone());
    }
    config::save(&cfg)?;

    // --- Step 6: success output ---
    if json_mode {
        output::print_json(&json!({
            "ok": true,
            "context": ctx_name,
            "project_id": project_id,
        }));
    } else {
        println!("Logged in. Context \"{ctx_name}\" saved.");
        if let Some(pid) = &project_id {
            println!("Project: {pid}");
        }
        if ctx_name == "login" {
            println!("Tip: use --context <name> to avoid overwriting this context on future logins.");
        }
        println!("\nRun `bosshogg doctor` to verify setup.");
    }

    Ok(())
}
