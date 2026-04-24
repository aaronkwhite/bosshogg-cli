# Agent-first design

The thesis, the math, the decision tree, and the skill layout.

## The thesis

A CLI that ships with a Claude Code skill is a strictly better agent surface than an MCP server for 90% of operational PostHog work.

Two reasons:

1. **Idle context cost.** PostHog's official MCP server ships approximately **44,000 tokens** of tool definitions (200+ tools across 28 feature groups) that load into context the moment an agent session starts, *before* a user has typed anything. A skill frontmatter is ~200 tokens and the body loads on demand. For workflows that don't need chart rendering or the web-UI wizard surface, the CLI wins a ~220x reduction in idle cost.

2. **Scriptability.** MCP servers are agent-only by design. A CLI also runs in CI pipelines, shell aliases, make targets, and `ssh && run` loops. The same binary serves two customers.

Numbers to cite:

| Surface | Idle context cost | Tool count |
|---|---|---|
| PostHog MCP server | ~44,000 tokens | 200+ (28 feature groups) |
| `PostHog/ai-plugin` | same (wraps MCP) + slash commands | 13 slash + 1 skill |
| `metta-ai/posthog-code-mode-mcp` | ~2,000 tokens | 1 (`execute_code`) |
| **BossHogg skill** | ~200 tokens frontmatter | 0 until invoked |

Supporting benchmarks:

- **Scalekit's gh-vs-GitHub-MCP benchmark** (Claude Sonnet 4, 75 runs): `gh` CLI 1,365 vs GitHub MCP 44,026 tokens on identical tasks (**32x gap**). MCP failure rate 28% (ConnectTimeout) vs CLI 100% success. At 10k ops/month: CLI $3.20 vs MCP $55.20 (17x cost multiplier). "A skill-augmented CLI — just an 800-token document of gh tips — reduces tool calls by a third and latency by a third versus naive CLI."
- **Anthropic's "Code execution with MCP"**: a 5-server, 58-tool setup starts at ~55k tokens upfront; their Tool Search Tool yields 85% reduction and lifts Opus 4 MCP-eval accuracy from 49% → 74%.

MCP client caps that bite real users:

- **Cursor**: hard-limits at 40 MCP tools.
- **GitHub Copilot**: 128.
- Multiple PostHog community threads report hitting these before adding a single other server.

## When to use what

Decision tree for an agent deciding how to touch PostHog:

```
Need to read/query/mutate PostHog state?
├─ Yes
│  ├─ Need a rendered chart, dashboard screenshot, or guided wizard?
│  │  └─ Use the PostHog MCP server or the web app.
│  ├─ Need source-map, dSYM, ProGuard upload, or release tracking?
│  │  └─ Use @posthog/cli.
│  └─ Everything else: list/get/create/update/delete/query/toggle?
│     └─ Use BossHogg.
└─ No (just instrumenting code)
   └─ Use posthog-rs SDK.
```

BossHogg's skill makes this explicit. The `references/mcp-gaps.md` file inside the skill is the decision matrix agents consult when a task is ambiguous.

## The skill we ship

Lives at `.claude/skills/bosshogg/` in the repo. Installable via:

- Copy-to-project: drop the directory into any repo's `.claude/skills/`.
- Claude Code plugin marketplace: eventually, after v1.
- Manual: link from global skills directory.

### Frontmatter (the ~200-token budget — pushy, triggers + exclusions)

Per Anthropic guidance, Claude **under-triggers** skills by default. The description is the only tool it has to decide between 100+ installed skills — it must be pushy, third-person, list both what the skill does *and* when to reach for it, and explicitly exclude look-alike scenarios. `allowed-tools` and `when_to_use` are Claude Code extensions that tighten triggering.

