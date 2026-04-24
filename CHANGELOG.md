# Changelog

All notable changes to BossHogg will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Calendar Versioning](https://calver.org/) —
`YYYY.MM.PATCH` — per the pattern established in our predecessor `lin` CLI.

The changelog uses dates in `YYYY-MM-DD` form. Each release also maps to a
crates.io publication and a GitHub Release with prebuilt tarballs.

## [Unreleased]

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
