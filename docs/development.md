# Development

How to build, test, and ship BossHogg.

## Prerequisites

- **Rust 1.95+** — pinned via `rust-toolchain.toml` at the repo root. Rustup auto-installs the pinned channel when you run any `cargo` command from the repo. If you have Homebrew's `rust` installed and it shadows rustup on PATH (common on macOS), prefix cargo commands with `PATH=~/.rustup/toolchains/1.95-aarch64-apple-darwin/bin:$PATH` or switch the PATH order so `~/.cargo/bin` wins.
- **macOS or Linux.** CI runs on `ubuntu-latest` + `macos-latest`.
- **A PostHog account** for live integration tests. Free cloud account works. Set `POSTHOG_CLI_TOKEN` in a local `.env` (gitignored) — see `.env.example`.

## Local setup

```bash
git clone <this repo>
cd bosshogg
cp .env.example .env       # fill in POSTHOG_CLI_TOKEN, POSTHOG_CLI_PROJECT_ID
cargo check
cargo build
./target/debug/bosshogg --help
```

Real PostHog credentials and project IDs belong in `.env` / `.env.local` (both gitignored), never in tracked files or commit messages.

## Testing

### The `test-harness` feature is required for most integration tests

Because v1.0 enforces `https_only(true)` on the production HTTP client, integration tests that spawn the binary against a wiremock `http://` mock need a feature-gated bypass. That bypass is the `test-harness` feature, gated by `cfg!(feature = "test-harness")` inside `src/client/mod.rs` and `src/commands/capture.rs`. Release builds (`cargo install bosshogg`, `cargo build --release` without the feature) **cannot** honor `BOSSHOGG_ALLOW_HTTP` even if the env var is set — the code path that reads it is compiled out.

**Always test with the feature:**

```bash
cargo test --features test-harness
```

Without the feature, subprocess-based integration tests that set `BOSSHOGG_ALLOW_HTTP=1` will fail because the spawned binary enforces HTTPS. This is intentional — the feature is the contract that "you're running tests, not a release."

### Unit tests

Next to the code. Standard Rust `#[cfg(test)] mod tests`. Cover argument parsing, output formatting, config migration, soft-delete route mapping.

```bash
cargo test --lib --features test-harness
```

### JSON contract tests

`tests/json_contract.rs` + `tests/schemas/*.schema.json`. Every `--json` output shape validates against a JSON Schema 2020-12 document via the `jsonschema` crate.

```bash
cargo test --test json_contract --features test-harness
```

When you add a resource, add a schema for its `--json` output shape.

### REST-shape tests

`tests/rest_shapes.rs` + `tests/fixtures/*.json`. Wiremock fixtures captured from real PostHog responses (sanitized). Verifies our typed structs deserialize cleanly.

Re-record a fixture when PostHog changes a response shape:

```bash
BOSSHOGG_RECORD_FIXTURES=1 cargo test --test rest_shapes --features test-harness -- --ignored record_
```

Requires `POSTHOG_CLI_TOKEN` set.

### Per-resource integration tests

Every resource has a `tests/<name>_cmd.rs` file with wiremock-backed tests covering its golden paths (list, get, create if applicable, update, delete, resource-specific verbs). These use either `Client::for_test` directly (fast) or `Command::cargo_bin("bosshogg")` for subprocess-level assertions.

```bash
cargo test --features test-harness
```

### Live integration tests

`tests/live.rs` + per-resource live tests marked `#[ignore]`. Run explicitly against a real PostHog project:

```bash
POSTHOG_CLI_TOKEN=phx_... POSTHOG_CLI_PROJECT_ID=999999 \
  cargo test --features test-harness -- --ignored
```

Exercises golden read paths against the dogfood project (999999 on US Cloud). Never writes — safe to run against production.

### HogQL smoke test

`tests/hogql_smoke.rs` — runs `bosshogg query run "SELECT 1"` end-to-end against the live API. `#[ignore]` by default.

```bash
cargo test --test hogql_smoke --features test-harness -- --ignored
```