```yaml
---
name: bosshogg
description: >
  Operate on PostHog from the terminal — feature flags, HogQL queries,
  insights, dashboards, cohorts, persons, events, experiments, surveys,
  session replays, error tracking, hog functions, batch exports. Use
  whenever the user mentions PostHog, Hog, feature flags, A/B tests,
  session recordings, funnels, retention, insights, dashboards, cohorts,
  HogQL, or $ai_* LLM observability events. Always prefer the `bosshogg`
  CLI for reads and simple writes; fall back to the PostHog MCP server
  for unusual tools, and to direct HogQL/REST via `bosshogg query run` or
  `bosshogg auth token` + curl for anything else. Run `bosshogg doctor` on
  first use. Do NOT use for unrelated analytics vendors (Amplitude,
  Mixpanel, Segment, GA4), generic SQL linting, or purely frontend
  bundling issues.
allowed-tools: Bash(bosshogg *), Bash(jq *), Read, Write
when_to_use: >
  Triggered when the user references PostHog, wants product analytics,
  feature flag changes, session replay debugging, or LLM observability.
---
```

Pre-v1.0, run skill-creator's description-optimizer loop: generate 20 trigger-eval queries (10 should-trigger, 10 near-miss), split 60/40 train/test, iterate up to 5 description variants, pick the one that scores highest on held-out test queries.

### Skill body outline

1. **When to use `bosshogg`** — the decision rule above, in one paragraph.
2. **Setup and auth** — `bosshogg auth login` (browser) or `POSTHOG_CLI_TOKEN` env var; never echo keys; respect `POSTHOG_CLI_HOST` for EU/self-hosted.
3. **Golden rules** — always pass `--json`; verify unknown flags with `bosshogg <cmd> --help` before composing; never inline API keys in shell commands; treat non-zero exit + stderr JSON as authoritative.
4. **Core workflows** — inline recipes for the 80% case:
   - *Query events* — `bosshogg events list --event <name> --days 7 --json`
   - *HogQL* — `bosshogg query run --file query.sql --json`
   - *Read insight* — `bosshogg insights get <id> --json`
   - *List / toggle flag* — `bosshogg flags list`, `bosshogg flags update <key> --enabled`
   - *Inspect person* — `bosshogg persons get <distinct_id> --json`
   - *Inspect cohort* — `bosshogg cohorts get <id> --json`
5. **Recipe: debug a flag rollout** — grab flag → query evaluation events → compute exposure split → list persons per variant.
6. **Recipe: verify event instrumentation** — list recent events for a distinct_id → inspect property schema → diff expected vs observed.
7. **Recipe: insight triage** — list dashboards → fetch insight → run its HogQL → return a terse summary.
8. **Recipe: cohort inspection** — resolve cohort → list members → sanity-check size.
9. **Recipe: experiment audit** — list running experiments → check exposure and primary metric → flag anomalies.
10. **When to reach for MCP instead** — one paragraph pointing at `references/mcp-gaps.md`.
11. **Error handling** — structured error shape; common codes (`AUTH_*`, `RATE_LIMITED`, `NOT_FOUND`, `VALIDATION`); recovery paths.
12. **JSON output contract** — stable field names; paginated results use `{results, next_cursor, count}`; timestamps RFC3339; schemas exported via `references/schemas.json`.
13. **Anti-patterns** — don't pipe `bosshogg` through `jq` when field selectors exist; don't loop over `bosshogg events list` per user; don't confuse flag *key* with flag *id*.
14. **References** — link to `references/commands.md`, `references/mcp-gaps.md`, `references/hogql-recipes.md`, `references/auth.md`.

### Supporting files

