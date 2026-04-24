# PostHog + AI Agents: Landscape Review

_Research conducted 2026-04-21 for `bosshogg` / `hog` CLI design._

## TL;DR

- PostHog ships a **mature official MCP server** (`mcp.posthog.com/mcp`, 100+ tools, OAuth + personal-key auth) and an **official Claude Code / Cursor / Codex / Gemini plugin** (`PostHog/ai-plugin`) that bundles 27+ slash commands and 17 skill directories (`exploring-autocapture-events`, `instrument-feature-flags`, `skills-store`, etc.).
- There is also `PostHog/skills` (marked "under construction") which is the canonical Claude Code plugin distribution — auto-generated from `PostHog/context-mill`.
- The main **agent pain point** is exactly the thesis of `bosshogg`: MCP tool-count bloat. Community reports the PostHog MCP ships **~70k tokens of tool definitions** idle. Cursor hard-caps at 40 MCP tools; GitHub Copilot at 128. A community "code-mode" alternative (`metta-ai/posthog-code-mode-mcp`) collapses the same surface into ~2k tokens by exposing a single typed client instead of N tools.
- There is already a minimal `posthog-cli` SKILL.md in `ComposioHQ/awesome-agent-clis` — 17 lines, essentially a stub. Nobody has shipped a serious CLI-first agent surface for PostHog yet. This is the gap `bosshogg` fills.
- PostHog's own Rust CLI (`PostHog/posthog` → `/cli`) is narrowly scoped to **source maps, dSYMs, ProGuard, releases, and HogQL query**. It does NOT cover feature-flag toggling, insights, persons, cohorts, or dashboards. Naming collision risk is real; we should make sure `bosshogg` differentiates clearly (e.g. crate name `bosshogg`, binary `hog`, positioning as "agent-first").

---

## 1. Official PostHog MCP Server

