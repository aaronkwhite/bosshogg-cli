//! Error types shared across BossHogg.
//!
//! `BosshoggError` is `#[non_exhaustive]` so new variants don't force SemVer
//! majors. Exit codes and SCREAMING_SNAKE error codes are the stable public
//! contract (see docs/conventions.md § Error code catalog).
//!
//! Security: never put a full API token in an error message or Debug
//! output. The HTTP client redacts `Authorization:` headers and truncates
//! bodies to 200 chars before they reach this type.

use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BosshoggError {
    #[error("missing API key (set POSTHOG_CLI_TOKEN, run `bosshogg configure`, or pass --api-key)")]
    MissingApiKey,

    #[error("invalid API key: rejected by PostHog")]
    InvalidApiKey,

    #[error("missing scope {scope}: {message}")]
    MissingScope { scope: String, message: String },

    #[error("HTTP {status}: {message}")]
    ServerError { status: u16, message: String },

    #[error("rate limited — retry after {retry_after_s}s (bucket: {bucket})")]
    RateLimit { retry_after_s: u64, bucket: String },

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("HogQL error: {0}")]
    HogQL(String),

    #[error("config error: {0}")]
    Config(String),

    #[error(transparent)]
    Http(#[from] reqwest::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}

pub type Result<T> = std::result::Result<T, BosshoggError>;

impl BosshoggError {
    /// Stable exit codes — see docs/conventions.md § Exit codes.
    ///
    /// Buckets: 10–12 auth, 20 not-found, 30–32 client, 40 rate, 50–52
    /// upstream/network/timeout, 60 schema drift, 70 internal.
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::MissingApiKey => 10,
            Self::InvalidApiKey => 11,
            Self::MissingScope { .. } => 12,
            Self::NotFound(_) => 20,
            Self::BadRequest(_) | Self::HogQL(_) => 30,
            Self::RateLimit { .. } => 40,
            Self::ServerError { .. } => 50,
            Self::Http(_) => 51,
            Self::Json(_) => 60,
            Self::Io(_) | Self::Toml(_) => 70,
            Self::Config(_) => 71,
        }
    }

    /// Stable SCREAMING_SNAKE code — see docs/conventions.md § Error code catalog.
    pub fn error_code(&self) -> &'static str {
        match self {
            Self::MissingApiKey => "AUTH_MISSING",
            Self::InvalidApiKey => "AUTH_INVALID",
            Self::MissingScope { .. } => "AUTH_SCOPE",
            Self::NotFound(_) => "NOT_FOUND",
            Self::BadRequest(_) | Self::HogQL(_) => "BAD_REQUEST",
            Self::RateLimit { .. } => "RATE_LIMITED",
            Self::ServerError { .. } => "UPSTREAM",
            Self::Http(_) => "NETWORK",
            Self::Json(_) => "SCHEMA_DRIFT",
            Self::Io(_) | Self::Toml(_) => "INTERNAL",
            Self::Config(_) => "CONFIG",
        }
    }

    /// Retry-after seconds when applicable; `None` otherwise.
    pub fn retry_after_s(&self) -> Option<u64> {
        if let Self::RateLimit { retry_after_s, .. } = self {
            Some(*retry_after_s)
        } else {
            None
        }
    }
}
