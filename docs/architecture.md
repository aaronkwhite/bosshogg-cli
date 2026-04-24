# Architecture

Technical design for BossHogg. Adapted from the [`lin` CLI playbook](../../Documents/building-a-rust-cli-for-a-product-api.md) (Rust CLI for Linear's GraphQL API), modified for PostHog's REST-plus-HogQL surface.

## Guiding principles

1. **One file per noun, singular.** `src/commands/flag.rs`, `src/commands/insight.rs`, `src/commands/cohort.rs`, etc. File names mirror the CLI command name, which mirrors the PostHog MCP tool taxonomy (MCP `feature-flag-create` → `bosshogg flag create`). Each file is self-contained: Clap args, subcommand dispatch, and HTTP calls live side by side. No abstract trait tree, no plugin registry.
2. **REST with typed responses, JSON-value escape hatches.** Unlike `lin` (which used `serde_json::Value` everywhere because Linear's GraphQL surface is huge), PostHog's REST resources are small enough to type. Use `#[derive(Deserialize)]` structs; fall back to `HashMap<String, Value>` for notoriously fluid fields (filters, properties, payloads).
3. **HogQL queries are inline strings passed through `serde_json::json!`.** No cynic or typed GraphQL infrastructure — this isn't GraphQL.
4. **Output is always structured.** Every command has `--json`. Default when stdout isn't a TTY.
5. **Auth resolution is deterministic and documented.** See below.
6. **Everything is async, runs on tokio.** Consistent with `posthog-rs`'s default feature and the `lin` playbook.

## Project layout

```
src/
  main.rs              # Parse CLI, dispatch to command, handle errors
  cli.rs               # Clap derive: Cli struct + Commands enum
  error.rs             # BosshoggError via thiserror
  client/
    mod.rs             # HTTP client: auth, retry, debug, host resolution,
                       #   soft-delete route table, error mapping to
                       #   BosshoggError variants.
    query.rs           # POST /query/ + async polling + auto-LIMIT 100;
                       #   `Client::query(sql, kind, is_async) -> QueryResponse`
                       #   where QueryResponse { results, columns, types,
                       #   hogql, timings }.
    cache.rs           # In-memory name→ID resolution (flags-by-key,
                       #   projects). Per-process; no disk layer in v1.
  commands/
    mod.rs             # pub mod for each resource (singular)
    query.rs           # `bosshogg query …`
    flag.rs            # `bosshogg flag …`     (M1 CRUD-deep)
    insight.rs         # M3
    dashboard.rs       # M3
    cohort.rs          # M3
    person.rs          # M4
    event.rs           # M4
    action.rs          # M4
    annotation.rs      # M4
    experiment.rs      # M6
    survey.rs          # M6
    session_recording.rs   # M8 (kebab in CLI: `session-recording`)
    hog_function.rs    # M7 (kebab in CLI: `hog-function`)
    batch_export.rs    # M7 (kebab in CLI: `batch-export`)
    subscription.rs    # M7
    group.rs           # M4
    role.rs            # M8
    early_access.rs    # M6 (kebab in CLI: `early-access`)
    error_tracking.rs  # M8 (kebab in CLI: `error-tracking`)
    endpoint.rs        # M5
    capture.rs         # M8
    event_definition.rs    # M5 (kebab: `event-definition`)
    property_definition.rs # M5 (kebab: `property-definition`)
    org.rs             # M2
    project.rs         # M2
    # M1 meta commands
    whoami.rs          # `bosshogg whoami`
    doctor.rs          # `bosshogg doctor` — preflight for the skill
    schema.rs          # `bosshogg schema hogql` — HogQL grounding schema
    auth.rs            # `bosshogg auth token` escape-hatch
    config.rs          # `bosshogg config set-context/get-contexts/...`
    use_cmd.rs         # `bosshogg use <name>` shortcut
    configure.rs       # `bosshogg configure` first-run wizard
  output/
    mod.rs              # print_json()
    detail.rs           # Human-readable detail views (per resource)
    table.rs            # comfy-table wrapper
    color.rs            # NO_COLOR-aware, TTY-aware helpers
    interactive.rs      # Fuzzy-select prompts, confirmations
    safe.rs             # Blob-safety rules — replay never-stdout,
                        #   LLM-trace summarize-default, size guards.
                        #   Day-one enforcement even before M8.
  config.rs             # TOML, multi-profile, ~/.config/bosshogg/config.toml
  util.rs               # UUID / short_id detection, date parsing
tests/
  query_validation.rs   # Ping the API; verify REST shapes match our structs
  json_contract.rs      # Every command's --json output validates against schema
.claude/
  skills/
    bosshogg/
      SKILL.md          # ~150 tokens frontmatter; body fetched on demand
      references/
        commands.md
        mcp-gaps.md
        hogql-recipes.md
        auth.md
docs/                   # Design + reference docs (this folder)
research/               # Kickoff research artifacts
```

## Command module shape

Every `src/commands/<resource>.rs` follows the same template:

```rust
use clap::{Args, Subcommand};
use serde::{Deserialize, Serialize};
use crate::client::Client;

#[derive(Args, Debug)]
pub struct FlagArgs {
    #[command(subcommand)]
    pub command: FlagCommand,
}

#[derive(Subcommand, Debug)]
pub enum FlagCommand {
    /// List feature flags
    List { /* args */ },
    /// Get a flag by key or id
    Get { identifier: String },
    /// Create a flag (accepts --filters-file)
    Create { /* args */ },
    /// Update a flag (by key)
    Update { /* args */ },
    // ...
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Flag {
    pub id: i64,
    pub key: String,
    pub name: Option<String>,
    pub active: bool,
    pub filters: serde_json::Value,  // fluid — leave as Value
    // ...
}

pub async fn execute(
    args: &FlagArgs,
    json: bool,
    debug: bool,
    context: Option<&str>,
) -> anyhow::Result<()> {
    let client = Client::new(context, debug)?;
    match &args.command {
        FlagCommand::List { /* … */ } => { /* … */ }
        // …
    }
    Ok(())
}
```

Dispatch in `main.rs`:

```rust
match cli.command {
    Commands::Flag(args) => commands::flag::execute(
        &args, cli.json, cli.debug, cli.context.as_deref(),
    ).await?,
    // …
}
```

Adding a new resource = new file + new enum variant + new dispatch arm. Mechanical.

## HTTP client

`client/mod.rs` exposes a small surface. REST and HogQL go through it.

```rust
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

impl Client {
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, BosshoggError>;
    pub async fn get_paginated<T: DeserializeOwned>(&self, path: &str, limit: Option<usize>) -> Result<Vec<T>, BosshoggError>;
    pub async fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T, BosshoggError>;
    pub async fn patch<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<T, BosshoggError>;
    pub async fn delete(&self, path: &str) -> Result<(), BosshoggError>;

    // Specialized HogQL wrapper in client/query.rs
    pub async fn query(&self, sql: &str, kind: QueryKind, is_async: bool) -> Result<QueryResponse, BosshoggError>;
}
```

Behaviors baked in:
- **rustls-tls** (not OpenSSL) for simple cross-compilation, matching `posthog-rs`.
- **`.https_only(true)`** per the `lin` security review; prevents redirect-to-HTTP.
- **Retry with exponential backoff** on 429 and 503 (1s, 2s, 4s; max 3 attempts). Honors `Retry-After` header when present.
- **Debug mode** prints the final URL, headers (redacted auth), body, and response to stderr.
- **Error-body truncation to 200 chars** in logs to avoid leaking tokens or PII.
- **Soft-delete normalization.** `client.delete(path)` checks a static list of soft-delete resources and internally routes to `PATCH {deleted: true}` when needed. Callers don't care.

## Auth resolution chain

Order matters, and we've stolen it directly from the `lin` playbook's post-review ordering:

1. **`--api-key <key>`** — highest priority, for one-off invocations and tests.
2. **`--context <name>`** (alias `--profile` kept for @posthog/cli habit) — named context from config; its `api_key` field.
3. **`POSTHOG_CLI_TOKEN`** / **`POSTHOG_CLI_API_KEY`** / **`POSTHOG_API_KEY`** env var — for CI, scripts, and agents. We reuse `@posthog/cli`'s variable names for drop-in CI compat; fall back to `POSTHOG_API_KEY`.
4. **Current context** (`current_context` field) in `~/.config/bosshogg/config.toml`.
5. **`.env` / `.env.local`** in the current directory. (Do NOT look at `current_exe().parent()` — see the `lin` security review findings for why.)

Host resolution chain, similar:

1. `--host <url>`
2. Profile `host`
3. `POSTHOG_CLI_HOST` / `POSTHOG_HOST` env var
4. Region-based default (`us.posthog.com` or `eu.posthog.com`)
5. Hard default: `https://us.posthog.com`

## Config file

```toml
# ~/.config/bosshogg/config.toml
# File perms enforced to 0600 on save.
current_context = "prod-us"

[contexts.prod-us]
host = "https://us.posthog.com"
api_key = "phx_..."
project_id = "999999"
env_id = "999999"
org_id = "0192f000-..."

[contexts.prod-eu]
host = "https://eu.posthog.com"
api_key = "phx_..."
project_id = "..."
env_id = "..."
```

Legacy-migration pattern lifted from `lin`: if an older `default_profile` / `[profiles.*]` shape shows up on load, migrate it to `current_context` / `[contexts.*]` silently on next save, keying off the old `default_profile` value. Plan for config evolution from day one.

## Error handling

```rust
// src/error.rs
use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]                 // borrowed from posthog-rs — grow without SemVer breaks
pub enum BosshoggError {
    #[error("missing API key (set POSTHOG_CLI_TOKEN, run `bosshogg configure`, or pass --api-key)")]
    MissingApiKey,

    #[error("API key rejected by PostHog — check the key and its scopes")]
    InvalidApiKey,

    #[error("missing scope `{scope}` on the active API key — re-issue with this scope")]
    MissingScope { scope: String },

    #[error("HTTP error: {status} — {message}")]
    ServerError { status: u16, message: String },

    #[error("rate limited by PostHog — retry after {retry_after_s}s (bucket: {bucket})")]
    RateLimit { retry_after_s: u64, bucket: String },

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("resource not found: {0}")]
    NotFound(String),

    #[error("HogQL error: {0}")]
    HogQL(String),

    #[error("config error: {0}")]
    Config(String),

    #[error(transparent)]
    Http(#[from] reqwest::Error),  // keep #[source] chain, don't stringify

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}
```

Command functions return `anyhow::Result<()>`; they call client methods that return `Result<T, BosshoggError>`. `main.rs` prints the error chain on non-zero exit.

## Output system

Three surfaces, all in `src/output/`:

- **JSON** — `print_json<T: Serialize>(value: &T)`. Compact (no pretty-printing) for ~30% token savings for agents. All JSON goes through this one function — never call `serde_json::to_string` directly from a command.
- **Table** — `print_table(rows, columns)` via `comfy-table`. Used for list views.
- **Detail** — per-resource detail renderers in `output/detail.rs`. Human-friendly `get` output.

Color rules (copied from `lin`):

- Respect `NO_COLOR` env var.
- Disable color when stdout isn't a TTY.
- Helpers: `bold()`, `dim()`, `green()`, `yellow()`, `red()`, `cyan()`.

Interactive rules:

- `is_interactive()` = `Term::stdout().is_term()`.
- Never prompt in non-interactive mode.
- Destructive actions auto-confirm when piped.
- Fuzzy-select pickers only fire when a flag that could take a value is omitted AND we're in a TTY.

## Caching

Users pass flag keys (`my-beta-flag`), insight short-IDs (`abc123`), cohort names. The API usually needs numeric IDs.

```rust
pub struct Cache {
    pub flags_by_key: OnceCell<HashMap<String, i64>>,
    pub insights_by_short_id: OnceCell<HashMap<String, i64>>,
    pub cohorts_by_name: OnceCell<HashMap<String, i64>>,
    pub projects: OnceCell<Vec<Project>>,
}
```

Fetched lazily per-process, invalidated at process exit. The cache is a request-lifetime convenience; it's not a persistent disk cache. Users who want resolution between invocations get that via named profiles.

## Query polling

Long-running HogQL queries go through the async pathway. Our `Client::query` wrapper handles the dance:

1. POST `/query/` with `async_: true`.
2. Poll GET `/query/:id/` with backoff (500 ms, 1 s, 2 s, 4 s, capped at 10 s) until `status` moves past `running`.
3. Honor `--timeout` (default 60 s); cancel via DELETE `/query/:id/` on `Ctrl-C` or timeout.
4. Return the final `results` block.

## Dependencies

Same baseline as the `lin` playbook, with PostHog-specific additions:

| Crate | Purpose |
|---|---|
| `clap` (derive) | CLI parsing |
| `clap_complete` | Shell completions |
| `reqwest` (rustls-tls, json, gzip) | HTTP client |
| `tokio` (full) | Async runtime |
| `serde` + `serde_json` | Serialization |
| `thiserror` | Error types |
| `anyhow` | Error propagation in command code |
| `comfy-table` | Tables |
| `console` | Terminal detection + styling |
| `dialoguer` (fuzzy-select) | Interactive prompts |
| `dirs` | `~/.config/bosshogg/` |
| `termimad` | Markdown rendering (insight/dashboard descriptions) |
| `toml` | Config file |
| `dashmap` | Concurrent name-ID maps where needed |
| `tracing` + `tracing-subscriber` | Debug/trace output — match posthog-rs and modern Rust norms |
| `temp-env` (dev) | Env var isolation in tests |
| `wiremock` (dev) | HTTP mocks for integration tests |
| `assert_cmd` + `predicates` (dev) | Binary tests |

Explicitly **not** using: `cynic` (not GraphQL), `openssl` (rustls), `dotenvy` (small inline parser is fine), `colored` (console does both color and TTY).

## Testing strategy

- **Unit tests** next to the code they test (standard Rust).
- **JSON contract tests** (`tests/json_contract.rs`): every command's `--json` output parses against a checked-in schema. Prevents accidental envelope changes.
- **REST-shape tests** (`tests/rest_shapes.rs`): run against a recorded set of `wiremock` fixtures for each resource. Catches struct drift vs API responses.
- **Live integration tests** (`tests/live.rs`, all `#[ignore]`): run only when `POSTHOG_CLI_TOKEN` is present; exercise the golden read path against project 999999 on US Cloud.
- **HogQL smoke test** (`tests/hogql_smoke.rs`): verifies `bosshogg query run "SELECT 1"` end-to-end.
- **Fixture recording**: `BOSSHOGG_RECORD_FIXTURES=1 cargo test --test rest_shapes -- --ignored record_` re-records sanitized responses into `tests/fixtures/` from the live API.

## CI / release plan

Three workflows, mirroring the `lin` playbook:

- **`ci.yml`** — ubuntu + macos matrix, fmt + clippy (`-D warnings`) + test + build. Toolchain from `rust-toolchain.toml` (single source of truth).
- **`audit.yml`** — weekly + on `Cargo.lock` changes. `rustsec/audit-check`. Files issues, doesn't block builds.
- **`release.yml`** — triggered by `v*` tags. Cross-compiles four targets, attests provenance, publishes to crates.io, updates Homebrew formula in a tap repo.

`scripts/preflight.sh` — clean tree check, toolchain check, CHANGELOG entry check, fmt + clippy + test + release build. Run before tagging.

Versioning: CalVer `YYYY.MM.PATCH` (e.g., `2026.4.0`). Matches the `lin` convention; crates.io accepts it.

## Things deliberately copied from `lin`

- One file per noun.
- Output before commands (build output helpers first so the first command has `print_table()` / `print_json()` available).
- Security posture: `.https_only(true)`; no `current_exe().parent()` in `.env` search; truncate error bodies in logs.
- Toolchain pinning via `rust-toolchain.toml` from the first commit.
- Single release workflow, four targets, four distribution channels.

## Things deliberately changed from `lin`

- **REST, not GraphQL.** No `cynic` schema snapshot; no query validation tests. Struct drift is caught via JSON contract tests instead.
- **Typed responses, not `Value` soup.** PostHog has a finite, knowable resource catalog.
- **Multi-profile config from day one.** `lin` learned this mid-project; we start there.
- **Ships-with-skill from the first commit.** `lin` added a Claude Code skill late. BossHogg's skill is part of v1 scope.
- **No need for cynic / query validation tests.** Replaced by `rest_shapes.rs` + `json_contract.rs`.

Resolved via the v2026.4.0 design spec:

1. **Browser-based login?** Deferred to v1.1. `bosshogg configure` + paste-a-key is v1.
2. **`bosshogg mcp --stdio` mode?** Stretch at M9 (v1.0); likely slips to v1.1.
3. **Persistent vs per-process cache?** Per-process for v1; XDG disk cache deferred.
4. **Keyring integration?** Deferred to v1.x. Plaintext TOML at `0600` + `.env` for v1.
5. **Typed HogQL result rendering?** v1 stringifies; typed column rendering is v1.x polish.