## Linting and formatting

```bash
cargo fmt --all
cargo clippy --all-targets --features test-harness -- -D warnings
```

CI runs both with `-D warnings`. The pinned toolchain ensures consistent `rustfmt` output between dev and CI.

## Preflight

Before tagging a release:

```bash
./scripts/preflight.sh
```

Checks:

1. Working tree is clean.
2. `rust-toolchain.toml` is set.
3. `CHANGELOG.md` has an entry for the version in `Cargo.toml`.
4. `cargo fmt --all --check` passes.
5. `cargo clippy --all-targets --features test-harness -- -D warnings` passes.
6. `cargo test --features test-harness` passes.
7. `cargo build --release` succeeds.

Non-zero exit on any failure.

## CI

Under `.github/workflows/`:

- **`ci.yml`** — on push to main and PRs. Matrix: `ubuntu-latest` + `macos-latest`. Jobs: fmt check, clippy (`-D warnings`), test matrix (runs with `--features test-harness` so subprocess tests can use the HTTP bypass). Uses `Swatinem/rust-cache@v2`.
- **`audit.yml`** — weekly + on `Cargo.lock` changes. `rustsec/audit-check@v2`. Files issues, doesn't block builds.
- **`release.yml`** — triggered by `v*` tags. Cross-compiles 4 targets (linux x86/arm, macos x86/arm), attests provenance (`actions/attest-build-provenance@v2`), creates a GitHub Release, publishes to crates.io. Homebrew tap update is a commented-out step awaiting the tap repo + deploy key (see `packaging/homebrew/README.md`).

## Releasing

Versioning: **CalVer** `YYYY.MM.PATCH`. All v1 tags: `v2026.4.0` → `v2026.4.0`.

```bash
# 1. Bump version in Cargo.toml
# 2. Add CHANGELOG.md entry above [Unreleased]
git add Cargo.toml CHANGELOG.md
git commit -m "chore(release): bump to YYYY.MM.PATCH (description)"

# 3. Preflight must pass
./scripts/preflight.sh

# 4. Tag (annotated; sign with -s if GPG is set up)
git tag -a vYYYY.MM.PATCH -m "..."

# 5. Push (release.yml takes over)
git push origin main
git push origin vYYYY.MM.PATCH
```

`release.yml` handles cross-compile, crates.io publish, and GitHub Release creation. Homebrew formula update is a separate TODO (see `packaging/homebrew/`).

## Distribution channels

v1 shipping channels:

- **crates.io** — `cargo install bosshogg` (v1.0+)
- **GitHub Releases** — prebuilt tarballs for 4 targets (v1.0+)
- **Homebrew tap** — `brew install aaronkwhite/tap/bosshogg` (formula drafted; tap repo and deploy key pending — see `packaging/homebrew/README.md`)
- **Source** — `git clone && cargo install --path .` (always)

## Adding a new resource

The pattern is mechanical. `src/commands/flag.rs` is the CRUD-deep reference template; `src/commands/whoami.rs` is the minimal-read template.

Steps:

1. **Create `src/commands/<resource>.rs`** with:
   - Typed response struct (`#[derive(Deserialize, Serialize, Debug, Clone)]`). Use `serde_json::Value` for fluid fields (filters, properties, payloads).
   - `<Resource>Args` wrapper with `#[command(subcommand)]`.
   - `<Resource>Command` enum with one variant per subcommand.
   - `pub async fn execute(args: &<R>Args, json: bool, debug: bool, context: Option<&str>, yes: bool) -> anyhow::Result<()>` — takes all five globals; threading is mandatory.
   - One private `async fn <verb>_<resource>(client: &Client, ...) -> Result<()>` per subcommand.
   - Every destructive subcommand (`update`, `delete`, `enable`, `disable`, `archive`, anything that mutates) gates on `yes || !is_interactive() || confirm(...)`.
