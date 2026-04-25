# CLAUDE.md

Maintainer notes for building and maintaining bosshogg. Read this before making changes.

> **Audience.** This file is for me (the maintainer) and any coding agent helping me *build* bosshogg. For agents *using* bosshogg to operate PostHog, point them at `.claude/skills/bosshogg/` instead — that's the operating manual. Don't re-derive the CLI surface from source; use the skill.

## Commands

```bash
cargo t                                              # alias: cargo test --features test-harness
cargo build --release                                # release binary to target/release/bosshogg
cargo clippy --all-targets --features test-harness -- -D warnings
cargo fmt --all
./scripts/preflight.sh                               # gate before any tag push
```

`--features test-harness` is required for anything that spawns the binary. Without it, `BOSSHOGG_ALLOW_HTTP` is compiled out and subprocess tests that hit wiremock over `http://` will fail. Release builds always enforce `https_only(true)`.

## What this is

A PostHog CLI covering all 25 GA resources (flags, HogQL, insights, dashboards, cohorts, persons, events, experiments, surveys, session recordings, error tracking, hog functions, batch exports, subscriptions, roles, …). Agent-first: JSON-first output with stable schemas, structured errors, deterministic exit codes. Ships a Claude Code skill so models can operate it at ~200 idle tokens instead of ~44k for the MCP server.

## Architecture map

```
src/
  main.rs            parse CLI, dispatch to command, handle errors
  cli.rs             clap Cli + Commands
  error.rs           BosshoggError via thiserror; Result alias
  client/            HTTP client (auth, retry, HTTPS, error mapping), query, cache
  commands/<noun>.rs one file per PostHog resource — args, dispatch, HTTP calls in one place
  commands/util.rs   shared: read_json_file, env_id_required, gate_destructive
  commands/context.rs CommandContext — Client + json_mode + yes, threaded through handlers
  config.rs          TOML at ~/.config/bosshogg/config.toml; multi-context
  output/            print_json, table, detail views, interactive prompts, blob safety
  analytics.rs       anonymous self-tracking telemetry (queue-then-flush to PostHog /batch/, `DO_NOT_TRACK` honored)
tests/
  cli.rs             consolidated integration binary (all wiremock-backed per-command tests)
  contracts.rs       JSON-schema validation of every --json output
  live.rs            live-API tests; #[ignore] unless BOSSHOGG_LIVE=1
.claude/skills/bosshogg/  operating manual for agents using the CLI
docs/                design + reference docs (read in reading-order in docs/README.md)
research/            kickoff research; authoritative on PostHog API + naming
packaging/homebrew/  bosshogg.rb + tap docs
```

Adding a new PostHog resource = new file in `src/commands/`, new variant in `src/cli.rs`, new arm in `main.rs`, new schema in `tests/schemas/`, new test module in `tests/cli.rs`. Mechanical.

## The JSON contract is the API

For an agent-operated CLI, the `--json` output *is* the public API. Treat it accordingly.

- Every command with `--json` has a schema at `tests/schemas/<command>.schema.json` validated in `tests/contracts.rs`.
- Changes must be **additive** — adding fields is safe; renaming, removing, or retyping is breaking and needs a major CalVer bump.
- Error envelope is `{error: true, code, message, hint?, retry_with?}` — see `src/error.rs` and `docs/conventions.md`.
- Exit codes: `10` auth, `20` not-found, `30` bad-request, `40` rate-limit, `50` upstream, `60` schema-drift, `70` internal, `71` config.

## Rust conventions

- `thiserror` for `BosshoggError`. Every failure in library code returns `Result<T, BosshoggError>`. No `anyhow` in `src/` — only in binaries/tests.
- No `unwrap()` in `src/`. `expect("specific reason")` is acceptable at known-safe points; prefer `.ok_or_else(|| BosshoggError::…)`.
- Handler signature: `pub async fn execute(args: FooArgs, cx: &CommandContext) -> Result<()>`. The `CommandContext` carries `Client`, `json_mode`, and `yes`; don't add new params to handler functions — extend `CommandContext`.
- Destructive ops (`update`/`delete`/`enable`/`disable`/`archive`) gate on `gate_destructive(yes, prompt)` via `cx.confirm()`.
- `clippy --all-targets -- -D warnings` gates CI.

## Testing

- Unit tests live next to the code, gated by `#[cfg(test)]`.
- All subprocess-based integration tests live in `tests/cli.rs` as modules (`tests/cli.rs` is the single binary target).
- JSON contracts live in `tests/contracts.rs` — every `--json` shape validates against a JSON Schema 2020-12 document. Adding a resource = adding a schema.
- Wiremock fixtures in `tests/fixtures/` are sanitized PostHog responses. Re-record via `BOSSHOGG_RECORD_FIXTURES=1 cargo test --features test-harness --test rest_shapes -- --ignored record_`.
- Live tests are `#[ignore]`-gated and only run with `BOSSHOGG_LIVE=1`. They exercise read paths against a real project; never write.

## Contribution hygiene

- **Use placeholders in tracked files.** `999999` for project/env IDs, `00000000-0000-0000-0000-000000000000` for UUIDs, `a@b.com` for emails. Never copy a live identifier into a fixture, doc, test, or commit message. Real auth lives in `.env.local` (gitignored).
- **Keep the skill in sync with the CLI surface.** `.claude/skills/bosshogg/` references, playbooks, and evals must match what `bosshogg --help` actually exposes. After adding or renaming any flag, grep the skill.
- **JSON output changes are additive only.** The `--json` shape is the public agent API. Renaming, removing, or retyping a field is breaking and requires a major CalVer bump.
- **CalVer discipline.** `YYYY.MM.PATCH` — today's month, not next month. Bump `Cargo.toml` + `packaging/homebrew/bosshogg.rb` + `.claude/skills/bosshogg/.claude-plugin/marketplace.json` together and add a CHANGELOG entry.
- **Internal process stays local.** `docs/superpowers/` is gitignored for plans, dogfood runs, session handoffs. Never commit under it; never link to it from tracked docs.
- **Preflight before tagging.** `./scripts/preflight.sh` is the gate (fmt, clippy `-D warnings`, tests, release build). No exceptions.

## Where to look for more

- `docs/architecture.md` — system design, HTTP client, auth resolution
- `docs/conventions.md` — JSON contract, error envelope, exit codes, env vars
- `docs/api-notes.md` — non-obvious PostHog API quirks
- `docs/capabilities.md` — full resource catalog
- `CHANGELOG.md` — what changed, when, why
- `.claude/skills/bosshogg/SKILL.md` — skill entry point

## Before committing

1. `./scripts/preflight.sh` passes (fmt, clippy -D warnings, tests, release build).
2. If you added a resource or changed a `--json` shape: schema updated in `tests/schemas/`, contract test added/updated.
3. If user-visible: CHANGELOG entry above the next CalVer heading.
4. Version bump lives in three files (see Gotchas). Use the same `YYYY.M.PATCH` in all three.
