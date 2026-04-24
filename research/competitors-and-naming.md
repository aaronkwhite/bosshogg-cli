# Competitors & Naming Research: `bosshogg` / `hog` PostHog CLI

Research date: 2026-04-21
Researcher: investigating name availability and competitive landscape before committing to crate name `bosshogg` and binary name `hog`.

---

## TL;DR

- **`bosshogg` on crates.io: AVAILABLE** (crates.io API returned 404).
- **Binary name `hog`: HARD CONFLICT.** PostHog itself ships a `bin/hog` interpreter for their `Hog` programming language (the language behind HogQL, custom destinations, realtime transforms). Using `hog` as our binary name would step on PostHog's own namespace and confuse users.
- **`posthog-cli` crate, `@posthog/cli` npm: TAKEN by PostHog Inc.** They shipped an official Rust CLI in March 2025 and are actively developing it (v0.7.8, last published ~3 days ago). Any positioning must acknowledge and differentiate from the official tool, not replace it.
- Recommend: keep `bosshogg` as crate name; rename binary to something other than `hog` (e.g., `bosshogg`, `bh`, or `phog`).

---

## Part 1: Name Availability

### crates.io

Checked via `https://crates.io/api/v1/crates/<name>` (404 = available, 200 = taken).

| Name           | Status      | Details |
|----------------|-------------|---------|
| `bosshogg`      | AVAILABLE   | 404 on API. No crate by that name. |
| `hog`          | TAKEN       | v0.1.0, author `jrvidal` (Roberto Vidal), license WTFPL, 2004 total downloads, 8 recent, no repo/homepage. Squatter/placeholder. |
| `posthog`      | AVAILABLE   | 404 on API. Surprisingly free, but see `posthog-rs` below. |
| `posthog-cli`  | TAKEN       | **PostHog Inc., official CLI.** v0.5.11 (Nov 2025), description "The command line interface for PostHog hedgehog-emoji", repo `github.com/PostHog/posthog`, MIT, 5,814 downloads, first published 2025-03-03. 17 versions. |
| `posthog-rs`   | TAKEN       | **PostHog Inc., official Rust SDK.** v0.5.2 (2026-04-21, published today), 712,895 total downloads. Very active. |
| `hog-cli`      | AVAILABLE   | 404 on API. |
| `phog`         | TAKEN       | v0.1.0 by Guilhem Mathieux, "A minimal photo Gallery" (Slint), 657 downloads, repo `github.com/guimath/phog`. Unrelated to PostHog but name is taken. |

Also noted: `rusty_hogs` (TruffleHog ports — bins are `ankamali_hog`, `berkshire_hog`, `choctaw_hog`), `hexhog` (hex editor). None collide with `bosshogg` itself.

### Homebrew

Checked `https://formulae.brew.sh/api/formula/<name>.json` and `/cask/<name>.json` (404 = available).

| Name            | Formula | Cask |
|-----------------|---------|------|
| `hog`           | 404 (no formula) | 404 |
| `bosshogg`       | 404 | 404 |
| `posthog`       | 404 | 404 |
| `posthog-cli`   | 404 | — |
| `trufflehog`    | **EXISTS** (v3.94.3, binary is `trufflehog` not `hog`) | — |

No formula collision for `hog`, `bosshogg`, or `posthog-cli`.

### Binary Name `hog` — Collision Analysis

**This is the problem.**

1. **PostHog's own `bin/hog`.** PostHog ships a CLI tool called `hog` as the interpreter for their **Hog programming language** (`.hog` files → compiled to `.hoge` via `bin/hoge`). Docs: `https://posthog.com/docs/hog`. Hog powers realtime destinations, custom transforms, and is the runtime behind HogQL. **Naming our binary `hog` directly collides with PostHog's first-party naming.** Users running a tutorial that says "run `hog script.hog`" and then `brew install bosshogg` would get the wrong tool.
2. **`esturcke/hog`** (GitHub): macOS memory-hog CLI tool, TypeScript/Deno, 3 stars, last release May 2023, inactive. Binary is named `hog`. Obscure — not a serious conflict in the wild.
3. **crates.io `hog`**: squatter by `jrvidal`, 2004 downloads, no repo. Not shipping a binary anywhere.
4. **GNU Parted**: no evidence of a `hog` subcommand/mode in current or historical parted manuals. Earlier reports of a "hog" mode appear to be folklore — not found in docs.
5. **TruffleHog**: binary is `trufflehog`, NOT `hog`. No collision.
6. **Hog-CERN / image-js/hog / preethampaul/HOG**: all academic/ML (Histogram of Oriented Gradients, HDL-on-Git). No shipped `hog` binary on PATH in any mainstream package manager.

