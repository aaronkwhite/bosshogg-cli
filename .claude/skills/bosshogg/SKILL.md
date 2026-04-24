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

# bosshogg

An agent-first PostHog CLI. Use this skill to decide *how* to touch a PostHog project from the terminal — CLI first, PostHog MCP next, direct HogQL third, raw REST last.

This body is a decision tree. Read the matching `references/*.md` only when a section below tells you to.

## 1. When to use `bosshogg`

Use `bosshogg` whenever the user wants to read or write PostHog state from the terminal: list or toggle feature flags, run HogQL, inspect insights/dashboards/cohorts/persons, look up events, check experiments and surveys, browse session replays metadata, review error fingerprints, operate hog functions or batch exports, or explore `$ai_*` LLM observability events.

**Decision rule:** if the task names a PostHog concept and does not require a rendered chart, a guided wizard, or a browser-based action, `bosshogg` is the right tool. Otherwise fall back per section 6.

## 2. Preflight (first call per session)

Before running any other `bosshogg` command, run:

```bash
bosshogg doctor --json
```

Branch on the output:

- `"summary": {"ok": true, ...}` — proceed.
- `"summary": {"ok": false, ...}` — one or more checks failed. Iterate `checks[]` and for each entry where `"ok": false`, follow the `"remediation"` string. Common check names and remediations:
  - `config_file` → config is missing or unparseable. Run `bosshogg configure`.
  - `active_context` → no context is active. Run `bosshogg configure` or `bosshogg use <name>`.
  - `api_key_present` → no API key in the active context. Run `bosshogg configure` or `bosshogg config set-context`.
  - `host_region_match` → the configured host doesn't match the region setting. Fix via `bosshogg config set-context`.
  - `key_alive` → key is rejected or expired. Rotate via PostHog settings and re-run `bosshogg configure`.
  - `project_access` → project ID is wrong or the key lacks `project:read` scope. Verify with `bosshogg whoami`.
  - `env_access` → env_id is misconfigured (optional check — `ok: true` if env_id is unset).
- If `bosshogg doctor` itself exits non-zero without valid JSON (binary crash or PATH issue), stop and surface the raw error to the user; do not retry in a loop.

Prefer the wrapper at `scripts/doctor.sh` if the user asked for a classified summary (it prints a one-line human verdict and exits non-zero on `error`).

## 3. Setup & auth

Two setup paths:

1. **Interactive** (recommended for workstations): `bosshogg configure`. This upserts a named context in `~/.config/bosshogg/config.toml` with `host`, `project_id`, `env_id`, and stores the key with `0600` perms.
2. **Env-var only** (required in CI): set one of `POSTHOG_CLI_TOKEN`, `POSTHOG_CLI_API_KEY`, `POSTHOG_API_KEY` (resolved in that order). Also set `POSTHOG_CLI_HOST` and `POSTHOG_CLI_PROJECT_ID` if not using US Cloud defaults.

Key hygiene:

