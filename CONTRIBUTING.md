# Contributing to BossHogg

Single-maintainer project. PRs are welcome for the scope listed below. Read this before opening one.

## Local setup

```bash
git clone https://github.com/aaronkwhite/bosshogg-cli
cd bosshogg-cli
cp .env.example .env.local          # fill in POSTHOG_CLI_TOKEN + POSTHOG_CLI_PROJECT_ID
cargo t                             # alias: cargo test --features test-harness
```

`--features test-harness` is required for integration tests. Without it, subprocess tests
that hit wiremock over `http://` fail — that's intentional. See `CLAUDE.md` and
`docs/development.md` for full setup details.

Preflight before committing:

```bash
./scripts/preflight.sh              # fmt, clippy -D warnings, tests, release build
```

## Adding a new PostHog resource

The pattern is mechanical. Reference templates:

- **`src/commands/flag.rs`** — CRUD-deep: list, get, create, update, delete, enable/disable,
  rollout, filters. The template for anything with mutations.
- **`src/commands/whoami.rs`** — minimal read-only command. Start here for read-only resources.

Steps:

1. **`src/commands/<noun>.rs`** — typed response structs, `<Noun>Args` + `<Noun>Command`
   enum, `pub async fn execute(args: &<N>Args, cx: &CommandContext) -> Result<()>`.
   Every destructive subcommand gates on `cx.confirm()`. See `docs/development.md` for
   the full signature and soft-delete routing rules.
2. **`src/commands/mod.rs`** — add `pub mod <noun>;`.
3. **`src/cli.rs`** — add `<Noun>(<NounArgs>)` variant to `Commands`. Use
   `#[command(name = "kebab-name")]` when the CLI name differs from the Rust identifier.
4. **`src/main.rs`** — add dispatch arm in `run()`.
5. **`tests/schemas/<noun>-list.schema.json`** (and `<noun>-get.schema.json`, etc.) —
   JSON Schema 2020-12 for every `--json` output shape. See `tests/contracts.rs`.
6. **`tests/<noun>_cmd.rs`** — wiremock-backed tests: minimum list, get, one mutation,
   destructive-op-requires-yes. Add the module to `tests/cli.rs`.
7. Update `docs/capabilities.md` and `research/capability-schema.yaml`.
8. Run `./scripts/preflight.sh`. Must pass.

Commit granularity: one resource per commit.

## Bug fix flow

1. Write a failing test that reproduces the bug (unit test or wiremock integration test).
2. Fix the bug.
3. Confirm `cargo t` passes.
4. Add a CHANGELOG entry above `[Unreleased]` (see format below).
5. Submit PR.

## PR conventions

**Tests.** New code needs test coverage. Wiremock integration tests live in `tests/cli.rs`
(as modules) and `tests/<resource>_cmd.rs`. JSON output changes need an updated schema in
`tests/schemas/`.

**Clippy.** `cargo clippy --all-targets --features test-harness -- -D warnings` must be
clean. No `#[allow(clippy::...)]` suppressions without justification.

**Formatting.** `cargo fmt --all`. Enforced in CI.

**JSON contract discipline.** `--json` output is the public API. Additive changes (new
fields) are fine. Renaming, removing, or retyping a field is breaking and requires a
discussion before the PR is opened.

**CHANGELOG entry.** Every user-visible change gets an entry in `CHANGELOG.md` above the
`[Unreleased]` heading. Format: `- <verb> <description> (#<pr>)`.

**Commit messages.** Conventional-commits style, imperative:

```
feat(flag): add --tag filter to flag list
fix(client): retry on 503 before surfacing UPSTREAM error
docs(contributing): clarify soft-delete routing steps
chore(release): bump to 2026.5.0
```

**Placeholder IDs in fixtures and tests.** Never commit real project IDs, org IDs, UUIDs,
or API keys. Use `999999` for project/env IDs, `00000000-0000-0000-0000-000000000000` for
UUIDs, `a@b.com` for emails. Real credentials stay in `.env.local` (gitignored).

## Scope

**Welcome:**

- Bug fixes with reproducers
- New PostHog GA resources following the pattern above
- Doc improvements (`docs/`, `CLAUDE.md`, skill references)
- Test coverage for untested paths

**Out of scope:**

- Support for analytics vendors other than PostHog
- Cosmetic refactors (renaming things without behavioral change)
- Dependency upgrades without a concrete fix or security reason
- Features deferred to v1.1+ in `docs/development.md` (open an issue first)

## Review bar

PRs get a thorough technical review — correctness, error handling, JSON contract
stability, test coverage, and fit with the architecture. Expect multiple rounds of
feedback; that's normal. The maintainer follows the conventions documented in `CLAUDE.md`.

For questions before writing code, open an issue or email aaron@aaronkwhite.com.
