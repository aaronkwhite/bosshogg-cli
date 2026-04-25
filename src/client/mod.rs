//! HTTP client for PostHog's REST + Query APIs.
//!
//! ## Security posture
//! - `https_only(true)` on the reqwest client — rejects redirects to HTTP.
//! - `Authorization:` redacted in debug output (auth_token never reaches tracing).
//! - Error response bodies clipped to 200 chars in debug output / logs.
//! - We DO NOT search `current_exe().parent()` for `.env` files — credential
//!   hijack vector (lesson from the `lin` security review). Auth resolution
//!   reads env vars, CLI flags, and config only.

pub mod cache;
pub mod query;

use std::time::Duration;

use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue, USER_AGENT};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tracing::debug;

use crate::config::{self, Config, Context};
use crate::error::{BosshoggError, Result};
use crate::util::redact_key;

pub use cache::Cache;
pub use query::{QueryKind, QueryResponse};

/// Resources that refuse hard DELETE (return 405). `Client::delete` transparently
/// rewrites these to `PATCH {"deleted": true}`. Only `flag` (feature_flags) is
/// relevant in M1 but the full list lives here so later milestones don't split
/// the source of truth. Mirrors docs/api-notes.md § Soft-delete.
pub(crate) const SOFT_DELETE_RESOURCES: &[&str] = &[
    "insights",
    "feature_flags",
    "cohorts",
    "annotations",
    "subscriptions",
    "hog_functions",
    "actions",
    "error_tracking/fingerprints",
    "dashboards",
];

pub struct Client {
    http: reqwest::Client,
    api_key: String,
    host: String,
    project_id: Option<String>,
    env_id: Option<String>,
    org_id: Option<String>,
    debug: bool,
    cache: Cache,
}

/// Result of walking the auth resolution chain.
#[derive(Debug, Clone)]
pub struct ResolvedAuth {
    pub api_key: String,
    pub host: String,
    pub project_id: Option<String>,
    pub env_id: Option<String>,
    pub org_id: Option<String>,
    /// The context (if any) whose values populated non-key fields.
    pub context_name: Option<String>,
}

pub fn resolve_auth(
    flag_key: Option<&str>,
    flag_context: Option<&str>,
    cfg: &Config,
) -> Result<ResolvedAuth> {
    let ctx: Option<(&str, &Context)> = if let Some(name) = flag_context {
        let c = cfg
            .contexts
            .get(name)
            .ok_or_else(|| BosshoggError::Config(format!("unknown context: {name}")))?;
        Some((name, c))
    } else {
        cfg.current_context
            .as_deref()
            .and_then(|n| cfg.contexts.get(n).map(|c| (n, c)))
    };

    let flag_key_s = flag_key.map(|s| s.to_string());

    let env_key = std::env::var("POSTHOG_CLI_TOKEN")
        .ok()
        .or_else(|| std::env::var("POSTHOG_CLI_API_KEY").ok())
        .or_else(|| std::env::var("POSTHOG_API_KEY").ok());

    let api_key = flag_key_s
        .or_else(|| {
            if flag_context.is_some() {
                ctx.and_then(|(_, c)| c.api_key.clone())
            } else {
                None
            }
        })
        .or(env_key)
        .or_else(|| ctx.and_then(|(_, c)| c.api_key.clone()))
        .ok_or(BosshoggError::MissingApiKey)?;

    let host = std::env::var("POSTHOG_CLI_HOST")
        .ok()
        .or_else(|| std::env::var("POSTHOG_HOST").ok())
        .or_else(|| ctx.map(|(_, c)| c.host.clone()))
        .unwrap_or_else(|| "https://us.posthog.com".to_string());

    let project_id = std::env::var("POSTHOG_CLI_PROJECT_ID")
        .ok()
        .or_else(|| std::env::var("POSTHOG_PROJECT_ID").ok())
        .or_else(|| ctx.and_then(|(_, c)| c.project_id.clone()));

    let env_id = std::env::var("POSTHOG_CLI_ENV_ID")
        .ok()
        .or_else(|| std::env::var("POSTHOG_ENV_ID").ok())
        .or_else(|| ctx.and_then(|(_, c)| c.env_id.clone()));

    let org_id = std::env::var("POSTHOG_CLI_ORG_ID")
        .ok()
        .or_else(|| std::env::var("POSTHOG_ORG_ID").ok())
        .or_else(|| ctx.and_then(|(_, c)| c.org_id.clone()));

    Ok(ResolvedAuth {
        api_key,
        host,
        project_id,
        env_id,
        org_id,
        context_name: ctx.map(|(n, _)| n.to_string()),
    })
}