2. **Register the module** in `src/commands/mod.rs`: `pub mod <resource>;`.
3. **Add the CLI variant** in `src/cli.rs`: `<Resource>(<Resource>Args)`. For kebab-case CLI names (e.g., `event-definition`), add `#[command(name = "event-definition")]`.
4. **Dispatch in `src/main.rs::run()`**: `Some(Commands::<R>(args)) => commands::<r>::execute(&args, cli.json, cli.debug, cli.context.as_deref(), cli.yes).await?,`.
5. **If the resource soft-deletes** (returns 405 on hard DELETE), add its path segment to `SOFT_DELETE_RESOURCES` in `src/client/mod.rs` and write a unit test for `is_soft_delete_path`. `client.delete(path)` will route to `PATCH {deleted: true}` automatically.
6. **Write tests** at `tests/<resource>_cmd.rs`. Minimum: list, get, one mutation, one destructive-op-requires-yes. Use `Client::for_test` for in-process tests; use `Command::cargo_bin("bosshogg")` + `BOSSHOGG_ALLOW_HTTP=1` for subprocess tests.
7. **Add the resource to `research/capability-schema.yaml`** and `docs/capabilities.md`.
8. **Add a skill reference** at `.claude/skills/bosshogg/references/<resource>.md` with recipes for common workflows. Optional but encouraged for v1.1+ resources.
9. **Update `.claude/skills/bosshogg/references/mcp-gaps.md`** with a row for any new verb that has an MCP equivalent.
10. **Run** `./scripts/preflight.sh`. Must pass before committing.

Commit granularity: one resource per commit. Don't lump unrelated resources together — it makes `git log` and `git blame` noisy.

## Coding conventions

- **One file per noun, singular.** File names mirror CLI command names (and PostHog MCP tool taxonomy): `commands/flag.rs`, `commands/insight.rs`, `commands/hog_function.rs` (file uses snake_case, CLI uses kebab-case via `#[command(name = ...)]`).
- **No abstract command traits.** The `match` on `Commands` in `main.rs` is the only dispatch point. Keep it boring.
- **Typed structs for responses**, `serde_json::Value` escape hatches for notoriously fluid fields (`filters`, `properties`, `payload`, `tiles`, `questions`, `destination`).
- **Raw REST paths inline** with the code that calls them. No path constants module. Readers want to see the URL right next to the call.
- **Errors bubble**: command code returns `anyhow::Result<()>`; client and config modules return `Result<T, BosshoggError>`. Converts via `?` operator.
- **No comments that explain what the code does** — rely on naming. Comments only for non-obvious *why* (hidden constraints, workarounds).
- **Async everywhere.** `tokio` is the runtime. No blocking HTTP.
- **`--json` everywhere.** Every command that produces output respects `--json`. Default to JSON when stdout isn't a TTY. All JSON goes through `output::print_json()` — never call `serde_json::to_string` from command code.
- **`--yes` + confirm gate** on every destructive operation. Never silently mutate.
- **HogQL goes through `client.query()`**. No command builds a raw HogQL POST inline — use the wrapper for the auto-LIMIT guarantee.

## Decided — don't rehash

- **Use GraphQL / `cynic` like `lin` did.** No — PostHog is REST, and HogQL is string-query POSTs, not typed GraphQL.
- **Name the binary `hog`.** No — collides with PostHog's own `bin/hog` for the Hog language. See [`naming.md`](naming.md).
- **Fork `posthog-rs` instead of writing our own client.** No — the SDK's scope is ingestion + flag eval; ours is admin + query. Different surface, different auth, different lifetime assumptions.
- **Use OpenSSL.** No — `rustls-tls` for simple cross-compilation, matching `posthog-rs`.
- **Drop `https_only(true)` for test convenience.** No — the `test-harness` feature gates the bypass; release builds never honor it.

## Deferred to v1.1+

- Browser-based `bosshogg auth login`
- `bosshogg mcp --stdio` (MCP server mode)
- Persistent name→ID cache (XDG cache dir)
- Keyring integration for API keys
- Typed HogQL result rendering (dates, numeric alignment)
- `$EDITOR`-based hog-function source editing