- Never paste the key on a command line. Use `--key-from-env` or `--key-from-stdin` with `bosshogg config set-context`.
- Keys are personal API keys (prefix `phx_...`). They carry granular scopes (`feature_flag:read`, `query:read`, etc.). There is no scope-introspection endpoint ([PostHog#25865](https://github.com/PostHog/posthog/issues/25865)); errors arrive via 403 on first use.
- `bosshogg auth token` emits the resolved bearer on stdout for a single invocation (see section 6 — escape hatch).

EU and self-host:

- EU Cloud: `POSTHOG_CLI_HOST=https://eu.posthog.com`.
- Self-host: set the custom host; `bosshogg doctor` verifies the host answers and reports the detected region back.

## 4. Golden rules

- Always pass `--json` when the output is going into another command or being parsed. The CLI auto-detects non-TTY stdout and defaults to JSON, but being explicit prevents surprises when redirection changes.
- Before composing a command with unfamiliar flags, run `bosshogg <cmd> --help`. Flag surfaces are stable across a minor version but grow additively — do not memorize.
- Never inline API keys in shell. Use env vars or named contexts.
- Treat structured errors as authoritative. A non-zero exit with `{"error": true, "code": "...", "hint": "..."}` on stderr tells you exactly what to do next. Do not parse human-readable error strings.
- Output is compact JSON with no envelope. Do not look for `{"data": ...}` — `bosshogg flag list --json` returns `{"count": N, "next_cursor": "...", "results": [...]}` directly.

## 5. Core workflows

One-liner recipes for the 80% case. Lean on these before reaching for HogQL or REST.

| Task | Command |
|---|---|
| Ground HogQL in the active project schema | `bosshogg schema hogql --json` (cache via `scripts/hogql-schema-dump.sh`) |
| Run a HogQL query (inline) | `bosshogg query run "SELECT count() FROM events WHERE event = '$pageview'" --json` |
| Run a HogQL query (file) | `bosshogg query run --file query.sql --json` |
| Run an async HogQL query | `bosshogg query run --file big.sql --async --json` (polls) |
| List active flags | `bosshogg flag list --active --json` |
| Get a specific flag | `bosshogg flag get my-feature --json` |
| Toggle a flag on | `bosshogg flag update my-feature --enabled --json` |
| Set rollout percentage | `bosshogg flag update my-feature --rollout 10 --json` |
| Look up a person | `bosshogg person get user@example.com --json` (M4+ — stub in M1) |
| Show the current context | `bosshogg whoami --json` |
| Read an insight | `bosshogg insight get <id> --json` (M3+) |
| Inspect a cohort | `bosshogg cohort get <id> --json` (M3+) |
| Escape-hatch bearer token | `curl -H "Authorization: Bearer $(bosshogg auth token)" https://us.posthog.com/api/...` |

Recipe references for depth:

- Flags: `references/flags.md`
- HogQL: `references/query-hogql.md`
- Auth and scopes: `references/auth-and-scopes.md`

## 6. Routing: CLI → MCP → HogQL → REST

Cascade when the first option does not fit:

1. **`bosshogg <verb>`** — always try first for reads and simple writes. JSON output is stable; error codes are deterministic.
2. **PostHog MCP** (`mcp__posthog__*`) — reach for this when the task needs a tool `bosshogg` does not expose. Common MCP-only tasks today: rendering an insight chart, authoring a PostHog Notebook, launching the Max AI wizard, issuing prompt-create, and uploading sourcemaps (use `@posthog/cli` for that). See `references/mcp-gaps.md` for the full decision matrix.
3. **`bosshogg query run`** — for analytics not modeled by a higher-level subcommand, write HogQL directly. Always ground via `bosshogg schema hogql` first so column names are real.
4. **Raw REST via `bosshogg auth token`** — last resort for endpoints neither the CLI nor the MCP covers (e.g., some activity-log filters, early-access beta endpoints). Pattern:

   ```bash
   TOKEN=$(bosshogg auth token)
   curl -sSf \
     -H "Authorization: Bearer $TOKEN" \
     "https://us.posthog.com/api/projects/999999/some_unmapped_endpoint/"
   ```

Cascade example — "render a trend chart":

1. `bosshogg insight get <id> --json` returns the insight definition but no rendered PNG.
2. `mcp__posthog__insight-render-chart` renders it (if configured).
3. If MCP is not available, fall back to the web UI URL `https://us.posthog.com/project/<pid>/insights/<short_id>`.

## 7. Output hygiene (mandatory)

Three rules baked into the CLI; do not try to bypass them:

- **HogQL auto-`LIMIT 100`.** `bosshogg query run` injects `LIMIT 100` when the parsed query has no `LIMIT`. Pass `--no-limit` only when the user has explicitly asked for an unbounded result and accepts the context cost.
- **Replay snapshots never go to stdout.** When `bosshogg session-recording get --snapshot` lands (M8), it requires `--out <file>`. If you are tempted to pipe a snapshot into `jq`, stop — use `--out`.
- **LLM trace bodies default summarized.** For `$ai_*` events, helper subcommands return model, cost, latency, and token counts — not the prompt or response body. Pass `--full` only when the user asked for the full message.

## 8. Destructive operations

For any write that would disable, delete, or meaningfully ramp a flag/experiment/cohort, the CLI requires `--yes`. Before adding `--yes`, confirm in prose with the user:

> About to set flag `checkout-redesign` rollout from 10% to 50%. Guardrails to monitor: error rate last 1h, p95 latency last 15 min. Proceed? (yes/no)

If the user says yes, re-run with `--yes`. If they say no, stop and summarize what you were going to change.

Destructive commands:

- `bosshogg flag update ... --disabled` (at non-zero rollout)
- `bosshogg flag update ... --rollout <N>` (when N increases)
- `bosshogg <resource> delete <id>` (lands per resource through M2–M8)

## 9. Error handling

When a command exits non-zero with `{"error": true, ...}` on stderr, consult this table.

| Code | Action |
|---|---|
| `AUTH_MISSING` | No key resolved. Run `bosshogg configure` or set `POSTHOG_CLI_TOKEN`. |
| `AUTH_INVALID` | Key rejected. It may be revoked — re-auth with a fresh personal API key. |
| `AUTH_SCOPE` | Key is missing the scope named in `hint`. Issue a new key with that scope added. See `references/auth-and-scopes.md`. |
| `NOT_FOUND` | Resource id or key does not exist in the active project. Verify `bosshogg whoami` shows the right project, then re-check the id. |
| `BAD_REQUEST` | Client-side input shape is wrong. Read `hint`, re-run `bosshogg <cmd> --help`, and retry with the suggested `--retry_with` flags. |
| `CONFLICT` | Concurrent edit or uniqueness violation. Re-fetch the resource and reapply. |
| `VALIDATION` | Pre-flight failed (e.g., bad date, missing `--out` on a binary-output command). Fix the flag and retry. |
| `RATE_LIMITED` | Wait `retry_after_s` seconds. The query bucket is separate (2400/hr) and team-wide — rotating keys does not help. |
| `UPSTREAM` | 5xx from PostHog. Transient. Back off and retry up to 3×. |
| `NETWORK` | DNS/TLS/connection error. Check `bosshogg doctor`. |
| `TIMEOUT` | Request exceeded `--timeout`. Retry with a larger timeout, or use `--async` on queries. |
| `SCHEMA_DRIFT` | Response did not match the typed struct. Upgrade `bosshogg` or file an issue. |
| `CONFIG` | Missing or unparseable config — usually a missing `POSTHOG_CLI_*` env var. The `message` names the exact fix. |
| `INTERNAL` | Unexpected client bug. Report with `--debug` trace. |

Exit codes are stable; scripts can rely on them. Full catalog in the `docs/conventions.md` of the repo.

## 10. JSON output contract

- Compact, single-line JSON. No `{"data": ...}` envelope.
- List endpoints: `{"count": N, "next_cursor": "...|null", "results": [...]}`.
- Singular `get`: the resource object itself (e.g., `{"id": 123, "key": "my-flag", ...}`).
- Mutations: `{"ok": true, "id": 123, "action": "update"}`.
- Timestamps are RFC3339 UTC everywhere in JSON mode.
- Field names are stable across minor versions; removing or renaming a field is a SemVer major.

## 11. Anti-patterns

- **Do not pipe `bosshogg` through `jq` when the CLI exposes a field selector.** (Planned polish — for now, `jq` is fine, but prefer CLI-native filters when they exist.)
- **Do not confuse flag *key* with flag *id*.** The key is a human-readable string (`checkout-redesign`); the id is an integer. `bosshogg flag get <key>` resolves keys via an in-process cache. Most CLI verbs accept either, but only the key is stable across environments.
- **Do not mix hyphens and underscores in flag keys.** PostHog normalizes but PostHog's own UI tooling treats `my-flag` and `my_flag` as distinct. Pick one style and stick to it.
- **Do not loop `bosshogg person get` per user.** For any "for each user" task, write a HogQL query that returns the whole set in one shot. Example: `SELECT distinct_id, properties.email FROM persons WHERE properties.plan = 'pro' LIMIT 1000`.
- **Do not shell out to `curl` when `bosshogg` has a verb.** The escape-hatch pattern exists for endpoints the CLI does not cover — not as a default.
- **Do not echo the resolved key.** `bosshogg auth token | pbcopy` is fine; `echo "key=$(bosshogg auth token)"` gets logged to shell history.
- **Do not trust a cached schema across projects.** `scripts/hogql-schema-dump.sh` caches per project id — re-run after a `bosshogg use <context>` that changes project.

## 12. Cross-product playbooks

See `references/cross-product-playbooks.md` for decision trees that span multiple PostHog products. Two are complete in M1:

- **Safe feature flag rollout** — create at 0%, ramp 1/10/50/100 with HogQL-driven guardrails, auto-rollback on breach.
- **Debug a specific user** — resolve person → events → flag variants → (M8) errors and replays.

Five more land as their enabling resources ship: conversion-drop (M3), ship-an-event (M4), llm-app-debug (M7), incident-notebook (M8), gdpr-deletion (M4+M8).

## 13. When to reach for MCP instead

PostHog's MCP server covers tools `bosshogg` does not expose — chart rendering, the Max AI wizard, notebook authoring, prompt-create, and a handful of workflow UI features. When the user asks for a rendered chart, an interactive wizard, or a feature gated to the web UI, use `mcp__posthog__*` (with fully-qualified tool names).

The full task-by-task decision matrix is in `references/mcp-gaps.md`. Do not guess — read it.

## 14. References

- `references/flags.md` — full feature-flag recipe book (the M1 CRUD-deep resource).
- `references/query-hogql.md` — HogQL recipes, schema grounding, async polling, LIMIT behavior.
- `references/auth-and-scopes.md` — scopes catalog, 403 remediation, context management, EU/self-host.
- `references/mcp-gaps.md` — `bosshogg` vs PostHog MCP vs HogQL vs REST decision matrix.
- `references/cross-product-playbooks.md` — safe-rollout and debug-a-user playbooks; placeholders for the other five.
- `scripts/doctor.sh` — wraps `bosshogg doctor --json` and prints a one-line verdict.
- `scripts/preflight-scope.sh` — probes a scope via a benign GET and emits a remediation string on 403.
- `scripts/hogql-schema-dump.sh` — caches `bosshogg schema hogql` per project under `~/.cache/bosshogg/`.
- `examples/safe-rollout.md` — end-to-end transcript of the safe-rollout playbook.
- `evals/evals.json` — 20 trigger/near-miss queries run in CI to guard description quality.