impl Client {
    pub fn new(context_name: Option<&str>, debug: bool) -> Result<Self> {
        let cfg = config::load()?;
        let auth = resolve_auth(None, context_name, &cfg)?;
        Self::from_resolved(auth, debug)
    }

    pub fn from_resolved(auth: ResolvedAuth, debug: bool) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let mut val = HeaderValue::from_str(&format!("Bearer {}", auth.api_key))
            .map_err(|_| BosshoggError::InvalidApiKey)?;
        val.set_sensitive(true);
        headers.insert(AUTHORIZATION, val);
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("bosshogg/", env!("CARGO_PKG_VERSION"))),
        );

        let http_allowed_in_test_harness =
            cfg!(feature = "test-harness") && std::env::var("BOSSHOGG_ALLOW_HTTP").is_ok();
        if http_allowed_in_test_harness {
            tracing::warn!(
                "TLS downgraded via BOSSHOGG_ALLOW_HTTP (test-harness feature); never use in production"
            );
        }
        let http = reqwest::Client::builder()
            .https_only(!http_allowed_in_test_harness)
            .gzip(true)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(60))
            .default_headers(headers)
            .build()?;

        debug!(
            context = auth.context_name.as_deref().unwrap_or("<none>"),
            host = %auth.host,
            key = %redact_key(&auth.api_key),
            "bosshogg client initialised"
        );

        Ok(Self {
            http,
            api_key: auth.api_key,
            host: auth.host,
            project_id: auth.project_id,
            env_id: auth.env_id,
            org_id: auth.org_id,
            debug,
            cache: Cache::new(),
        })
    }

    pub fn api_key(&self) -> &str {
        &self.api_key
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn project_id(&self) -> Option<&str> {
        self.project_id.as_deref()
    }

    pub fn env_id(&self) -> Option<&str> {
        self.env_id.as_deref()
    }

    pub fn org_id(&self) -> Option<&str> {
        self.org_id.as_deref()
    }

    pub fn cache(&self) -> &Cache {
        &self.cache
    }

    #[allow(dead_code)]
    pub(crate) fn http(&self) -> &reqwest::Client {
        &self.http
    }

    #[allow(dead_code)]
    pub(crate) fn debug_enabled(&self) -> bool {
        self.debug
    }
}

impl Client {
    /// Test-only constructor: skips `https_only(true)` so wiremock (http://) works.
    #[doc(hidden)]
    pub fn for_test(auth: ResolvedAuth, debug: bool) -> Result<Self> {
        let mut headers = HeaderMap::new();
        let mut val = HeaderValue::from_str(&format!("Bearer {}", auth.api_key))
            .map_err(|_| BosshoggError::InvalidApiKey)?;
        val.set_sensitive(true);
        headers.insert(AUTHORIZATION, val);
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(concat!("bosshogg/", env!("CARGO_PKG_VERSION"))),
        );