- **`.claude-plugin/marketplace.json`** — `/plugin marketplace add aaronkwhite/bosshogg-cli` → `/plugin install bosshogg@bosshogg` one-liner install.
- **`references/flags.md`** — full from M1 (the single CRUD-deep resource in MVP). Other resource references land as their milestones ship.
- **`references/query-hogql.md`** — HogQL recipes + `bosshogg schema hogql` grounding instructions. Cribbed (with attribution) from `PostHog/ai-plugin/skills/query-examples`.
- **`references/auth-and-scopes.md`** — complete from M1. Scopes, 403 remediation, contexts, EU/self-host config, CI setup.
- **`references/mcp-gaps.md`** — decision matrix: *Task → Use this (bosshogg/MCP/HogQL/REST) → Why not the other*.
- **`references/cross-product-playbooks.md`** — the killer differentiator. Decision trees for multi-product tasks nobody else has encoded: "why did conversion drop", "safe rollout", "debug a user", "ship an event", "LLM-app debug", "incident notebook", "GDPR deletion". Two playbooks land in M1; all seven by M9.
- **`scripts/`** — deterministic bash/TS that runs off-context. Only stdout enters context: `doctor.sh` (wraps `bosshogg doctor`), `preflight-scope.sh` (benign GET, parses 403), `hogql-schema-dump.sh` (caches schema.json per session), `replay-summarize.ts` (bounded replay summary), `flag-rollout-guard.ts` (error-rate/latency check before ramp).
- **`examples/`** — end-to-end transcripts showing how a task flows through the skill (one per playbook eventually).
- **`evals/evals.json`** — 20 test queries (10 should-trigger, 10 near-miss, multi-step). CI runs against Haiku on PRs, Opus on release tags. Asserts trigger/no-trigger decisions.
- **`references/schemas.json`** — JSON schema for every command's `--json` output. Machine-readable for agents planning tool composition.

## `bosshogg mcp` mode (post-v1)

Same binary, different transport. The vision:

```
bosshogg mcp --stdio
```

Exposes the full CLI surface as MCP tools over stdio. Same auth, same JSON contracts, different invocation. Purpose: users who *want* MCP ergonomics can get them without reinstalling anything, and without the 70k-token bloat — because BossHogg's tool descriptions are far terser than the hosted MCP's.

**Why it's not v1:**

- Keeps v1 scope small.
- The CLI contract has to be stable first — flipping MCP tools to new JSON shapes is worse than rolling a new CLI subcommand.
- Need to decide: one tool per subcommand (fine-grained, familiar) or one `execute` tool that takes command + args (code-mode style, ~2k tokens, matches metta-ai's fork). Probably the former for discoverability.

**What we'll do in v1 to make this easy later:**

- Every subcommand's `--json` output is already the tool response.
- Every subcommand's flag list is already a parsed Clap schema — can be reflected into JSON Schema at compile time.
- Error shape is already the MCP error shape.

## Measurement

We'll prove the thesis with numbers. Before v1 ships:

- Measure idle token cost of a cold Claude Code session with just BossHogg's skill present. Target: <300 tokens.
- Measure token cost after the skill triggers on a representative task ("list my feature flags"). Target: <3,000 tokens including the response.
- Compare to the PostHog MCP session doing the same task. Document both in `docs/agent-first.md` (this file) once measured.

## Competitive awareness

- **PostHog's own AI investment** is `ai-plugin` (active) + `skills` (under construction, auto-generated from `context-mill`). Both are instrumentation-flavored ("teach Claude to add PostHog to my codebase"). BossHogg is operational-flavored ("teach Claude to operate my running PostHog project"). Non-overlapping.
- **`metta-ai/posthog-code-mode-mcp`** reached the same philosophical conclusion (context cost matters) but stayed in MCP-land with a single `execute_code` tool. Interesting prior art; worth linking from the skill's anti-patterns section (don't use `execute_code` when a first-class verb exists).
- **`ComposioHQ/awesome-agent-clis/posthog-cli/SKILL.md`** is a 17-line stub. Our skill is the reference implementation for what a real CLI-first PostHog agent surface looks like.

## Non-goals

- **Not a replacement for MCP entirely.** Chart rendering, Max AI, the PostHog wizard — those stay on MCP or the web. We'll link to them from the skill.
- **Not a universal agent protocol.** The skill is Claude-Code-shaped first. Cursor rules, Codex config, Gemini manifests come next via the Skills spec. Don't invent new formats.
- **Not a training-data replacement.** PostHog MCP docs, `ai-plugin` skills, and `context-mill` outputs inform recipes and terminology — we cite and thank them; we don't fork them.
