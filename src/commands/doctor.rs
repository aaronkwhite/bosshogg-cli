use chrono::{DateTime, Utc};
use clap::Args;
use serde::Serialize;
use serde_json::Value;

use crate::client::{Client, ResolvedAuth, resolve_auth};
use crate::config;
use crate::error::{BosshoggError, Result};
use crate::output;

#[derive(Args, Debug)]
pub struct DoctorArgs {}

#[derive(Serialize, Debug)]
pub struct Check {
    pub name: &'static str,
    pub ok: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

#[derive(Serialize, Debug)]
struct DoctorOutput {
    checks: Vec<Check>,
    summary: Summary,
}

#[derive(Serialize, Debug)]
struct Summary {
    ok: bool,
    passed: usize,
    failed: usize,
}

pub async fn execute(
    _args: DoctorArgs,
    json_mode: bool,
    debug: bool,
    context: Option<&str>,
) -> Result<()> {
    let mut checks: Vec<Check> = Vec::new();

    checks.push(check_binary_path());

    let cfg = match config::load() {
        Ok(c) => {
            checks.push(Check {
                name: "config_file",
                ok: true,
                message: "config file parses".into(),
                remediation: None,
            });
            Some(c)
        }
        Err(e) => {
            checks.push(Check {
                name: "config_file",
                ok: false,
                message: format!("{e}"),
                remediation: Some("run `bosshogg configure`".into()),
            });
            None
        }
    };

    let resolved: Option<ResolvedAuth> = cfg
        .as_ref()
        .and_then(|c| resolve_auth(None, context, c).ok());

    checks.push(check_auth_source(cfg.as_ref(), resolved.as_ref(), context));
    checks.push(check_api_key_present(resolved.as_ref()));
    checks.push(check_host_region(resolved.as_ref()));

    // Network checks — only when we have an api key.
    if let Some(auth) = resolved.as_ref() {
        if let Ok(client) = Client::from_resolved(auth.clone(), debug) {
            checks.push(check_key_alive(&client).await);
            checks.push(check_project_access(&client, auth.project_id.as_deref()).await);
            checks.push(check_env_access(&client, auth.env_id.as_deref()).await);
        }
    }

    let passed = checks.iter().filter(|c| c.ok).count();
    let failed = checks.len() - passed;
    let summary = Summary {
        ok: failed == 0,
        passed,
        failed,
    };

    let out = DoctorOutput { checks, summary };
    if json_mode {
        // In JSON mode: always exit 0 when the JSON is valid (even on failed checks).
        // Agents branch on summary.ok; a non-zero exit here would cause main.rs to
        // emit a second JSON error object on stderr alongside the valid doctor output.
        output::print_json(&out);
    } else {
        for c in &out.checks {
            let marker = if c.ok { "[OK]" } else { "[FAIL]" };
            println!("{marker}  {:<20} {}", c.name, c.message);
            if let Some(r) = &c.remediation {
                println!("       -> {r}");
            }
        }
        println!();
        println!(
            "{} passed, {} failed",
            out.summary.passed, out.summary.failed
        );
        // In human-readable mode, exit non-zero so shell callers can detect failure.
        if !out.summary.ok {
            return Err(BosshoggError::Config("doctor found problems".into()));
        }
    }
    Ok(())
}

fn check_binary_path() -> Check {
    match std::env::current_exe() {
        Ok(p) => Check {
            name: "binary_path",
            ok: true,
            message: p.display().to_string(),
            remediation: None,
        },
        Err(e) => Check {
            name: "binary_path",
            ok: false,
            message: format!("{e}"),
            remediation: None,
        },
    }
}

fn check_auth_source(
    cfg: Option<&crate::config::Config>,
    resolved: Option<&ResolvedAuth>,
    flag_context: Option<&str>,
) -> Check {
    let env_key_set = std::env::var("POSTHOG_CLI_TOKEN").is_ok()
        || std::env::var("POSTHOG_CLI_API_KEY").is_ok()
        || std::env::var("POSTHOG_API_KEY").is_ok();
    let persisted_ctx_name = resolved.and_then(|r| r.context_name.clone());

    match (env_key_set, persisted_ctx_name.as_deref(), flag_context) {
        (_, Some(name), _) if env_key_set => Check {
            name: "auth_source",
            ok: true,
            message: format!("env vars (key) + context '{name}' (host/ids)"),
            remediation: None,
        },
        (true, _, _) => Check {
            name: "auth_source",
            ok: true,
            message: "env vars (POSTHOG_CLI_* / POSTHOG_API_KEY)".into(),
            remediation: None,
        },
        (false, Some(name), _) => Check {
            name: "auth_source",
            ok: true,
            message: format!("persisted context '{name}'"),
            remediation: None,
        },
        (false, None, Some(flag)) => Check {
            name: "auth_source",
            ok: false,
            message: format!("requested context '{flag}' not found in config"),
            remediation: Some("run `bosshogg configure` or pick another context".into()),
        },
        (false, None, None) => {
            // No env var. Maybe a current_context is set but points to nothing usable.
            let hint = cfg
                .and_then(|c| c.current_context.clone())
                .map(|n| format!("current_context '{n}' missing or has no api_key"));
            Check {
                name: "auth_source",
                ok: false,
                message: hint.unwrap_or_else(|| "no auth source (no env var, no context)".into()),
                remediation: Some(
                    "run `bosshogg configure`, or export POSTHOG_CLI_TOKEN=phx_...".into(),
                ),
            }
        }
    }
}

fn check_api_key_present(resolved: Option<&ResolvedAuth>) -> Check {
    match resolved {
        Some(r) if !r.api_key.is_empty() => Check {
            name: "api_key_present",
            ok: true,
            message: "api key present".into(),
            remediation: None,
        },
        _ => Check {
            name: "api_key_present",
            ok: false,
            message: "no api key resolved".into(),
            remediation: Some(
                "run `bosshogg configure` or export POSTHOG_CLI_TOKEN=phx_...".into(),
            ),
        },
    }
}

fn check_host_region(resolved: Option<&ResolvedAuth>) -> Check {
    let Some(r) = resolved else {
        return Check {
            name: "host_region_match",
            ok: false,
            message: "no host resolved".into(),
            remediation: None,
        };
    };
    // Match host against known cloud endpoints. Anything else is treated as self-hosted.
    let region = match r.host.as_str() {
        "https://us.posthog.com" => Some("us"),
        "https://eu.posthog.com" => Some("eu"),
        _ => None,
    };
    match region {
        Some(code) => Check {
            name: "host_region_match",
            ok: true,
            message: format!("host '{}' ({} cloud)", r.host, code),
            remediation: None,
        },
        None => Check {
            name: "host_region_match",
            ok: true,
            message: format!("host '{}' (self-hosted or custom)", r.host),
            remediation: None,
        },
    }
}

async fn check_key_alive(client: &Client) -> Check {
    match client.get::<Value>("/api/users/@me/").await {
        Ok(_) => Check {
            name: "key_alive",
            ok: true,
            message: "personal API key authenticates".into(),
            remediation: None,
        },
        Err(e) => Check {
            name: "key_alive",
            ok: false,
            message: format!("{e}"),
            remediation: Some("verify key scope and region; rotate via PostHog settings".into()),
        },
    }
}

async fn check_project_access(client: &Client, project_id: Option<&str>) -> Check {
    let Some(pid) = project_id else {
        return Check {
            name: "project_access",
            ok: false,
            message: "no project_id configured".into(),
            remediation: Some(
                "set POSTHOG_CLI_PROJECT_ID or `bosshogg config set-context <name> --project <id>`"
                    .into(),
            ),
        };
    };
    match client.get::<Value>(&format!("/api/projects/{pid}/")).await {
        Ok(_) => Check {
            name: "project_access",
            ok: true,
            message: format!("project {pid} accessible"),
            remediation: None,
        },
        Err(e) => Check {
            name: "project_access",
            ok: false,
            message: format!("{e}"),
            remediation: Some("check project id and key scopes (project:read)".into()),
        },
    }
}

async fn check_env_access(client: &Client, env_id: Option<&str>) -> Check {
    let Some(eid) = env_id else {
        return Check {
            name: "env_access",
            ok: true,
            message: "no env_id configured (optional)".into(),
            remediation: None,
        };
    };
    match client
        .get::<Value>(&format!("/api/environments/{eid}/"))
        .await
    {
        Ok(_) => Check {
            name: "env_access",
            ok: true,
            message: format!("environment {eid} accessible"),
            remediation: None,
        },
        Err(e) => Check {
            name: "env_access",
            ok: false,
            message: format!("{e}"),
            remediation: Some("verify env_id and key scopes".into()),
        },
    }
}

// Retained for doc-symmetry with the spec; clock skew detection requires the
// Phase B client to expose response headers. If that helper exists when this
// task runs, replace the inlined skew stub above with a real implementation
// that parses `Date:` into chrono::DateTime<Utc> and compares to Utc::now().
#[allow(dead_code)]
fn clock_skew(server_date: &str) -> Option<i64> {
    DateTime::parse_from_rfc2822(server_date)
        .ok()
        .map(|d| (Utc::now() - d.with_timezone(&Utc)).num_seconds())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_skew_parses_rfc2822() {
        let skew = clock_skew("Tue, 21 Apr 2026 12:00:00 GMT");
        assert!(skew.is_some());
    }

    #[test]
    fn clock_skew_rejects_garbage() {
        assert!(clock_skew("not a date").is_none());
    }
}
