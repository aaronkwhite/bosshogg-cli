# Capability surface

Human-readable render of [`../research/capability-schema.yaml`](../research/capability-schema.yaml). If you're editing one, edit both.

## At a glance

- **1 central command**: `bosshogg query run` (HogQL over `POST /api/environments/:env_id/query/`)
- **25 resource command groups** covering the full GA surface
- **~45-55 subcommands** total at v1
- **Agent utilities**: `bosshogg doctor`, `bosshogg schema hogql`, `bosshogg auth token`
- **Kubectl/gh-style contexts**: `bosshogg config set-context / use-context` + `bosshogg use <name>` shortcut
- **Resource names are SINGULAR** and mirror PostHog MCP tool taxonomy (`feature-flag-create` → `bosshogg flag create`) so model training on MCP transfers for free

## Command shape

```
bosshogg <resource> <verb> [identifier] [flags]
```

Examples:

```
bosshogg flag list --active --json
bosshogg flag get my-flag --json
bosshogg flag update my-flag --enabled --rollout 25
bosshogg query run --file q.sql --async --json
bosshogg insight refresh 12345 --json
bosshogg person get distinct_id_123 --json
bosshogg doctor --json
bosshogg schema hogql | jq '.tables[0]'
```

### Global flags

- `--json` — machine-readable output. Default when stdout isn't a TTY.
- `--debug` — print HTTP requests/responses to stderr (redacted `Authorization`).
- `--context <name>` — override active context for one invocation.
- `--project <id>` / `--env <id>` — override the context's project/env for one invocation.
- `--host <url>` / `--api-key <key>` — full manual override.
- `--no-paginate` — don't auto-follow `next` cursors.
- `--limit <n>` — cap results.
- `--timeout <s>` — override request timeout.

## Meta commands

### `bosshogg configure`
Interactive first-run wizard. Prompts for name, host/region, personal API key, default project/env. Writes to `~/.config/bosshogg/config.toml`. Thin wrapper over `config set-context`.

### `bosshogg whoami`
`GET /api/users/@me/`. Returns current user, current project, and the scope set on the active key. Used by `doctor` internally.

### `bosshogg doctor`
Preflight health check. Validates binary on PATH, active context present, API key alive, project/env accessible, region matches key, clock skew, and (where possible) scope-for-action remediation on 403. `--json` output is consumed by the skill's `scripts/doctor.sh`.

### `bosshogg schema`
Schema introspection for grounded HogQL.

- `bosshogg schema hogql` — dumps the ClickHouse/HogQL schema (events, persons, sessions, groups, `system.*`, warehouse tables) for the active project. LLMs need this constantly. **Ships in M1.**
- `bosshogg schema mcp` — dumps local cached MCP tool names + filter groups so the skill can route between CLI and MCP. **Deferred to v1.x.**

### `bosshogg auth`
- `bosshogg auth token` — emits the personal API key for the active context to stdout. Escape hatch for `curl -H "Authorization: Bearer $(bosshogg auth token)" ...`. **Ships in M1.**
- `bosshogg auth login` (browser-based) / `bosshogg auth logout` — **deferred to v1.1.**

### `bosshogg config`
Context (kubectl/gh-style named environments) management.

- `bosshogg config set-context <name> [flags]` — upsert a context with `--host`, `--region`, `--project`, `--env`, `--key-from-env`, `--key-from-stdin`.
- `bosshogg config get-contexts` — list.
- `bosshogg config current-context` — show active.
- `bosshogg config use-context <name>` — switch active.
- `bosshogg config delete-context <name>` — remove.

### `bosshogg use <name>`
Shorthand for `bosshogg config use-context <name>`.

### Other meta
- `bosshogg version` — version + build info.
- `bosshogg completion <shell>` — bash/zsh/fish/powershell.
- `bosshogg mcp --stdio` — run BossHogg as an MCP server. **Stretch at v1.0; likely v1.1.**

## Resources

Grouped by milestone and purpose. All names are singular.

### Query (central command) — M1

| Verb | Call | Notes |
|---|---|---|
| `query run` | POST `/query/` | inline SQL, `--file`, or stdin. Auto `LIMIT 100` unless `--no-limit`. `--async` polls for completion. |
| `query hogql` | POST `/query/` | alias for `run --kind HogQLQuery` |
| `query events` | POST `/query/` | EventsQuery wrapper — flag-driven event filter |
| `query trends` | POST `/query/` | TrendsQuery wrapper |
| `query funnel` | POST `/query/` | FunnelsQuery wrapper |
| `query status` | GET `/query/:id/` | check async query |
| `query cancel` | DELETE `/query/:id/` | cancel async query |
| `query log` | GET `/query/:id/log/` | last-24h execution log |
| `query draft-sql` | GET `/query/draft_sql/` | server-side SQL draft helper |

**Rate limit:** separate bucket at 2400/hr.

### Flag — M1 (single CRUD-deep resource in MVP)

| Verb | Call | Notes |
|---|---|---|
| `flag list` / `get` | `/feature_flags/[:id/]` | filters: `--active`, `--type`, `--runtime`, `--tag`, `--search` |
| `flag create` | POST | accepts `--filters-file` (JSON), `--payload-file` |
| `flag update` | PATCH | `--enabled` / `--disabled` / `--rollout N`, soft-delete via `--deleted` |
| `flag delete` | PATCH `{deleted: true}` | normalized soft-delete |
| `flag enable` / `disable` / `rollout` | PATCH | convenience wrappers |
| `flag evaluate` | POST `/flags?v=2` | **project token**, `--distinct-id`, `--groups`, `--person-props` |
| `flag dependents` | GET `/:id/dependent_flags/` | |
| `flag activity` | GET `/:id/activity/` | |