        let http = reqwest::Client::builder()
            .gzip(true)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(30))
            .default_headers(headers)
            .build()?;

        Ok(Self {
            http,
            api_key: auth.api_key,
            host: auth.host,
            project_id: auth.project_id,
            env_id: auth.env_id,
            org_id: auth.org_id,
            debug,
            cache: Cache::new(),
        })
    }

    fn url(&self, path: &str) -> String {
        let trimmed = path.trim_start_matches('/');
        format!("{}/{}", self.host.trim_end_matches('/'), trimmed)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = self.url(path);
        self.send_with_retry(|| self.http.get(&url), path).await
    }

    pub async fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let url = self.url(path);
        self.send_with_retry(|| self.http.post(&url).json(body), path)
            .await
    }

    pub async fn patch<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let url = self.url(path);
        self.send_with_retry(|| self.http.patch(&url).json(body), path)
            .await
    }

    pub async fn put<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T> {
        let url = self.url(path);
        self.send_with_retry(|| self.http.put(&url).json(body), path)
            .await
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        if is_soft_delete_path(path) {
            let _: Value = self
                .patch(path, &serde_json::json!({"deleted": true}))
                .await?;
            return Ok(());
        }

        let url = self.url(path);
        let _: Value = self
            .send_with_retry(|| self.http.delete(&url), path)
            .await?;
        Ok(())
    }

    async fn send_with_retry<T, B>(&self, mut build: B, path: &str) -> Result<T>
    where
        T: DeserializeOwned,
        B: FnMut() -> reqwest::RequestBuilder,
    {
        const MAX_ATTEMPTS: u32 = 3;
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            let req = build();

            if self.debug {
                debug!(attempt, path, "request");
            }

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) if attempt < MAX_ATTEMPTS && (e.is_timeout() || e.is_connect()) => {
                    backoff_sleep(attempt).await;
                    continue;
                }
                Err(e) => return Err(BosshoggError::Http(e)),
            };

            let status = resp.status();
            let retry_after = resp
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());

            if status.is_success() {
                if status.as_u16() == 204 {
                    let v: Value = serde_json::json!({});
                    return serde_json::from_value(v).map_err(BosshoggError::Json);
                }
                let bytes = resp.bytes().await.map_err(BosshoggError::Http)?;
                if bytes.is_empty() {
                    let v: Value = serde_json::json!({});
                    return serde_json::from_value(v).map_err(BosshoggError::Json);
                }
                return serde_json::from_slice::<T>(&bytes).map_err(BosshoggError::Json);
            }

            let retryable = status.as_u16() == 429 || matches!(status.as_u16(), 502..=504);
            if retryable && attempt < MAX_ATTEMPTS {
                if let Some(secs) = retry_after {
                    tokio::time::sleep(Duration::from_secs(secs)).await;
                } else {
                    backoff_sleep(attempt).await;
                }
                continue;
            }

            let body_text = resp.text().await.unwrap_or_default();
            let truncated = truncate_body(&body_text, 200);
            if self.debug {
                debug!(status = %status, body = %truncated, "error response");
            }

            return Err(map_status(status.as_u16(), &body_text, retry_after));
        }
    }
}

async fn backoff_sleep(attempt: u32) {
    let secs = 1u64 << (attempt.saturating_sub(1));
    tokio::time::sleep(Duration::from_secs(secs)).await;
}

fn truncate_body(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let cut: String = s.chars().take(n).collect();
        format!("{cut}… [truncated]")
    }
}

/// Returns true when a URL path segment looks like an id (numeric or UUID/slug)
/// rather than a resource name. Used by `is_soft_delete_path` to strip trailing
/// id segments before checking the resource name.
fn is_id_segment(seg: &str) -> bool {
    if seg.is_empty() {
        return false;
    }
    // Pure numeric
    if seg.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    // UUID or hex-slug: only hex digits and dashes, and contains at least one dash
    // (avoids false-positives on all-lowercase resource names like "actions").
    let is_hex_dash = seg.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if is_hex_dash && seg.contains('-') {
        return true;
    }
    false
}

fn is_soft_delete_path(path: &str) -> bool {
    let mut segments: Vec<&str> = path
        .trim_start_matches('/')
        .trim_end_matches('/')
        .split('/')
        .collect();

    // Drop trailing id segment if present (best-effort).
    // Matches: pure numeric ids, UUIDs (hex + dashes), or any non-empty segment
    // that looks like an identifier (not a known API scope keyword).
    if let Some(last) = segments.last() {
        if !last.is_empty() && is_id_segment(last) {
            segments.pop();
        }
    }

    // Expect: api / {projects|environments|organizations} / <scope_id> / <resource...>
    if segments.len() < 4 || segments[0] != "api" {
        return false;
    }

    // Resource is everything from segments[3..].join("/")
    let resource = segments[3..].join("/");

    SOFT_DELETE_RESOURCES.iter().any(|r| *r == resource)
}

