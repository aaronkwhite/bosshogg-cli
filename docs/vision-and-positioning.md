# Vision & positioning

## TL;DR

BossHogg is an agent-first PostHog CLI that complements — does not replace — PostHog's official tooling. Its core argument is that a ships-with-skill CLI costs ~500x less idle context than the official MCP server while covering a broader operational surface than the official `@posthog/cli`.

## The problem

A developer or coding agent working with PostHog has three first-party options and none of them covers the middle.

| Tool | What it covers | What it misses |
|---|---|---|
| `@posthog/cli` (official Rust) | `login`, `query` (HogQL), sourcemap/dSYM/ProGuard uploads, experimental `endpoints` | feature flag management, insights, dashboards, persons, cohorts, multi-profile config, consistent `--json` |
| `posthog-rs` (official SDK) | event capture, remote + local feature flag evaluation | admin API (projects, flags CRUD, insights, etc.); not a CLI at all |
| PostHog MCP server | 200+ tools across 28 feature groups, full web-UI parity, chart rendering, Max AI hooks | idle cost ≈ 44k tokens (independent benchmarks show 28% MCP failure rate vs 100% CLI success); schema drift; not scriptable outside an agent loop; EU requires self-host |
| PostHog `ai-plugin` / `skills` | 17 instrumentation skills, 27 slash commands | wraps the MCP server, so pays the 70k tax; skills are instrumentation-flavored, not operational |

Result: the engineer who wants to *operate on a running PostHog project from a shell* — toggle a flag in CI, run a HogQL query, check a cohort, debug an insight — has no well-shaped tool. And the agent that wants to do the same thing either loads 70k tokens of MCP definitions or fails over to shelling out to an incomplete official CLI.

## The thesis

Two bets:

1. **CLIs are better agent surfaces than MCP servers for read/query/simple-write workflows.** A CLI that ships with a Claude Code skill exposes ~200 tokens of frontmatter idle, body loaded on demand. The same capability surface as an MCP server, at <0.5% the idle cost. Benchmarks: PostHog MCP is ~44k tokens idle; Scalekit's gh-vs-GitHub-MCP test showed 32x token gap (1,365 vs 44,026 tokens on identical tasks) and 28% MCP failure rate vs 100% CLI success. Cursor hard-caps at 40 MCP tools; GitHub Copilot at 128. See [`agent-first.md`](agent-first.md) for the token math and the "when to use MCP instead" decision tree.
2. **The operational admin/query surface is wider than any existing PostHog tool covers.** `@posthog/cli` stops at sourcemaps and HogQL. The SDK stops at capture and flag eval. The MCP covers everything but at a prohibitive context cost. A dedicated CLI closes both gaps.

## What BossHogg is

- A **single-binary Rust CLI** wrapping the PostHog REST + Query API.
- **Agent-native by default:** `--json` on every subcommand, stable JSON schemas, structured errors, deterministic exit codes, `--help` is the source of truth for flags.
- **Multi-profile, multi-region:** US, EU, and self-hosted in one `~/.config/bosshogg/config.toml`.
- **HogQL is first class:** `bosshogg query` is the central command; `bosshogg events`, `bosshogg flags evaluate`, etc. all route through the Query API where it makes sense.
- **Ships a Claude Code skill** (`.claude/skills/bosshogg/`) from the first commit. Future clients (Cursor, Codex, Gemini, Windsurf, …) get the same skill via the skills spec.
- **Explicitly complementary** to `@posthog/cli` (reuses its env var names for CI drop-in) and `posthog-rs` (sends data *out*; SDK sends data *in*).

## What BossHogg is not

- Not a replacement for `@posthog/cli`. Source-map uploads, dSYM/ProGuard symbolication, and release pipelines stay with the official tool. BossHogg will never fight for that surface.
- Not an SDK. Embedding event capture in a Rust application is `posthog-rs`'s job. BossHogg exposes `bosshogg capture` for debugging only — not for production ingestion.
- Not a UI. Chart rendering, dashboard browsing, and wizard flows belong in the PostHog web app or the MCP server. BossHogg returns URLs; it doesn't try to paint them.
- Not a new MCP server. We may ship a `bosshogg mcp` mode later (same binary, stdio transport, same auth) as an option — but the primary interface stays the CLI.

## Target users

1. **Senior engineers operating PostHog in CI and terminals.** They want `bosshogg flags rollout my-flag --percent 10 --cohort beta` and `bosshogg query "SELECT ..." --json | jq` without context-switching.
2. **Coding agents (Claude Code first).** They want a predictable, low-idle-cost tool that returns structured JSON, exposes its schema, and doesn't eat 70k tokens before the first prompt.
3. **Multi-workspace teams spanning US and EU PostHog Cloud.** They want named profiles instead of `POSTHOG_CLI_HOST` juggling.

## Positioning rules

- Always describe BossHogg as **complementary**, never as a *replacement* or *alternative to* official PostHog tooling.
- When referencing the official CLI, name it: `@posthog/cli` or the crate `posthog-cli`.
- When referencing the official SDK, name it: `posthog-rs`.
- When referencing the MCP, name it: *PostHog MCP server*.
- Avoid the phrase *"better than"*. Use *"different focus"*, *"covers the gap"*, *"complementary surface"*.
- Attribute any HogQL recipes or skill scaffolding borrowed from `PostHog/ai-plugin` or `PostHog/skills`.
- Never squat `posthog-*` names on crates.io. Trademark friction is not worth saving.

## SEO targeting

We don't try to outrank `@posthog/cli` for `posthog cli` — they own the top five slots and should.

Long-tail targets instead:

- `posthog agent cli`
- `posthog cli rust`
- `posthog cli json`
- `posthog cli for claude code`
- `posthog feature flags cli`
- `posthog hogql cli`
- `posthog cli scriptable`
- `posthog cli multi project`
- `posthog cli eu region`

These get baked into:

- crates.io crate description (indexed for search)
- GitHub repository description
- GitHub topic tags: `posthog`, `posthog-cli`, `cli`, `rust`, `agent`, `claude-code`, `ai-agents`, `feature-flags`, `hogql`, `product-analytics`, `developer-tools`
- README opening paragraph

## Non-goals for v1

See [`v1-scope.md`](v1-scope.md) for the detailed skip list with rationale. Headline: notebooks (beta/unstable), data warehouse external sources (beta/churny), legacy plugins (deprecated), full user admin (UI concern), SSE dashboard streaming (wrong transport for a CLI), rrweb payload downloads (not an agent workflow).

## Success criteria

We'll call v1 a success when:

1. A Claude Code session with the `bosshogg` skill active uses fewer than 300 idle tokens to expose the full capability surface.
2. A new user can go from `brew install` to a working `bosshogg query "SELECT ..."` in under five minutes.
3. A CI pipeline can do `bosshogg flags list --json | jq '...'` without any env var juggling that wouldn't already work for `@posthog/cli`.
4. Feature-flag management works end to end: list, create, toggle, rollout percentage, cohort assignment, JSON payload editing, dependent-flag inspection.
5. HogQL queries, saved endpoints, and the `events`/`trends`/`funnel` wrappers all share the same JSON output contract.

Explicit non-criteria: we don't need to match MCP's tool count, we don't need chart rendering, we don't need OAuth in v1 (personal API keys are fine).
