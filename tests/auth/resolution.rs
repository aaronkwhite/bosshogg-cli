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
            allow_http: false,
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
                allow_http: false,
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

// ── allow_http resolution ────────────────────────────────────────────────

#[test]
fn allow_http_default_is_false() {
    temp_env::with_vars_unset(
        [
            "POSTHOG_CLI_TOKEN",
            "POSTHOG_CLI_API_KEY",
            "POSTHOG_API_KEY",
            "BOSSHOGG_ALLOW_HTTP",
        ],
        || {
            let cfg = base_cfg();
            let resolved = resolve_auth(None, None, &cfg).unwrap();
            assert!(!resolved.allow_http);
        },
    );
}

#[test]
fn allow_http_env_var_strict_one() {
    temp_env::with_vars_unset(
        [
            "POSTHOG_CLI_TOKEN",
            "POSTHOG_CLI_API_KEY",
            "POSTHOG_API_KEY",
        ],
        || {
            // Strict "1" — empty and "0" must NOT activate.
            for (val, expected) in [
                (Some("1"), true),
                (Some("0"), false),
                (Some(""), false),
                (Some("true"), false),
                (None, false),
            ] {
                temp_env::with_var("BOSSHOGG_ALLOW_HTTP", val, || {
                    let cfg = base_cfg();
                    let resolved = resolve_auth(None, None, &cfg).unwrap();
                    assert_eq!(
                        resolved.allow_http, expected,
                        "BOSSHOGG_ALLOW_HTTP={val:?} should resolve to allow_http={expected}",
                    );
                });
            }
        },
    );
}

#[test]
fn allow_http_propagates_from_context() {
    temp_env::with_vars_unset(
        [
            "POSTHOG_CLI_TOKEN",
            "POSTHOG_CLI_API_KEY",
            "POSTHOG_API_KEY",
            "BOSSHOGG_ALLOW_HTTP",
        ],
        || {
            let mut cfg = base_cfg();
            cfg.contexts.insert(
                "onprem".into(),
                Context {
                    host: "http://posthog.internal".into(),
                    region: Some("self-hosted".into()),
                    api_key: Some("phx_onprem".into()),
                    project_token: None,
                    project_id: None,
                    env_id: None,
                    org_id: None,
                    allow_http: true,
                },
            );
            let resolved = resolve_auth(None, Some("onprem"), &cfg).unwrap();
            assert!(resolved.allow_http);
            assert_eq!(resolved.host, "http://posthog.internal");
        },
    );
}

#[test]
fn allow_http_env_or_context_wins() {
    // Either source enabling allow_http is sufficient.
    temp_env::with_vars_unset(
        [
            "POSTHOG_CLI_TOKEN",
            "POSTHOG_CLI_API_KEY",
            "POSTHOG_API_KEY",
        ],
        || {
            // Env on, context off → allowed.
            temp_env::with_var("BOSSHOGG_ALLOW_HTTP", Some("1"), || {
                let cfg = base_cfg(); // base_cfg sets allow_http: false on the prod context
                let resolved = resolve_auth(None, None, &cfg).unwrap();
                assert!(resolved.allow_http);
            });
        },
    );
}