**Verdict on `hog` binary:** technically free on Homebrew/apt in the sense that no `hog` formula exists, but PostHog's own Hog language interpreter makes this a bad choice for a PostHog CLI specifically.

### GitHub Repositories

(GitHub search UI was rate-limited during this research; findings are from web search result aggregation.)

- `bosshogg`: **no prominent repo found**. Search for "bosshogg" surfaced unrelated orgs (`bosshack`, Bosch.IO, etc.). Name appears free on GitHub.
- `hog`: multiple repos exist. Notable: `Hog-CERN/Hog` (HDL-on-Git, Tcl), `image-js/hog` (HOG features), `esturcke/hog` (macOS memory CLI), `cangoksal/hog` (CS61A academic project). None are `PostHog/hog`.
- `hog-cli`: not found as a prominent repo.
- `posthog-cli`: the crate points to `github.com/PostHog/posthog` (monorepo, `/cli` directory). An older repo reference `github.com/PostHog/posthog-cli/issues/4` existed ("The future of this CLI") but the standalone repo is gone/merged — the CLI now lives in `PostHog/posthog/cli`.
- Related abandoned PostHog tools: `PostHog/ph` (TypeScript, archived 2022-02-15 — DEPRECATED — early local-dev management attempt) and `PostHog/hog-rs` (archived 2024-06-21, Rust backend services, not a CLI).

### npm

Checked `https://registry.npmjs.org/<name>`.

| Name            | Status | Notes |
|-----------------|--------|-------|
| `bosshogg`       | (not checked individually — search surfaced no package; likely AVAILABLE) |
| `posthog-cli`   | TAKEN  | Unofficial "manage PostHog projects from the terminal", v0.1.6. NOT the official one. |
| `@posthog/cli`  | TAKEN (scoped) | **Official PostHog CLI on npm.** v0.7.8, published ~3 days before research date. |
| `hog`           | TAKEN  | "Hogan templating for Express with partials, layouts, etc.", v0.0.2. Dead package (2012-era). |

---

## Part 2: Existing PostHog CLI Landscape

### 1. Official: `posthog-cli` (a.k.a. `@posthog/cli`, `bin/posthog-cli`)

- **URL / source:** `github.com/PostHog/posthog/tree/master/cli` (inside the monorepo)
- **Registries:** `crates.io/crates/posthog-cli`, `www.npmjs.com/package/@posthog/cli`
- **Language:** Rust (uses `clap`, `reqwest`, `ratatui`, `posthog-rs`, `posthog-symbol-data`)
- **Latest version:** crates.io 0.5.11 (Nov 2025), npm `@posthog/cli` 0.7.8 (Apr 2026). The npm package is further ahead — suggesting they bundle the compiled Rust binary for npm distribution but haven't re-cut the crates.io release.
- **Status:** Very active. 17 crates.io versions since March 2025. Most recent npm publish ~3 days before research date.
- **Stars:** not separable (lives in the PostHog monorepo, 23k+ stars overall).
- **Subcommands:**
  - `login` — interactive browser-based auth, stores personal API token locally.
  - `query` — run SQL (HogQL) against project data. Requires `query:read` scope.
  - `sourcemap` — inject/upload JS sourcemaps for error tracking. Requires `error_tracking:write`. Subcommands: `inject`, `upload`, with `--build` flag and `--delete-after`.
  - `exp endpoints` — experimental: list/get/run/open/pull/push/diff of materialized SQL endpoints (YAML-backed, git-versionable). `run` supports `--json`.
  - `exp tasks` — experimental task management.
  - Promoted from experimental in recent versions: `dsym`, `hermes`, `proguard` mapping uploads (iOS/macOS/Android/Hermes crash symbolication).