fn map_status(status: u16, body: &str, retry_after: Option<u64>) -> BosshoggError {
    match status {
        401 => BosshoggError::InvalidApiKey,
        403 => {
            if let Some(scope) = extract_scope(body) {
                BosshoggError::MissingScope {
                    scope,
                    message: first_line(body).unwrap_or_else(|| "permission denied".into()),
                }
            } else {
                BosshoggError::ServerError {
                    status,
                    message: first_line(body).unwrap_or_else(|| "forbidden".into()),
                }
            }
        }
        404 => {
            BosshoggError::NotFound(first_line(body).unwrap_or_else(|| "resource not found".into()))
        }
        400 | 422 => {
            BosshoggError::BadRequest(first_line(body).unwrap_or_else(|| "bad request".into()))
        }
        429 => BosshoggError::RateLimit {
            retry_after_s: retry_after.unwrap_or(60),
            bucket: infer_bucket(body),
        },
        500..=599 => BosshoggError::ServerError {
            status,
            message: first_line(body).unwrap_or_else(|| "upstream error".into()),
        },
        _ => BosshoggError::ServerError {
            status,
            message: first_line(body).unwrap_or_else(|| "unexpected status".into()),
        },
    }
}

fn first_line(body: &str) -> Option<String> {
    if let Ok(v) = serde_json::from_str::<Value>(body) {
        if let Some(d) = v.get("detail").and_then(|d| d.as_str()) {
            return Some(d.to_string());
        }
    }
    body.lines().next().map(|l| truncate_body(l, 200))
}

fn extract_scope(body: &str) -> Option<String> {
    let lower = body.to_ascii_lowercase();
    let idx = lower.find("scope '")?;
    let rest = &body[idx + "scope '".len()..];
    let end = rest.find('\'')?;
    Some(rest[..end].to_string())
}

fn infer_bucket(body: &str) -> String {
    let lower = body.to_ascii_lowercase();
    if lower.contains("query") {
        "query".into()
    } else if lower.contains("write") || lower.contains("crud") {
        "crud".into()
    } else {
        "analytics".into()
    }
}

// Keep C4 stub alive until that task lands.
impl Client {
    /// GET a paginated DRF list endpoint, auto-following `next` until exhausted
    /// or `limit` items have been collected.
    pub async fn get_paginated<T: DeserializeOwned>(
        &self,
        path: &str,
        limit: Option<usize>,
    ) -> Result<Vec<T>> {
        #[derive(serde::Deserialize)]
        struct Page<U> {
            #[serde(default)]
            next: Option<String>,
            results: Vec<U>,
        }

        let mut collected: Vec<T> = Vec::new();
        let first_url = self.url(path);
        let mut current = first_url;

        loop {
            let url_for_log = current.clone();
            let current_for_build = current.clone();
            let page: Page<T> = self
                .send_with_retry(move || self.http.get(&current_for_build), &url_for_log)
                .await?;
            collected.extend(page.results);
            if let Some(max) = limit {
                if collected.len() >= max {
                    collected.truncate(max);
                    break;
                }
            }
            match page.next {
                Some(u) => current = u,
                None => break,
            }
        }

        Ok(collected)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soft_delete_path_feature_flags_with_id() {
        assert!(is_soft_delete_path("/api/projects/1/feature_flags/42/"));
    }

    #[test]
    fn soft_delete_path_feature_flags_base_route() {
        assert!(is_soft_delete_path("/api/projects/1/feature_flags/"));
    }

    #[test]
    fn soft_delete_path_persons_is_false() {
        assert!(!is_soft_delete_path("/api/projects/1/persons/foo/"));
    }

    #[test]
    fn soft_delete_path_cohort_actions_false_positive_guard() {
        // cohort_actions contains "actions" as a substring; anchored match must reject it.
        assert!(!is_soft_delete_path("/api/projects/1/cohort_actions/foo/"));
    }

    #[test]
    fn soft_delete_path_error_tracking_fingerprints() {
        assert!(is_soft_delete_path(
            "/api/projects/1/error_tracking/fingerprints/9/"
        ));
    }

    #[test]
    fn soft_delete_path_environments_scope() {
        assert!(is_soft_delete_path("/api/environments/5/feature_flags/99/"));
    }

    #[test]
    fn soft_delete_path_hog_functions() {
        assert!(is_soft_delete_path(
            "/api/projects/1/hog_functions/abc-123/"
        ));
    }

    #[test]
    fn soft_delete_path_dashboards_with_id() {
        assert!(is_soft_delete_path("/api/environments/5/dashboards/99/"));
    }

    #[test]
    fn soft_delete_path_dashboards_projects_scope() {
        assert!(is_soft_delete_path("/api/projects/1/dashboards/42/"));
    }

    #[test]
    fn soft_delete_path_dashboards_base_route() {
        assert!(is_soft_delete_path("/api/environments/5/dashboards/"));
    }
}