- **Repo:** [`PostHog/mcp`](https://github.com/PostHog/mcp) — archived Jan 19 2026, moved into the PostHog monorepo.
- **Endpoint:** `https://mcp.posthog.com/mcp` (US), `https://mcp-eu.posthog.com/mcp` (EU). Self-host documented but requires running the TypeScript server locally.
- **Auth:** OAuth (primary, auto-handled by supported clients) or Personal API Key via `Authorization: Bearer phx_…`. Keys have a "MCP Server" preset that scopes to a single project.
- **Install:** `npx @posthog/wizard mcp add` — supports Claude Code, Claude Desktop, Cursor, VS Code, Zed, Codex, Windsurf, Lovable, Replit, v0.
- **Tool count:** PostHog docs say **100+ tools**; OpenTools registry says **27**; `PostHog/ai-plugin` says **27+**. Discrepancy is almost certainly "27 top-level capability groups, 100+ underlying endpoints." Categories: feature flags, dashboards, experiments, cohorts, surveys, error tracking, workflows, SQL/HogQL, session replays, CDP functions, LLM analytics, prompts, actions, insights.
- **Stars / traction:** 143 stars, 25 forks (before archival).
- **Max AI:** PostHog has an in-product AI assistant called **Max** (separate from MCP) that lives in the PostHog web UI. Not directly relevant to coding-agent workflows but overlaps conceptually.

### Context cost

A widely-cited community number: **PostHog MCP ships ~70k tokens of tool definitions idle**. This validates the bosshogg thesis verbatim. The `metta-ai/posthog-code-mode-mcp` fork reduces the same surface to ~2k tokens by moving from "N individual MCP tools" to "one `execute_code` tool with a typed client."

### What works well

- **OAuth out of the box** — no API-key-in-env fiddling for first-run.
- **HogQL via MCP** — the `query` tool is genuinely useful; agents can write HogQL directly.
- **Broad surface coverage** — flags, experiments, cohorts, insights, error tracking, surveys, CDP, LLM analytics, prompts all addressable.
- **First-party install wizard** — `npx @posthog/wizard mcp add` is frictionless.
- **EU region support** at the server level.

### What sucks

- **Idle token cost (~70k).** Dominant complaint. Pushes out real context before you prompt.
- **Schema validation bugs** (open issue: "Invalid schema in some tools"). Tool call inputs fail with unhelpful errors.
- **No screenshot/chart rendering** for insights — agents can't "see" visualizations.
- **Can't edit feature-flag JSON payloads** (open issue).
- **No insight filtering when querying.**
- **API key permission changes don't reload tools** — must restart the server.
- **Windows performance issues** (open issue).
- **EU users can't use the hosted US endpoint** — have to self-host or use `cduguet/posthog-eu-mcp` (0 stars, unmaintained).
- **Error messages are terse** — agents struggle to self-correct tool call failures.

---

## 2. Official PostHog plugins / skill collections

### `PostHog/ai-plugin` (active, successor to `posthog-for-claude`)

[Repo.](https://github.com/PostHog/ai-plugin) Official. 46 commits. Supports Claude Code, Cursor, Codex, Gemini CLI. Installs via `claude plugin install posthog`.

**Slash commands (16):** `flags`, `insights`, `errors`, `experiments`, `dashboards`, `surveys`, `query`, `logs`, `llm-analytics`, `docs`, `actions`, `search`, `workspace`, plus three `llma-cc-*` commands for LLM-analytics code ingestion.

**Skills (17):** `analyzing-experiment-session-replays`, `auditing-experiments-flags`, `cleaning-up-stale-feature-flags`, `exploring-autocapture-events`, `exploring-llm-clusters`, `exploring-llm-evaluations`, `exploring-llm-traces`, `instrument-error-tracking`, `instrument-feature-flags`, `instrument-integration`, `instrument-llm-analytics`, `instrument-logs`, `instrument-product-analytics`, `managing-subscriptions`, `query-examples`, `signals`, `skills-store`.

Auth: OAuth (browser). Wraps the MCP server — not a standalone CLI. So it still carries the MCP token cost, it just adds curated workflows on top.

### `PostHog/skills` (under construction)

[Repo.](https://github.com/PostHog/skills) Official, 22 stars, 69 commits, 3 open PRs. Structured as:

```
skills/
  posthog/         # auto-generated from context-mill + monorepo
    all/
    cutting-costs/
    error-tracking/
    feature-flags/
    integration/
    llm-analytics/
    logs/
    migrations/
    product-analytics/
    tools-and-features/
  omnibus/
  community/
  team/
  .meta/
```

Distributed as a Claude Code plugin (installable marketplace or copy-to-`.claude/skills/`). Skills are the "instrumentation" flavor — teaching agents how to add PostHog to a codebase, not how to operate on a running PostHog instance.

### `PostHog/context-mill`

[Repo.](https://github.com/PostHog/context-mill) The build pipeline that turns PostHog docs + curated prompts + example apps into Agent-Skills-spec-compliant zip packages with `manifest.json`. Source of truth for both MCP tool prompts and the `PostHog/skills` repo.

### `PostHog/posthog-for-claude` (archived March 5, 2026)

Superseded by `ai-plugin`. 13 stars. Historical only.

### `ComposioHQ/awesome-agent-clis/posthog-cli/SKILL.md`

The only existing CLI-shaped PostHog skill. 17 lines of content, essentially a stub:

```yaml
---
name: "PostHog CLI"
description: "Query events, manage feature flags, and pull analytics data..."
---
```

Lists three example commands (`posthog events query`, `posthog feature-flags list`, `posthog feature-flags toggle`). No recipes, no `--json` convention, no error-handling guidance. **Prior-art-shaped but not actually useful.** We should do much better.

---

## 3. Community MCP servers / integrations

- **`metta-ai/posthog-code-mode-mcp`** — ⭐ interesting. Collapses PostHog MCP into a single "code mode" tool (~2k tokens vs ~70k). Same philosophical argument as bosshogg but still using MCP as the transport. Not well-known.
- **`cduguet/posthog-eu-mcp`** — 0 stars, MIT, TypeScript fork for EU. Essentially dead.
- **`heygen-com/posthog-mcp`** — Company fork, presumably internal use. No public traction.
- **Smithery / PulseMCP / OpenTools registry listings** — all wrap the official MCP. No independent builds.
- **Activepieces / Composio / n8n** — all surface PostHog through their generic MCP brokers; not CLI-shaped.

No one has shipped a serious community CLI for PostHog. The space is wide open.

---

## 4. Official PostHog CLI (existing, Rust, narrow scope)

Lives at [`PostHog/posthog/cli`](https://github.com/PostHog/posthog/tree/master/cli). Written in Rust. Published as `@posthog/cli` on npm and via shell installer. Commands:

- `login` — interactive OAuth / API-key auth
- `query` — HogQL SQL over CLI
- `sourcemap` — upload source maps for error tracking
- `exp` — experimental endpoint / task operations
- Symbol uploads: dSYM, Hermes, ProGuard

**Does NOT cover:** flag toggling, insights browsing, person inspection, cohort management, dashboards, experiments, surveys, session replays.

**Naming collision risk.** The existing binary is `posthog-cli` / `@posthog/cli`, we're shipping `hog`. Differentiation angle: "agent-first; broad operational surface; not source-maps-and-releases."

---

## 5. What sucks / what works — consolidated

### Works well (adopt)

- OAuth first, API-key fallback (match this)
- HogQL query as first-class (the one MCP tool nobody complains about)
- Scoped "MCP Server" key preset — limits blast radius (we need analogous `hog auth login --scope=…`)
- `context-mill`-style auto-generation from docs (consider pulling their manifests for recipe content)
- Progressive disclosure of skills (description visible, body fetched on demand)
- Good HogQL example library in `PostHog/ai-plugin/skills/query-examples`

### Sucks (fix or avoid)

- Huge idle token footprint — **core bosshogg value prop**
- No screenshots / chart rendering — we probably also can't solve this, but should return URLs to PostHog UI
- Opaque tool errors — `hog` should return structured `{error, hint, retry_with}` JSON
- Tool schema drift vs runtime — tests and `--help` should be source of truth
- EU support as afterthought — bosshogg should read `POSTHOG_HOST` cleanly from day one
- Flag JSON payload editing missing — `hog flags update --payload-file` should work
- No persons / cohort inspection from CLI at all — big gap

---

## 6. Proposed `.claude/skills/bosshogg/SKILL.md` outline

```yaml
---
name: bosshogg
description: Operate on PostHog from the CLI — query events, manage feature flags,
  inspect persons and cohorts, debug insights. Use when an agent needs to read or
  modify PostHog state without loading a 70k-token MCP server.
---
```

### Proposed sections (headings + one-line descriptions)

1. **When to use `hog`** — one-line decision rule: CLI for operational reads/writes and HogQL; skip MCP unless you need UI-rendered visualizations.
2. **Setup & auth** — `hog auth login` (OAuth) or `POSTHOG_API_KEY` + `POSTHOG_PROJECT_ID` env; never echo keys; respect `POSTHOG_HOST` for EU/self-hosted.
3. **Golden rules** — always pass `--json`; verify unknown flags with `hog <cmd> --help` before composing; never inline API keys in shell commands; treat non-zero exit + stderr JSON as authoritative.
4. **Core workflows** — short inline recipes for the 80% case:
   - _Query events_ — `hog events query --event X --days 7 --json`
   - _HogQL_ — `hog query 'SELECT …' --json`
   - _Read insight_ — `hog insights get <id> --json`
   - _List / toggle flag_ — `hog flags list`, `hog flags update <key> --enabled`
   - _Inspect person_ — `hog persons get <distinct_id> --json`
   - _Inspect cohort_ — `hog cohorts get <id> --json`
5. **Recipe: debug a flag rollout** — grab flag definition → query evaluation events → compute exposure split → list persons in each variant.
6. **Recipe: verify event instrumentation** — list recent events for a distinct_id → check property schema → diff expected vs observed.
7. **Recipe: insight triage** — list dashboards → fetch insight → run its HogQL → return a terse summary.
8. **Recipe: cohort inspection** — resolve cohort ID → list members → sanity-check size.
9. **Recipe: experiment audit** — list running experiments → check exposure + primary metric → flag anomalies.
10. **When to reach for MCP instead** — one paragraph: chart rendering, `create experiment` wizard, anything interactive. Reference `references/mcp-gaps.md`.
11. **Error handling** — `hog` returns `{error, code, hint, retry_with}`; on `AUTH_EXPIRED` run `hog auth login`; on `RATE_LIMITED` back off; on schema errors re-read `--help`.
12. **JSON output contract** — stable field names, paginated results use `{data: […], next_cursor}`, timestamps are RFC3339.
13. **Anti-patterns** — don't pipe `hog` into `jq` when `--json --field-select` exists; don't loop over `hog events query` for every user; don't write flag keys with hyphens-and-underscores ambiguity.
14. **References** — points at `references/commands.md` (exhaustive subcommand reference) and `references/mcp-gaps.md` (decision matrix: hog vs MCP per task).

### Supporting files to ship alongside

- `references/commands.md` — auto-generated from clap help; one section per subcommand with exit codes and JSON schema.
- `references/mcp-gaps.md` — decision matrix + list of tasks that still require MCP (chart rendering, Max AI, etc.).
- `references/hogql-recipes.md` — crib from `PostHog/ai-plugin/skills/query-examples` (attribution).
- `references/auth.md` — scope recommendations, key rotation, EU/self-host config.

### Target frontmatter cost

Aim for **< 150 tokens of frontmatter + description**. Body is fetched only when the skill triggers. Compare to ~70k for PostHog MCP idle — a ~500x reduction. That's the headline.

---

## Sources

- [`PostHog/mcp`](https://github.com/PostHog/mcp)
- [`PostHog/ai-plugin`](https://github.com/PostHog/ai-plugin)
- [`PostHog/skills`](https://github.com/PostHog/skills)
- [`PostHog/context-mill`](https://github.com/PostHog/context-mill)
- [`PostHog/posthog-for-claude`](https://github.com/PostHog/posthog-for-claude) (archived)
- [`PostHog/posthog/cli`](https://github.com/PostHog/posthog/tree/master/cli)
- [PostHog MCP docs](https://posthog.com/docs/model-context-protocol)
- [PostHog MCP for Claude Code](https://posthog.com/docs/model-context-protocol/claude-code)
- [PostHog Skills Store](https://posthog.com/docs/llm-analytics/skills-store)
- [`ComposioHQ/awesome-agent-clis/posthog-cli/SKILL.md`](https://github.com/ComposioHQ/awesome-agent-clis/blob/master/posthog-cli/SKILL.md)
- [`metta-ai/posthog-code-mode-mcp`](https://lobehub.com/mcp/metta-ai-posthog-code-mode-mcp)
- [`cduguet/posthog-eu-mcp`](https://github.com/cduguet/posthog-eu-mcp)
- [OpenTools PostHog MCP registry](https://opentools.com/registry/posthog-mcp)
- [Speakeasy: reducing MCP token usage 100x](https://www.speakeasy.com/blog/how-we-reduced-token-usage-by-100x-dynamic-toolsets-v2)
- [MCP Playground: token counter article](https://mcpplaygroundonline.com/blog/mcp-token-counter-optimize-context-window)