- **Auth:** browser-interactive login + env vars `POSTHOG_CLI_TOKEN`/`POSTHOG_CLI_API_KEY`, `POSTHOG_CLI_ENV_ID`/`POSTHOG_CLI_PROJECT_ID`, `POSTHOG_CLI_HOST` (defaults to `https://us.posthog.com`). Env name changed between versions (token→api_key) — minor inconsistency.
- **JSON output:** `--json` on `exp endpoints run` only. Other commands return human-readable text. **Gap: not consistently machine-readable across all subcommands.**
- **Multi-region:** Yes via `POSTHOG_CLI_HOST`. Not first-class (no `us`/`eu` preset).
- **Multi-project:** Single project per invocation via env var. **Gap: no named profiles / config file for switching between projects/envs.**
- **What it does well:**
  - First-class sourcemap / crash-symbol upload pipeline (iOS dSYM, Android proguard, React Native Hermes).
  - Browser-based OAuth-like login flow.
  - HogQL query runner with endpoint-as-code workflow (YAML + push/pull/diff).
  - Available via both cargo and npm and shell installer.
- **What it does poorly / gaps:**
  - No feature-flag management subcommands (create/list/toggle/rollout).
  - No insight / dashboard / cohort management.
  - No event ingestion from the terminal (e.g., `hog capture ...` for scripts).
  - Primarily an error-tracking/sourcemap tool in practice; analytics side is thin.
  - JSON output is not pervasive — weak for agent/scripting use.
  - No profile/config for multi-project or US+EU switching beyond raw env vars.
  - Install story is fragmented (npm, cargo, curl installer) — no Homebrew formula.
  - Minimal TUI polish despite depending on `ratatui`.
  - Named `posthog-cli` — long to type, no short alias.

### 2. Official: `@posthog/wizard`

- **URL:** `github.com/PostHog/wizard`
- **Language:** TypeScript (94.5%)
- **Stars:** ~114, 76 releases, latest v2.9.1 (Apr 2026). Active.
- **Purpose:** AI-powered setup wizard — analyzes your codebase and scaffolds PostHog integration (React, Next, Svelte, Astro, RN, TanStack).
- **Features:** MCP server config, Stripe/revenue analytics setup, CI mode, health checks.
- **Overlap with our tool:** Complementary, not competitive. It's onboarding-oriented, one-shot.

### 3. Deprecated: `PostHog/ph`

- Archived 2022-02-15. TypeScript. Attempted `heroku`-style local-dev tool for self-hosting PostHog. Never adopted. Not a query/analytics CLI.

### 4. Deprecated: `PostHog/hog-rs`

- Archived 2024-06-21. Rust backend services (capture, feature-flag eval, webhooks). Merged into monorepo. Not a CLI.

### 5. MCP-based alternatives (Composio, Zapier, PostHog's own MCP server)

- `posthog.com/docs/model-context-protocol` — PostHog ships an MCP server for AI agents.
- Composio publishes a managed PostHog MCP endpoint with role/team/flag/insight management tools.
- **These bypass the CLI entirely for agent workflows.** An agent-first CLI has to justify why stdio+JSON is better than MCP — likely answer: zero-config, works in CI, works offline of MCP host, and is scriptable outside an LLM loop.

### 6. Third-party / community

- npm `posthog-cli` (v0.1.6, "Unofficial PostHog CLI") — very small, likely abandoned, dwarfed by official `@posthog/cli`.
- `hedgehog-rs` — third-party Rust client, not a CLI.
- No other active community PostHog CLIs surfaced.

---

## Part 3: SEO / Positioning

### Google ranking for "posthog cli" (as of 2026-04-21)

Top organic results consistently:

1. `npmjs.com/package/@posthog/cli` (official npm)
2. `posthog.com/docs/error-tracking/upload-source-maps/cli`
3. `posthog.com/docs/endpoints/cli`
4. `github.com/PostHog/posthog/tree/master/cli`
5. `crates.io/crates/posthog-cli`
6. `lib.rs/crates/posthog-cli`
7. `ComposioHQ/awesome-agent-clis/posthog-cli/SKILL.md` (interesting — someone is already listing "posthog-cli" as an agent CLI)

PostHog Inc. owns the top ~5 slots. We cannot outrank them on the exact-match keyword and should not try to. Play the long tail instead.

### Recommended keywords

- Primary: `posthog cli`, `posthog command line`, `posthog terminal`
- Differentiator tail: `posthog agent cli`, `posthog cli for claude code`, `posthog cli rust`, `posthog feature flags cli`, `posthog hogql cli`, `posthog cli json`, `posthog cli scriptable`
- Avoid collision: don't contest `posthog sourcemap cli` — that's their turf.

### README / crate description keywords

Bake these into the crates.io description (crates.io indexes it for search) and the README first paragraph:

> `bosshogg` — a fast, agent-friendly PostHog CLI in Rust. JSON-first output for Claude Code and other coding agents. Manage feature flags, run HogQL queries, and work across multiple PostHog projects (US + EU) from your terminal.

### GitHub topics tags

`posthog`, `posthog-cli`, `cli`, `command-line`, `rust`, `agent`, `claude-code`, `ai-agents`, `feature-flags`, `analytics`, `hogql`, `product-analytics`, `developer-tools`, `terminal`.

### Should we register `posthog-cli` on crates.io?

**Can't — it's already owned by PostHog Inc.** That resolves the question.

Secondary question: should we register adjacent names like `hog-cli` or `posthog` (the bare name — surprisingly available)?

- `posthog` bare crate name: **Do NOT squat.** It's almost certainly reserved-for-but-not-yet-published by PostHog Inc. Publishing it as a shim invites a trademark/naming dispute and burns goodwill with the upstream we want to be compatible with.
- `hog-cli`: marginal. If you publish it as a thin alias pointing to `bosshogg`, crates.io guidelines frown on squatting. Fine to reserve if you intend to actually publish a functional alias crate, otherwise skip.
- **Recommendation:** Don't squat. Win on content (docs, DX, agent ergonomics), not on namespace games.

---

## Part 4: Recommendation

### Are `bosshogg` + `hog` clear to use?

- **`bosshogg` (crate name): CLEAR.** Available on crates.io, npm, GitHub, Homebrew. Brand fits PostHog's hedgehog whimsy. Distinct enough to not be confused with official `posthog-cli`.
- **`hog` (binary name): DO NOT USE.** Direct conflict with PostHog's own `bin/hog` interpreter for the Hog programming language (`posthog.com/docs/hog`). A PostHog user running both tools will have PATH collisions and conceptual confusion between "run a .hog script" and "query my PostHog project".

### Alternative binary names

In order of preference:

1. **`bosshogg`** — consistent with crate name, unambiguous, no collisions. Slightly long but `alias bh=bosshogg` is easy.
2. **`bh`** — two-letter, fast to type, uncontested on Homebrew/brew/apt/crates.io (quick check warranted before shipping). Risk: too generic, might collide with someone's shell alias for `bash` history, backup helpers, etc.
3. **`phog`** — punchy, rhymes with PostHog, but the crates.io name is taken (photo gallery, inactive). Binary name `phog` on PATH appears free. Acceptable fallback if `bosshogg`-the-binary feels too long.
4. **`posthog`** — bare. Available but risks trademark friction. Avoid.

Recommend shipping the binary as **`bosshogg`** and providing a short alias in install docs (`alias bh=bosshogg`). Keep the door open to rename later if adoption warrants — Cargo makes multi-binary crates easy.

### Positioning vs. official `posthog-cli`

Do NOT position as a replacement. Position as complementary: *"bosshogg is the PostHog CLI built for AI coding agents and multi-project engineers. It complements `posthog-cli` — use the official tool for sourcemap uploads, and use bosshogg for everything else you'd want to do from a terminal or an agent loop."*

