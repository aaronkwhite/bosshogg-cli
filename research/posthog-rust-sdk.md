# PostHog Rust SDK — research for `bosshogg` / `hog` CLI

Research date: 2026-04-21. Purpose: understand the official PostHog Rust SDK so
the `bosshogg` crate / `hog` binary can (a) avoid collisions, (b) complement
rather than compete, and (c) reuse any battle-tested patterns for auth, errors,
and serialization.

---

## 1. SDK identification

| Fact | Value |
|---|---|
| Crate name | **`posthog-rs`** |
| Latest version | **0.5.2** (Cargo.toml); README still references 0.3.7; crates.io doc page references 0.5.1 |
| Official? | **Yes** — Cargo.toml description: *"The official Rust client for Posthog"* |
| Repo | https://github.com/PostHog/posthog-rs (under the `PostHog` GitHub org) |
| Docs | https://docs.rs/posthog-rs |
| PostHog docs page | https://posthog.com/docs/libraries/rust |
| License | MIT |
| Edition / MSRV | 2018 / rustc 1.78.0 |
| Maintainers (crate owners) | `timgl`, `oliverb123`, `Piccirello`, `rafaeelaudibert` (PostHog staff) |
| Activity | ~187 commits on main, 8 releases, 50 stars, 6 open issues. Active, not archived. No "beta" / "experimental" warning in README. |

### The other `posthog` name

- `crates.io/crates/posthog` — **does not exist** on crates.io (docs.rs returned
  404 for the bare `posthog` crate; the name appears to be unclaimed or squatted
  without a release). The only real crate in this namespace is `posthog-rs`.
- This means the `bosshogg` crate name we've picked avoids collisions entirely;
  it is not "posthog-cli", "posthog-rs-cli", or similar, and there is no
  official CLI in the PostHog org today.

### No official CLI

PostHog does not publish a first-party CLI. The ecosystem gap that `hog` fills
is real — every other PostHog surface (JS, Python, Go, etc.) is SDK-only.

---

## 2. What the SDK actually does

From README + docs.rs + `examples/`:

**Core surface (ingestion + flags only):**

- `posthog_rs::client(api_key)` — builds a `Client` (blocking or async).
- `Client::capture(Event)` / `capture_batch(Vec<Event>)` — POSTs to the
  ingestion endpoint (`US_INGESTION_ENDPOINT` / `EU_INGESTION_ENDPOINT`
  constants; self-hosted URL configurable via `ClientOptions`).
