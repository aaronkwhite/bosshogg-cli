//! TOML-backed config at `~/.config/bosshogg/config.toml`.
//!
//! Named **contexts** (kubectl/gh-style) carry host, region, API key,
//! project token (phc_ public key for flag evaluation), project/env/org
//! IDs. One is marked `current`. Load/save roundtrips and migrates the
//! legacy `profiles` shape (lin playbook) to `contexts`.
//!
//! Security: on Unix, the file is chmod 0600 after every save. We never
//! log the contents of an API key; see `src/util.rs::redact_key`.

use std::collections::HashMap;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{BosshoggError, Result};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    /// Which named context is active by default.
    pub current_context: Option<String>,

    /// All known contexts, keyed by name.
    #[serde(default)]
    pub contexts: HashMap<String, Context>,

    /// Anonymous self-tracking telemetry. `None` = default (enabled).
    /// `Some(false)` = explicitly disabled. See `src/analytics.rs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analytics_enabled: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Context {
    pub host: String,
    /// "us", "eu", or "self-hosted".
    #[serde(default)]
    pub region: Option<String>,
    /// Personal API key (phx_ prefix). Primary auth for admin + query endpoints.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Project public token (phc_ prefix). Used ONLY by `flag evaluate` (public /flags endpoint).
    #[serde(default)]
    pub project_token: Option<String>,
    #[serde(default)]
    pub project_id: Option<String>,
    #[serde(default)]
    pub env_id: Option<String>,
    #[serde(default)]
    pub org_id: Option<String>,
}

/// Resolves to `$BOSSHOGG_CONFIG` if set, else `~/.config/bosshogg/config.toml`.
///
/// Falls back to `./bosshogg-config.toml` only if no home directory can be
/// determined (rare; CI containers mostly).
pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("BOSSHOGG_CONFIG") {
        return PathBuf::from(p);
    }
    match dirs::config_dir() {
        Some(base) => base.join("bosshogg").join("config.toml"),
        None => PathBuf::from("bosshogg-config.toml"),
    }
}

/// Read config from disk. Returns `Config::default()` when the file is missing.
/// Silently migrates any legacy `profiles` section into `contexts` (no save —
/// migration persists on the next `save`).
pub fn load() -> Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }

    let raw = std::fs::read_to_string(&path)?;

    // Happy path: parses as the modern `Config` shape.
    if let Ok(cfg) = toml::from_str::<Config>(&raw) {
        if !cfg.contexts.is_empty() || cfg.current_context.is_some() {
            return Ok(cfg);
        }
    }

    // Legacy shape from the lin playbook: top-level `default_profile` + `[profiles.*]`.
    #[derive(Deserialize)]
    struct Legacy {
        default_profile: Option<String>,
        #[serde(default)]
        profiles: HashMap<String, Context>,
    }

    let legacy: Legacy = toml::from_str(&raw)?;
    Ok(Config {
        current_context: legacy.default_profile,
        contexts: legacy.profiles,
        analytics_enabled: None,
    })
}

/// Write config to disk, creating the parent directory and setting 0600 perms on Unix.
pub fn save(config: &Config) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let body = toml::to_string_pretty(config).map_err(|e| BosshoggError::Config(e.to_string()))?;

    // Atomic-ish: write to temp in same dir, then rename.
    let tmp = path.with_extension("toml.tmp");
    {
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).write(true).truncate(true);
        #[cfg(unix)]
        opts.mode(0o600);
        let mut f = opts.open(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.flush()?;
    }
    std::fs::rename(&tmp, &path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o600);
        std::fs::set_permissions(&path, perms)?;
    }

    Ok(())
}

/// Directory where bosshogg stores per-user state (config, analytics queue,
/// install id). Resolves under `$BOSSHOGG_CONFIG`'s parent if set, otherwise
/// `~/.config/bosshogg`. Returns `None` only if no home directory is
/// resolvable AND `$BOSSHOGG_CONFIG` is unset (rare; CI containers).
pub fn data_dir() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("BOSSHOGG_CONFIG") {
        return PathBuf::from(p).parent().map(PathBuf::from);
    }
    dirs::config_dir().map(|d| d.join("bosshogg"))
}