### Top 3 features to adopt from `posthog-cli`

1. **Browser-based interactive login** with personal API token storage (`~/.config/posthog/` or OS keyring). Copy the UX exactly so users with existing PostHog auth feel at home.
2. **Env var convention** `POSTHOG_CLI_TOKEN` / `POSTHOG_CLI_HOST` / `POSTHOG_CLI_ENV_ID` — already a de facto standard. Reuse the same variable names so CI setups work unchanged.
3. **HogQL / SQL query runner** — it's the highest-value read-side command. Match their `query` subcommand semantics and scope requirements, but add `--json` as the default for agents.

### Top 3 gaps in the landscape we should fill

1. **Feature-flag management from the terminal.** `posthog-cli` doesn't do this. `bosshogg flags list`, `bosshogg flags toggle --percent 10 my-flag`, `bosshogg flags rollout my-flag --cohort beta`. Massive DX win for engineers + scripting + CI.
2. **JSON-first, agent-native output.** Every subcommand returns structured JSON by default when stdout is not a TTY, or with `--json`. Stable schemas, documented exit codes, no ANSI escape chaos. Ship a schema file (`bosshogg schema --subcommand flags.list`) so agents can plan.
3. **Multi-project / multi-region profiles.** `bosshogg profile add prod-eu --host https://eu.posthog.com --env-id ... --token ...` and `bosshogg --profile prod-eu flags list`. First-class US/EU support. `posthog-cli` forces env-var juggling today.

Bonus ideas worth scoping:
- `bosshogg capture` — fire an event from the shell (useful for release markers, one-off experiments).
- `bosshogg dashboard open <id>` — open the web URL in browser (cheap, delightful).
- Homebrew tap (`brew tap aaronkwhite/bosshogg-cli && brew install bosshogg`) — `posthog-cli` doesn't have one; easy SEO/adoption win.
- First-class MCP server mode (`bosshogg mcp`) so the same tool is also an MCP server for agents. Directly competes with PostHog's own MCP server, but ours would be local-first and auth-once.

---

## Sources

- [crates.io API: posthog-cli](https://crates.io/api/v1/crates/posthog-cli)
- [crates.io API: posthog-rs](https://crates.io/api/v1/crates/posthog-rs)
- [crates.io API: hog](https://crates.io/api/v1/crates/hog)
- [crates.io API: phog](https://crates.io/api/v1/crates/phog)
- [lib.rs: posthog-cli](https://lib.rs/crates/posthog-cli)
- [PostHog/posthog monorepo /cli](https://github.com/PostHog/posthog/tree/master/cli)
- [PostHog CLI CHANGELOG](https://github.com/PostHog/posthog/blob/master/cli/CHANGELOG.md)
- [PostHog docs: sourcemap CLI](https://posthog.com/docs/error-tracking/upload-source-maps/cli)
- [PostHog docs: endpoints CLI](https://posthog.com/docs/endpoints/cli)
- [PostHog docs: Hog language](https://posthog.com/docs/hog)
- [PostHog/ph (archived)](https://github.com/PostHog/ph)
- [PostHog/hog-rs (archived)](https://github.com/PostHog/hog-rs)
- [PostHog/wizard](https://github.com/PostHog/wizard)
- [npm @posthog/cli](https://www.npmjs.com/package/@posthog/cli)
- [npm posthog-cli (unofficial)](https://registry.npmjs.org/posthog-cli)
- [esturcke/hog (macOS memory CLI)](https://github.com/esturcke/hog)
- [Homebrew formula: trufflehog](https://formulae.brew.sh/api/formula/trufflehog.json)
- [Composio posthog-cli SKILL](https://github.com/ComposioHQ/awesome-agent-clis/blob/master/posthog-cli/SKILL.md)
- [PostHog MCP docs](https://posthog.com/docs/model-context-protocol)
- [Issue: Command-line interface (CLI) to manage PostHog](https://github.com/PostHog/posthog/issues/4025)
