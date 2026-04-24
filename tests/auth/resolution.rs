//! Auth resolution chain test (docs/conventions.md § Auth resolution precedence).
//! Uses `temp_env` to isolate env vars per-case.

use bosshogg::client::resolve_auth;
use bosshogg::config::{Config, Context};

fn base_cfg() -> Config {
    let mut cfg = Config::default();
    cfg.contexts.insert(
        "prod".into(),
        Context {
            host: "https://us.posthog.com".into(),
            region: Some("us".into()),
            api_key: Some("phx_from_context".into()),
            project_token: None,
            project_id: Some("999999".into()),
            env_id: None,
            org_id: None,
        },
    );
    cfg.current_context = Some("prod".into());
    cfg
}

#[test]
fn explicit_flag_wins() {
    temp_env::with_vars_unset(
        [
            "POSTHOG_CLI_TOKEN",
            "POSTHOG_CLI_API_KEY",
            "POSTHOG_API_KEY",
        ],
        || {
            let cfg = base_cfg();
            let resolved = resolve_auth(Some("phx_override"), None, &cfg).unwrap();
            assert_eq!(resolved.api_key, "phx_override");
        },
    );
}

#[test]
fn named_context_override_beats_env() {
    temp_env::with_var("POSTHOG_CLI_TOKEN", Some("phx_env"), || {
        let mut cfg = base_cfg();
        cfg.contexts.insert(
            "staging".into(),
            Context {
                host: "https://us.posthog.com".into(),
                region: Some("us".into()),
                api_key: Some("phx_staging".into()),
                project_token: None,
                project_id: None,
                env_id: None,
                org_id: None,
            },
        );
        let resolved = resolve_auth(None, Some("staging"), &cfg).unwrap();
        assert_eq!(resolved.api_key, "phx_staging");
    });
}

#[test]
fn env_beats_default_context() {
    temp_env::with_var("POSTHOG_CLI_TOKEN", Some("phx_from_env"), || {
        let cfg = base_cfg();
        let resolved = resolve_auth(None, None, &cfg).unwrap();
        assert_eq!(resolved.api_key, "phx_from_env");
    });
}

#[test]
fn env_fallback_cli_api_key_var() {
    temp_env::with_vars(
        [
            ("POSTHOG_CLI_TOKEN", None::<&str>),
            ("POSTHOG_CLI_API_KEY", Some("phx_alt")),
            ("POSTHOG_API_KEY", None),
        ],
        || {
            let cfg = Config {
                current_context: None,
                ..Default::default()
            };
            let resolved = resolve_auth(None, None, &cfg).unwrap();
            assert_eq!(resolved.api_key, "phx_alt");
        },
    );
}

#[test]
fn default_context_used_when_no_env_no_flag() {
    temp_env::with_vars_unset(
        [
            "POSTHOG_CLI_TOKEN",
            "POSTHOG_CLI_API_KEY",
            "POSTHOG_API_KEY",
        ],
        || {
            let cfg = base_cfg();
            let resolved = resolve_auth(None, None, &cfg).unwrap();
            assert_eq!(resolved.api_key, "phx_from_context");
        },
    );
}

#[test]
fn missing_everywhere_returns_missing_api_key() {
    temp_env::with_vars_unset(
        [
            "POSTHOG_CLI_TOKEN",
            "POSTHOG_CLI_API_KEY",
            "POSTHOG_API_KEY",
        ],
        || {
            let cfg = Config::default();
            let err = resolve_auth(None, None, &cfg).unwrap_err();
            assert_eq!(err.error_code(), "AUTH_MISSING");
        },
    );
}