- `Client::is_feature_enabled(key, distinct_id, groups, person_props, group_props)`
- `Client::get_feature_flag(...)` → `Option<FlagValue::{Boolean,String}>`
- `Client::get_feature_flags(...)` → bulk flag + payload map
- `LocalEvaluator` — polls flag definitions and evaluates in-process
  ("100–1000× faster"). Requires a **personal API key** (project key alone
  isn't sufficient for local evaluation).
- Global singleton: `init_global()` + free `capture()` / `match_feature_flag()`.

**Authentication pattern:**

- Project API key for capture + remote flag evaluation.
- Personal API key **additionally** for local flag evaluation (needed to pull
  flag definitions). This is the same split we'll need in the CLI — the admin
  API requires the personal key too.
- Configuration goes through `ClientOptions` / `ClientOptionsBuilder`
  (`derive_builder`).

**What's NOT in the SDK:**

- No `/api/projects`, `/api/insights`, `/api/cohorts`, `/api/dashboards`,
  `/api/annotations`, `/api/persons` (beyond capture-side identify),
  `/api/event_definition`, `/api/property_definition`, `/api/users/@me`,
  session-recording endpoints, HogQL query endpoint, or any CRUD/admin surface.
- No OAuth, no org switching, no workspace management.
- No CLI, no TTY output, no config file, no keychain.

---

## 3. Scope comparison table

| Area | `posthog-rs` SDK | `bosshogg` / `hog` CLI |
|---|---|---|
| Event capture (`/i/v0/e`, `/capture`) | **Yes** — primary use case | Probably no (or thin: `hog capture` for debugging only) |
| Batch capture | Yes (`capture_batch`) | Probably no |
| Remote feature-flag eval (`/flags`, `/decide`) | Yes | Maybe (`hog flags check <key> --user <id>`) |
| Local feature-flag eval | Yes (`LocalEvaluator` with polling) | No — not a CLI concern |
| Flag **admin** (create / edit / archive flags) | **No** | **Yes** (`hog flags list/create/update`) |
| Insights CRUD | No | Yes |
| Cohorts | No | Yes |
| Dashboards | No | Yes |
| Annotations | No | Yes |
| Persons / event-definitions / property-definitions | No | Yes (read-mostly) |
| HogQL / `query` endpoint | No | Yes (core differentiator — `hog query "select ..."`) |
| Session recordings | No | Yes (list / fetch metadata) |
| Projects / orgs / users / API-token management | No | Yes |
| Personal-API-key auth | Partial (only for local eval) | **Primary auth** |
| Tokio required | Optional (`async-client` default feature) | Yes (tokio 1, like `lin`) |
| HTTP layer | `reqwest 0.13` (rustls, blocking, json, gzip) | `reqwest` (same) — independent client |
| TLS | rustls (pinned in `reqwest` features) | Use rustls too for consistency |

**Overlap:** essentially none on the *admin/read* surface the CLI targets. The
only functional overlap is that both can capture events and check flag values —
and for the CLI that's a "nice to have for debugging", not the raison d'être.

---

## 4. Patterns worth adopting

### 4.1 Error enum shape (verbatim from `src/error.rs`)

```rust
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Connection(String),
    Serialization(String),
    AlreadyInitialized,
    NotInitialized,
    InvalidTimestamp(String),
    InconclusiveMatch(String),
    RateLimit,
    BadRequest(String),
    ServerError { status: u16, message: String },
}
```

Takeaways:

- **`#[non_exhaustive]`** on the error — we should do the same so our `Error`
  can grow without SemVer breaks.
- **Status-aware variants** (`RateLimit`, `BadRequest`, `ServerError { status, message }`)
  — mirrors what a CLI needs for user-friendly exit codes and messages. The SDK
  has a `Error::from_http_response()` constructor that maps HTTP status → variant;
  we should have the equivalent in our `http` module.
- **No `thiserror`.** They hand-implement `Display` + `std::error::Error`. The
  `lin` playbook uses `thiserror`; stick with that — it's strictly better for
  maintenance and the SDK's choice is idiosyncratic, not exemplary.
- **String-wrapping inner errors** (`Connection(String)` rather than
  `Connection(#[from] reqwest::Error)`) is a questionable choice — it loses
  source chain. **Don't copy this.** Keep `#[from] reqwest::Error` / `#[from] serde_json::Error`
  with `#[source]` preserved.

### 4.2 Auth / options pattern

- `ClientOptionsBuilder` via `derive_builder` → clean config surface. For the
  CLI, config comes from file + env + flags, so we don't need a builder per
  request, but the idea of a single `ClientOptions { api_key, host, timeout,
  region }` struct threaded through the HTTP layer is sound.
- **Region awareness** (`US_INGESTION_ENDPOINT`, `EU_INGESTION_ENDPOINT`
  constants) — we need this. PostHog Cloud US vs EU vs self-hosted is a
  first-class concern; bake `--region us|eu` or `host` URL into config on day 1.

### 4.3 Observability

- Uses `tracing` (not `log`). Respects `RUST_LOG=posthog_rs=debug`. We should
  use `tracing` + `tracing-subscriber` with an env filter keyed on `bosshogg=…`
  — consistent with both the SDK and modern Rust norms.

### 4.4 Docs / README style

- README leads with a one-line install, then a 6-line "capture an event"
  example, then progressively deeper sections: feature flags → properties →
  groups → all-flags → error matching → observability.
- **Adopt this shape** for our own README: install → "list your projects in 5
  lines" → progressively cover resources.
- Their error-handling section uses a `match` block that handles
  `Err(Error::Connection(..))` and `Err(Error::InconclusiveMatch(..))`
  explicitly — a good template for our `hog flags check` error table.

### 4.5 Serialization

- Plain `serde` + `serde_json`. `Event` has a typed struct with
  `insert_prop(&str, T)` helpers (wraps into a `HashMap<String, Value>`).
- For the CLI's admin surface we'll have many more typed resources (insight,
  cohort, dashboard…). Follow the SDK's convention: typed top-level struct,
  `HashMap<String, serde_json::Value>` escape hatch for `properties` /
  `filters` where PostHog's schema is notoriously fluid.

---

## 5. Patterns to skip

- **String-wrapped errors** — see 4.1. We want `#[from]` chain preservation.
- **Global singleton (`init_global`, free-standing `capture()`)** — pointless
  in a CLI where `main` owns the client lifetime.
- **`LocalEvaluator` flag-polling loop** — a CLI command is a one-shot; no
  long-lived polling task. If we ever add `hog flags check`, hit `/decide`
  (or `/flags`) remotely and exit.
- **`derive_builder` for options** — overkill for CLI-internal config; clap +
  a plain struct is cleaner.
- **Both async *and* blocking clients.** SDK supports both because it's
  embedded in varied apps. CLI is tokio-only; mirror `lin`.
- **Edition 2018 / MSRV 1.78** — we're a greenfield 2024/2021 crate, pick a
  newer edition and don't inherit their constraints.

---

## 6. Positioning implications

When someone Googles "posthog rust" or searches crates.io for `posthog`, they
should find:

1. **`posthog-rs`** — embed analytics / flags in your Rust *application*.
2. **`bosshogg` / `hog`** — operate your PostHog *project* from the terminal or
   an agent.

Recommended positioning language for our README / crate description:

> `bosshogg` is a command-line client and Rust library for the PostHog admin
> and query API. It is complementary to
> [`posthog-rs`](https://crates.io/crates/posthog-rs), the official SDK for
> capturing events and evaluating feature flags from inside your application.
> Use `posthog-rs` to *send data in*; use `hog` to *manage, query, and export
> data out*.

Concrete dos and don'ts:

- **Do** link to `posthog-rs` from our README ("if you want to capture events
  from a Rust app, use the official SDK").
- **Do** pick a crate description that includes the word "CLI" so crates.io
  search results disambiguate at a glance.
- **Don't** name the crate `posthog-cli` — that invites a future PostHog-owned
  crate to collide, and `bosshogg` is more memorable.
- **Don't** duplicate their capture surface beyond a minimal `hog capture`
  debug command. If users want real ingestion, send them to the SDK.
- **Do** document in FAQ: "Why not just extend `posthog-rs`?" Answer: different
  surface (admin vs ingestion), different auth (personal vs project key),
  different runtime assumptions (one-shot CLI vs embedded).

---

## 7. Other PostHog Rust code worth knowing

- **No significant community forks or alternative crates.** crates.io returned
  no scrapeable search results in this pass, but `posthog-rs` is the only
  PostHog-labeled crate in docs.rs that resolved; `posthog` (bare name) 404s.
- The PostHog monorepo (`PostHog/posthog`) contains *internal* Rust services
  (event ingestion pipeline, capture-rs, plugin-server bits) — these are not
  published as libraries. Not relevant to us except as a style reference for
  PostHog's idiomatic Rust.
- No community "posthog-admin" or "posthog-query" crate exists at time of
  research — **the admin-API niche is open**.

---

## 8. Quick-reference sources

- SDK repo: https://github.com/PostHog/posthog-rs
- SDK docs: https://docs.rs/posthog-rs
- SDK Cargo.toml: https://github.com/PostHog/posthog-rs/blob/main/Cargo.toml
- SDK error.rs: https://github.com/PostHog/posthog-rs/blob/main/src/error.rs
- SDK examples: https://github.com/PostHog/posthog-rs/tree/main/examples
  (`feature_flags.rs`, `local_evaluation.rs`, `advanced_config.rs`)
- PostHog Rust docs page: https://posthog.com/docs/libraries/rust
- PostHog API overview: https://posthog.com/docs/api