/// Whether anonymous self-tracking is enabled. Checks in order:
/// 1. `DO_NOT_TRACK=1` → disabled (industry-standard opt-out).
/// 2. Config `analytics_enabled = false` → disabled.
/// 3. Otherwise → enabled (opt-out default).
pub fn is_analytics_enabled() -> bool {
    if std::env::var("DO_NOT_TRACK").ok().as_deref() == Some("1") {
        return false;
    }
    !matches!(load().ok().and_then(|c| c.analytics_enabled), Some(false))
}

/// Persist the `analytics_enabled` setting. `None` removes the field
/// (revert to default). Creates the config file if absent.
pub fn set_analytics_enabled(value: Option<bool>) -> Result<()> {
    let mut cfg = load().unwrap_or_default();
    cfg.analytics_enabled = value;
    save(&cfg)
}

/// Region string of the active context (or the named override). Used as a
/// telemetry property so we can split self-tracking by US / EU / self-hosted
/// without identifying anyone. `None` if no config / no current context.
pub fn active_region(context_override: Option<&str>) -> Option<String> {
    let cfg = load().ok()?;
    let name = context_override
        .map(String::from)
        .or(cfg.current_context.clone())?;
    cfg.contexts.get(&name)?.region.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn with_fake_home<F: FnOnce(&std::path::Path)>(f: F) {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().to_path_buf();
        // Unset XDG_CONFIG_HOME so `dirs::config_dir()` falls back to
        // `$HOME/.config` on Linux. Without this, CI's real XDG path leaks in
        // and the `starts_with(home)` assertion fails.
        temp_env::with_vars(
            [
                ("HOME", Some(home.to_str().unwrap().to_string())),
                ("XDG_CONFIG_HOME", None),
            ],
            || f(&home),
        );
    }

    #[test]
    fn config_path_under_home_xdg() {
        with_fake_home(|home| {
            let p = config_path();
            assert!(p.starts_with(home), "{p:?} should be under {home:?}");
            assert!(p.ends_with("bosshogg/config.toml"));
        });
    }

    #[test]
    fn load_missing_returns_default() {
        with_fake_home(|_| {
            let cfg = load().unwrap();
            assert!(cfg.contexts.is_empty());
            assert_eq!(cfg.current_context, None);
        });
    }

    #[test]
    fn save_then_load_roundtrip() {
        with_fake_home(|_| {
            let mut cfg = Config::default();
            cfg.contexts.insert(
                "prod-us".into(),
                Context {
                    host: "https://us.posthog.com".into(),
                    region: Some("us".into()),
                    api_key: Some("phx_secret".into()),
                    project_token: Some("phc_public_token".into()),
                    project_id: Some("999999".into()),
                    env_id: None,
                    org_id: None,
                },
            );
            cfg.current_context = Some("prod-us".into());

            save(&cfg).unwrap();

            let loaded = load().unwrap();
            assert_eq!(loaded.current_context.as_deref(), Some("prod-us"));
            let ctx = loaded.contexts.get("prod-us").expect("ctx present");
            assert_eq!(ctx.project_id.as_deref(), Some("999999"));
            assert_eq!(ctx.api_key.as_deref(), Some("phx_secret"));
            assert_eq!(ctx.project_token.as_deref(), Some("phc_public_token"));
        });
    }

    #[test]
    fn save_sets_0600_perms_on_unix() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            with_fake_home(|_| {
                save(&Config::default()).unwrap();
                let perms = std::fs::metadata(config_path()).unwrap().permissions();
                assert_eq!(perms.mode() & 0o777, 0o600);
            });
        }
    }

    #[test]
    fn legacy_profiles_migrate_to_contexts_on_load() {
        with_fake_home(|_| {
            let path = config_path();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(
                &path,
                r#"
default_profile = "prod"

[profiles.prod]
host = "https://us.posthog.com"
api_key = "phx_old"
project_id = "999999"
"#,
            )
            .unwrap();

            let cfg = load().unwrap();
            assert_eq!(cfg.current_context.as_deref(), Some("prod"));
            let ctx = cfg.contexts.get("prod").expect("migrated");
            assert_eq!(ctx.api_key.as_deref(), Some("phx_old"));
            assert_eq!(ctx.project_id.as_deref(), Some("999999"));
        });
    }
}
