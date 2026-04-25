# Changelog

All notable changes to BossHogg will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Calendar Versioning](https://calver.org/) —
`YYYY.MM.PATCH` — per the pattern established in our predecessor `lin` CLI.

The changelog uses dates in `YYYY-MM-DD` form. Each release also maps to a
crates.io publication and a GitHub Release with prebuilt tarballs.

## [Unreleased]

## [2026.4.4] — 2026-04-25

Closes 2 of the 4 MCP-parity gaps from the v2026.4.2 coverage analysis, plus
adds 7 experiment lifecycle verbs PostHog exposes that bosshogg never wired.

### Added

- **`experiment launch / end / pause / resume / reset / ship-variant / recalculate-timeseries`** —
  full PostHog experiment lifecycle. `experiment archive` previously errored
  `"Experiment must be ended before it can be archived"` — now you can `end`
  via CLI. `ship-variant <id> --variant <key>` declares the winner; the
  variant's rollout goes to 100% on the underlying flag.
- **`experiment results <id> --metric-uuid <uuid>`** — fetches PostHog's
  timeseries-results aggregate for a specific metric. Closes the
  `experiment-results-get` MCP parity gap.
- **`query ai-costs [--since <Nd>]`** — convenience HogQL aggregate over
  `$ai_generation` events: per-model total cost (USD) and generation count
  for the time window (default 30d). Closes the
  `get-llm-total-costs-for-project` MCP parity gap.

### Notes

- The 2 remaining MCP gaps (`docs-search`, `query-generate-hogql-from-question`)
  are not closeable: `docs-search` isn't in PostHog's REST schema (MCP routes
  to a private Inkeep endpoint); `query-generate-hogql-from-question` returns
  HTTP 403 for personal API keys (was the same root cause that had us drop
  `query draft-sql` in v2026.4.3).

## [2026.4.3] — 2026-04-25

Right-sized surface: remove 5 more verbs confirmed session-only via live HTTP 403 dogfood. Same auth-boundary rationale as v2026.4.2 — ship only what works via Personal API Key. Resource count unchanged (24); verb count drops by 5.

### Removed

- **`event-definition tag --add` / `--remove` (2 verbs)** — PostHog's `/event_definitions/bulk_update_tags/` endpoint returns HTTP 403 (`"This action does not support Personal API Key access"`) for personal API keys. Session-cookie auth only.

- **`property-definition tag --add` / `--remove` (2 verbs)** — PostHog's `/property_definitions/bulk_update_tags/` endpoint returns HTTP 403 (`"This action does not support Personal API Key access"`) for personal API keys. Session-cookie auth only.

- **`query draft-sql` (1 verb)** — PostHog's `/query/draft_sql/` server-side NL→HogQL helper returns HTTP 403 (`"This action does not support Personal API Key access"`) for personal API keys. Session-cookie auth only.

## [2026.4.2] — 2026-04-24

Right-sized surface: remove verbs that cannot work via Personal API Key — the only auth mode a CLI user has. Live-dogfood of v2026.4.1 against a real PostHog account confirmed each returns a permanent HTTP 402 or 403 regardless of input.

### Removed

- **`subscription *` (7 verbs: `list`, `get`, `create`, `update`, `delete`, `test-delivery`, `deliveries`)** — PostHog returns HTTP 402 (`"This feature is not available on your current plan"`) for all subscription endpoints on non-Teams/Enterprise plans. The underlying PostHog subscription API exists but requires a paid Teams or Enterprise subscription — not reachable from a CLI personal API key on free or Growth plans. Users needing subscription management should use the PostHog web UI.

- **`dashboard tiles move`, `tiles copy`, `tiles reorder` (3 verbs)** — PostHog's dedicated `move_tile`, `copy_tile`, and `reorder_tiles` endpoints return HTTP 403 (`"This action does not support Personal API Key access"`) for personal API keys. These endpoints are session-cookie auth only. `tiles add` and `tiles remove` remain — both work correctly via `PATCH insight.dashboards`.

- **`event-definition metrics` (1 verb)** — PostHog's `/event_definitions/{id}/metrics/` endpoint returns HTTP 403 (`"This action does not support Personal API Key access"`) for personal API keys. Session-cookie auth only. Users needing event usage metrics should use the PostHog web UI (Data Management → Events → select event → Insights tab).

Net: 25 → 24 GA resources. The underlying PostHog endpoints still exist — these are auth-boundary removals only, not PostHog regressions.

## [2026.4.1] — 2026-04-24

Dogfood + UX cleanup. No resource changes; tightens the surface found during full-surface live-testing of v2026.4.0 against a real PostHog project.

### Added

- **`--limit <N>` on 9 more `list` subcommands.** `batch-export`, `cohort`, `dashboard`, `error-tracking/{fingerprints,assignment-rules,grouping-rules}`, `experiment`, `group`, `hog-function`, `person`, and `survey` now accept `--limit` for consistency with the rest of the CLI. Every `list` verb in the surface now takes `--limit`.

### Changed

- **`bosshogg event values --event <name>` is now required.** PostHog's `/events/values/` endpoint returns `HTTP 400: "The event_name parameter is required when using a personal API key"` if `event_name` is missing. Previously `--event` was optional in the CLI, so the command always failed with a backend error. Now clap enforces it up-front with a clear message.
- **Homebrew tap auto-update wired into the release workflow.** The previously commented-out `homebrew_tap` job is now active and uses a classic PAT (`HOMEBREW_TAP_TOKEN`) over HTTPS. On every tag push, `Formula/bosshogg.rb` in `aaronkwhite/homebrew-tap` is updated automatically with the new version and SHA256s computed from the release artifacts.

### Fixed

- **CI: `config::tests::config_path_under_home_xdg` failed on Linux** because the test helper set a fake `HOME` but `XDG_CONFIG_HOME` from the runner leaked into `dirs::config_dir()`. Helper now unsets `XDG_CONFIG_HOME` alongside `HOME`.
- **CI: `cargo-audit` failed with `not found: Couldn't load ./Cargo.lock`** because the lockfile was gitignored. For binary crates the lockfile belongs in VCS; removed the ignore rule and committed `Cargo.lock`.
- **Release workflow: cross-compile failed with `can't find crate for core`** on macOS targets because `dtolnay/rust-toolchain@stable` installed the stable channel and added the matrix target there, but `cargo build` then auto-switched to channel `1.95` from `rust-toolchain.toml` where the target was missing. Replaced with `rustup show active-toolchain` + `rustup target add`.
- **`cargo fmt --check` drift across 32 files** — single-line match arms, arg-list split/join differences. All cosmetic; clippy was already clean.
- **Docs overclaimed `person timeline` / `properties-timeline`** that the CLI doesn't expose. Corrected `README.md` and `docs/capabilities.md`. PostHog's underlying endpoint still exists; implementation is a v1.1+ candidate.
- **Docs overclaimed `dashboard snapshot`** that the CLI doesn't expose. Corrected `README.md`, `docs/capabilities.md`, `docs/v1-scope.md`, and the CHANGELOG.

## [2026.4.0] — 2026-04-24

**v1.0 — first public release. M1 through M9 complete. All 25 GA PostHog
resources covered.** The capstone of the BossHogg v1 roadmap: nine
development milestones shipped as a single initial public release.

### Development milestones

These were internal development milestones preceding the first public release. Published as a single `v2026.4.0` tag; the phased rollout lives here for reference.

| Milestone | Resources added |
|---|---|
| M1 — Core | `flag`, `query`, `configure`, `doctor`, `schema`, `auth`, `config` |
| M2 — Org & project | `org`, `project` |
| M3 — Analytics | `insight`, `dashboard`, `cohort` |
| M4 — People & events | `person`, `group`, `event`, `action`, `annotation` |
| M5 — Taxonomy | `event-definition`, `property-definition`, `endpoint` |
| M6 — Growth | `experiment`, `survey`, `early-access` |
| M7 — CDP pipeline | `hog-function`, `batch-export`, `subscription` |
| M8 — Ops & debug | `session-recording`, `error-tracking`, `role`, `capture` |
| M9 — Release polish | Homebrew tap, announcement README, ecosystem doc, shell completion, live-dogfood fixes |

Total: 25 GA resources, 415 tests (unit + wiremock + JSON contract + HogQL smoke).

### Added

- **`bosshogg completion <shell>`** — first-class shell completion subcommand (bash, zsh, fish, powershell, elvish). The Homebrew formula's `generate_completions_from_executable` now actually works.
- **`bosshogg insight create --query-file`** (and `insight update --query-file`) — supports PostHog's modern `query` schema (`InsightVizNode` / `TrendsQuery` / `FunnelsQuery` / `HogQLQuery`). Newer PostHog accounts reject legacy `filters` with HTTP 403; `--query-file` is the path forward. `--filters-file` remains for back-compat.
- **`bosshogg config set-context --project-token <phc_...>`** — previously only settable by editing TOML. `bosshogg configure` also prompts for it (optional) after the personal key is validated.
- **`bosshogg cohort get <name>`** — accepts names via list+filter fallback (mirrors `project get`). Previously only numeric ids parsed.
- **Five new cross-product playbooks** in `.claude/skills/bosshogg/references/cross-product-playbooks.md`:
  - Why did conversion drop? (web analytics → funnel → cohort → replays → error tracking)
  - Ship a tracking event (naming → SDK capture → taxonomy → insight → dashboard → cohort → CDP)
  - Debug an LLM app (HogQL on `$ai_generation`/`$ai_trace` → trace timeline → exception → replay → prompt version → eval)
  - Incident notebook (error fingerprints → deployment annotation → 3 replays → log snippets → postmortem → share)
  - GDPR deletion (person list → hard delete → activity log verify → cohort purge → flag evaluation verify)
- **Homebrew tap formula** at `packaging/homebrew/bosshogg.rb` — multi-platform Ruby formula for `aaronkwhite/homebrew-tap`. SHA256 placeholders filled by the release workflow.
- **Announcement-quality README** — Quickstart with `bh` shortcut hint, "Why BossHogg over PostHog MCP?" comparison table, full 25-resource feature list by milestone, Safety section.
- **Ecosystem integration doc** at `docs/ecosystem-integration.md` — how BossHogg relates to `@posthog/cli`, `posthog-rs`, PostHog MCP, PostHog Wizard.
- **`cargo t` alias** via `.cargo/config.toml` — short form for `cargo test --features test-harness`. Local runs used to need the feature flag every invocation because `BOSSHOGG_ALLOW_HTTP` is compiled out without it.
- **`--limit <N>` on four list commands.** `bosshogg event-definition list`, `property-definition list`, `action list`, and `annotation list` now accept `--limit` for consistency with `insight list` / `flag list`. Without it they continue to fetch all pages.
- **Community hygiene files** for the OSS release: `CONTRIBUTING.md` (add-a-resource pattern + PR conventions), `SECURITY.md` (reporting channel + in/out-of-scope), `CODE_OF_CONDUCT.md` (Contributor Covenant v2.1), `.github/ISSUE_TEMPLATE/bug_report.yml` + `feature_request.yml`, `.github/pull_request_template.md`, and a trademark disclaimer at the foot of `README.md`.

### Changed

- **Brand rename: `BossHog` → `BossHogg`.** The binary, crate, skill directory, Homebrew formula, Rust type `BosshoggError`, env vars `BOSSHOGG_*`, config dir `~/.config/bosshogg/`, and repository URL `aaronkwhite/bosshogg-cli` all moved from single-g to double-g. The Dukes-of-Hazzard-spelling pun now lands as intended.
- **Structural simplification refactor (PRs 1–6):**
  - Test layout consolidated — `tests/` used to be 56 separate integration-test binaries, now 3 (`cli.rs`, `contracts.rs`, `live.rs`). `cargo test --no-run` time: ~27s → ~21s.
  - `TestHarness` (`tests/common/harness.rs`) replaces ~20 LOC of per-test MockServer/TempDir/config.toml/Command scaffolding. Total test LOC ~11.2k → ~10k with identical coverage.
  - Shared command helpers extracted to `src/commands/util.rs` (`read_json_file`, `read_text_file`, `env_id_required`, `gate_destructive`). Removed ~248 LOC of duplicated code.
  - `CommandContext` (`src/commands/context.rs`) unifies handler signatures — one value carrying `Client`, `json_mode`, and `yes`. Collapsed ~200 handler signatures and 8 inline `confirm()` sites; migrated 27 command modules.
  - `tokio` narrowed from `features = ["full"]` to `rt-multi-thread`, `rt`, `macros`, `time`, `fs`. `chrono` dropped default `oldtime`/`wasmbind` in favor of explicit `clock` + `std` + `serde`.
- **`CONFIG` error code (new, exit 71).** Config-missing errors (missing `POSTHOG_CLI_ENV_ID` / `POSTHOG_CLI_ORG_ID`) were mis-classified as `INTERNAL`. `Io`/`Toml` (truly internal) stay on `INTERNAL` / exit 70. `CONFIG` row added to the error table in `SKILL.md` and `docs/conventions.md`.
- **`anyhow` dependency removed** — all command error returns use `BosshoggError` directly.
- **`WhoamiArgs` empty struct dropped**; `Commands::Whoami` is now a unit variant.
- **`bosshogg capture` inline reqwest client** now enforces `https_only(true)` and sends `User-Agent: bosshogg/<version>` to match the main client.

### Fixed

**Round 1 — live dogfood against PostHog project 999999**

- **HogQL `async_` field rejected by PostHog.** Strict pydantic mode on `/query/` rejected the legacy alias. Renamed request-body field to `async` in `src/client/query.rs`. Had blocked all HogQL.
- **HogQL response `types` schema drift.** PostHog returns per-column `[name, ch_type]` pairs; `QueryResponse.types` was typed as `Vec<String>`. Relaxed to `Vec<Value>` (still accepts the legacy bare-string shape). Fixture and JSON schema updated.

**Round 2 — live dogfood against real Lin project**

- **`insight create --filters-file` returned HTTP 403** on modern PostHog accounts (`"Creating or updating insights with legacy filters is not available for this user"`). Fixed via new `--query-file` flag accepting modern `query` schema.
- **`dashboard tiles add` was a silent no-op** — `PATCH /dashboards/{id}/ {"tiles":[...]}` returns 200 but PostHog silently drops the field on current accounts. Rewrote to PATCH `insight.dashboards` (modern path). `tiles remove` rewritten symmetrically. Both verbs now work end-to-end.
- **`is_short_id` matched numeric IDs of 6–8 digits**, misrouting `insight get/update/delete <numeric-id>` as short_id lookups that returned `NOT_FOUND`. Added letter guard: a short_id must contain at least one letter. Numeric-only strings route as numeric IDs. Fixes the same class of issue for every resource that resolves short_ids.
- **`dashboard tiles move / copy / reorder` had the same silent-drop bug** as `tiles add` originally did. Retargeted to PostHog's dedicated `/move_tile/`, `/copy_tile/`, `/reorder_tiles/` endpoints. These endpoints currently require session-cookie auth and return clean `HTTP 403: This action does not support Personal API Key access` for personal API keys — better than silent success-lies. If PostHog relaxes this restriction, bosshogg works automatically.

**Opus review — C1–C9, I1–I9**

- **C1** — Homebrew formula install path corrected; URL now embeds `#{version}` in the tarball filename to match CI artifact names.
- **C2** — `skills/bosshogg/scripts/doctor.sh` parses the real `{checks, summary}` JSON shape.
- **C3** — Five of seven cross-product playbooks had shell-invalid commands; every `bosshogg` invocation verified against `--help`.
- **C4** — `evals/evals.json` eval-07 aligned with cohort-name resolution.
- **I1** — `capture --debug` actually emits tracing (was silently dropped).
- **I2** — Dashboard tile mutations rewritten as GET-merge-PATCH (interim; superseded by the modern-endpoint fixes in round 2).
- **I3** — `--yes` help text broadened from "flag-only" wording to cover all 22 resources' destructive ops.
- **I4** — README `cohort history` → `cohort calculation-history` (matches the real CLI surface).
- **I5** — Dead `notebooks` entry removed from `SOFT_DELETE_RESOURCES`.
- **I6** — `dashboard tiles add/copy`, `dashboard snapshot` gated on `--yes`/confirm for symmetry.
- **I7** — `doctor --json` exits 0 on check failure so agents can parse `summary.ok` without stderr-vs-stdout juggling. Genuinely-fatal errors (binary missing, unreadable config) still return non-zero.
- **I9** — `query cancel` documented as intentionally ungated.

### Known limitations

- Browser-based `bosshogg auth login` — deferred to v1.1.
- `bosshogg mcp --stdio` — v1.1.
- Persistent name→ID cache (XDG cache dir) — v1.x; M1 is per-process only.
- Keyring integration — v1.x; plaintext TOML at `0600` + `.env` for v1.
- Typed HogQL result rendering (dates-as-dates, etc.) — v1.x polish.
- **Dashboard tile move / copy / reorder via Personal API Key** — PostHog-imposed; the dedicated endpoints require session-cookie auth. Commands return a clean 403 with the PostHog message. `tiles add` and `tiles remove` work fully via `insight.dashboards`.

[Unreleased]: https://github.com/aaronkwhite/bosshogg-cli/compare/v2026.4.0...HEAD
[2026.4.0]: https://github.com/aaronkwhite/bosshogg-cli/releases/tag/v2026.4.0