### Endpoint (materialized HogQL) — M5

| Verb | Call |
|---|---|
| `endpoint list` / `get` / `create` / `update` / `delete` | `/endpoints/[:name/]` |
| `endpoint run` | GET `/endpoints/:name/run/` |
| `endpoint materialize-preview` | POST `/endpoints/:name/materialization_preview/` |
| `endpoint materialize-status` | GET `/endpoints/:name/materialization_status/` |
| `endpoint openapi` | GET `/endpoints/:name/openapi.json/` |

### Org / Project — M2

| Verb | Call | Notes |
|---|---|---|
| `org list` / `get` / `current` / `switch` | `/organizations/…` | create/delete deferred |
| `project list` / `get` / `current` / `switch` | `/organizations/:org_id/projects/…` | read-focused |
| `project reset-token` | PATCH `/projects/:id/reset_token/` | mutating but narrow |

### Insight / Dashboard / Cohort — M3

| Resource | Subcommands |
|---|---|
| `insight` | `list`, `get`, `refresh`, `create`, `update`, `delete`, `tag`, `activity`, `share` |
| `dashboard` | `list`, `get`, `refresh`, `create`, `update`, `delete`, `tiles`, `share`, `snapshot` |
| `cohort` | `list`, `get`, `create`, `update`, `delete`, `members`, `add-person`, `remove-person`, `calculation-history`, `activity` |

### Person / Group / Event / Action / Annotation — M4

| Resource | Subcommands |
|---|---|
| `person` | `list`, `get`, `delete`, `update-property`, `delete-property`, `properties-timeline`, `activity`, `split` |
| `group` | `list`, `find`, `property-definitions`, `property-values`, `related`, `activity`, `update-property`, `delete-property` |
| `event` | `list` (via HogQL), `get`, `values`, `tail` |
| `action` | `list`, `get`, `create`, `update`, `delete`, `references`, `tag` |
| `annotation` | `list`, `get`, `create`, `update`, `delete` |

### Event / Property Definition — M5

| Resource | Subcommands |
|---|---|
| `event-definition` | `list`, `get`, `update`, `delete`, `by-name`, `metrics`, `tag` |
| `property-definition` | `list`, `get`, `update`, `delete`, `seen-together`, `tag` |

### Experiment / Survey / Early-access — M6

| Resource | Subcommands |
|---|---|
| `experiment` | `list`, `get`, `create`, `update`, `delete`, `archive`, `duplicate`, `copy-to-project`, `create-exposure-cohort` |
| `survey` | `list`, `get`, `create`, `update`, `delete`, `activity`, `duplicate`, `archive-response` |
| `early-access` | `list`, `get`, `create`, `update`, `delete` |

### Hog-function / Batch-export / Subscription — M7

| Resource | Subcommands |
|---|---|
| `hog-function` | `list`, `get`, `create`, `update`, `delete`, `enable`, `disable`, `invoke`, `logs`, `metrics`, `enable-backfills` |
| `batch-export` | `list`, `get`, `create`, `update`, `delete`, `pause`, `unpause`, `backfills` {list/create/cancel}, `runs` {list/get/logs/cancel/retry} |
| `subscription` | `list`, `get`, `create`, `update`, `delete`, `test-delivery`, `deliveries` |

### Session-recording / Error-tracking / Role / Capture — M8

| Resource | Subcommands | Safety note |
|---|---|---|
| `session-recording` | `list`, `get`, `update`, `delete` | `get --snapshot` requires `--out <file>`; never stdout |
| `error-tracking` | `fingerprints`, `assignment-rules`, `grouping-rules`, `resolve-github`, `resolve-gitlab` | |
| `role` | `list`, `get`, `create`, `update`, `delete`, `members`, `add-member`, `remove-member` | Enterprise RBAC |
| `capture` | `event`, `batch`, `identify` | **project token** (not personal key); debug-only — use `posthog-rs` for production |

## Shared patterns

- **Pagination**: cursor-style `{count, next, previous, results}`. Auto-follows `next` unless `--no-paginate` or `--limit` set.
- **Soft-delete**: universal across insights/flags/cohorts/annotations/notebooks/subscriptions/hog-functions/actions/error-tracking-fingerprints. Hard DELETE returns 405; CLI normalizes to `PATCH {deleted: true}`.
- **Tags, search, activity, sharing**: consistent flag names — `--tag`, `--search`, `--since`, `--until`.
- **File-based write inputs**: `--filters-file`, `--payload-file`, `--description-file`, `--query-file` for anything JSON-y. Shell escaping is a footgun for agents.

## Safety rules (baked into client)

- HogQL queries without explicit `LIMIT` → auto-append `LIMIT 100` (unless `--no-limit`)
- Session-recording snapshots → `--out <file>` required; explicit error if absent in pipe mode
- LLM trace message bodies (when that resource lands) → summarized by default; `--full` opts in
- Error bodies truncated to 200 chars in debug output
- `https_only(true)` on reqwest client
- `Authorization:` header redacted in debug output

## What's not in v1

See [`v1-scope.md`](v1-scope.md) for the full skip list with rationale. Headline exclusions:

- Notebooks (beta, explicit "breaking changes without announcement")
- Data warehouse external sources (beta, connector churn)
- Legacy plugins (deprecated → hog-function)
- OAuth third-party tokens (`pha_` / `phr_`)
- Full user admin (web UI concern)
- SSE dashboard streaming (wrong transport for a CLI)
- Raw rrweb payload download (via `--out` only; never default stdout)
- Legacy `/decide?v=3` (superseded by `/flags?v=2`)
