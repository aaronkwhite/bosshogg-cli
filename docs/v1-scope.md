# V1 scope

The explicit in/out list for BossHogg's first shippable release. Pulled from [`../research/capability-schema.yaml`](../research/capability-schema.yaml); kept in sync by hand. If you're about to add a resource, update both files.

## In scope — v1 resources

All resources below are generally available in PostHog's API, have stable shapes, and are operationally useful from a terminal or agent loop.

### Core / query

| Resource | Command group | Why it's in |
|---|---|---|
| **Query (HogQL)** | `bosshogg query` | Central command. Nearly every analytics use case routes through `POST /api/environments/:env_id/query/`. |
| **Endpoint** | `bosshogg endpoint` | Saved HogQL queries with CRUD, run, materialize preview, and an OpenAPI export — pairs naturally with `query`. (M5) |

### Identity / config

| Resource | Command group | Why it's in |
|---|---|---|
| **Contexts** | `bosshogg config set-context/use-context`, `bosshogg use`, `bosshogg configure` | Multi-project / multi-region kubectl-style named contexts. |
| **Org** | `bosshogg org` | List + switch. Create/delete deferred. |
| **Project** | `bosshogg project` | List + switch + reset-token. Create/delete deferred. |
| **whoami** | `bosshogg whoami` | `/users/@me/` — confirms auth and current project. |

### Product surface

| Resource | Command group | Why it's in |
|---|---|---|
| **Flag** | `bosshogg flag` | The biggest gap in `@posthog/cli`. CRUD, enable/disable, rollout %, evaluate, dependents, activity. **CRUD-deep in M1.** |
| **Insight** | `bosshogg insight` | CRUD, refresh, tag, activity, share. (M3) |
| **Dashboard** | `bosshogg dashboard` | CRUD, refresh, tiles, share, snapshot. SSE streaming skipped. (M3) |
| **Cohort** | `bosshogg cohort` | CRUD, members, static add/remove, calculation history. (M3) |
| **Experiment** | `bosshogg experiment` | CRUD, archive, duplicate, copy-to-project, create-exposure-cohort. (M6) |
| **Survey** | `bosshogg survey` | CRUD, activity, duplicate, archive-response. (M6) |

### Events & properties

| Resource | Command group | Why it's in |
|---|---|---|
| **Event** | `bosshogg event` | Routes through HogQL (legacy REST endpoint is deprecated). `list`, `get`, `values`, `tail`. (M4) |
| **Event definition** | `bosshogg event-definition` | List/update/delete + bulk tag + metrics. (M5) |
| **Property definition** | `bosshogg property-definition` | Same shape as event definitions; `seen-together` is useful for agents. (M5) |
| **Action** | `bosshogg action` | CRUD + references + bulk tag. (M4) |
| **Annotation** | `bosshogg annotation` | CRUD. Release markers etc. (M4) |
| **Capture** | `bosshogg capture` | Public endpoint, debug-only. Project token auth (not personal key). (M8) |

### People

| Resource | Command group | Why it's in |
|---|---|---|
| **Person** | `bosshogg person` | Read + delete + property edits; writes happen via capture. (M4) |
| **Group** | `bosshogg group` | Group analytics — accounts/companies/teams as first-class entities. (M4) |

### Operational

| Resource | Command group | Why it's in |
|---|---|---|
| **Session recording** | `bosshogg session-recording` | Metadata only — list, get, update, delete. rrweb payload download requires `--out`. (M8) |
| **Hog function** | `bosshogg hog-function` | Modern CDP pipeline — destinations/transformations/webhooks. Replaces legacy plugins. (M7) |
| **Batch export** | `bosshogg batch-export` | Scheduled exports to S3/BigQuery/Snowflake/Postgres. CRUD + pause/unpause + backfills + runs. (M7) |
| **Subscription** | `bosshogg subscription` | Scheduled delivery of dashboards/insights to email/Slack/webhook. (M7) |
| **Role** | `bosshogg role` | Enterprise RBAC. CRUD + membership. (M8) |
| **Early access** | `bosshogg early-access` | Early-access feature program CRUD. (M6) |
| **Error tracking** | `bosshogg error-tracking` | Fingerprints (read), assignment + grouping rules, github/gitlab source resolution. (M8) |

## Out of scope — v1 skip list

Each entry here has a concrete reason and a rough re-evaluation plan.

### Notebooks — **beta, explicit instability warning**

PostHog's own docs warn: *"The API can have breaking changes without announcement."* Includes a kernel / code-execution surface that's complicated and moving. Also Claude Code + agents have better spaces to compose a narrative than a PostHog Notebook.

*Re-evaluate:* once the beta banner is removed from `/docs/api/notebooks` and the shape settles for a full release cycle.

### Data warehouse external sources — **beta, connector churn**

The external-data-sources API (Stripe, Salesforce, HubSpot, Ashby, etc.) is broad but evolving — new connectors, schema changes, CDC prerequisites. Too much moving surface for v1.

*Re-evaluate:* read-only `list` / `get` might be safe earlier. Creation + schema management wait until the ecosystem stabilizes.

### Plugins — **deprecated**

Superseded by hog functions. `/docs/api/plugins` returns 404.

*Re-evaluate:* never. Plugins are gone.

### Legacy `/decide?v=3` endpoint — **deprecated**

Superseded by `POST /flags?v=2`. `bosshogg flags evaluate` wires up the new endpoint only.

*Re-evaluate:* never.

### Direct `/events/` REST endpoint — **soft-deprecated**

PostHog docs point users to batch exports or HogQL for bulk event access; the endpoint still works but caps offset at 50 000 and windows at one year. `bosshogg events list` accepts the filter flags a user expects but tunnels the query through HogQL.

*Re-evaluate:* `bosshogg event list --direct` escape hatch, if anyone needs it.

### LLM observability (`$ai_generation`, `$ai_trace`) — **no dedicated endpoints**

PostHog doesn't expose CRUD endpoints for LLM traces; they're just events with a known property convention. Agents can query them with HogQL through `bosshogg query`. No separate command group needed.

*Re-evaluate:* when/if PostHog ships first-class LLM-observability endpoints (not just conventions over events).

### OAuth / third-party app tokens (`pha_`, `phr_`) — **wrong audience**

Those tokens exist for OAuth integrations between PostHog and other apps, not for personal CLI use. We stick to personal API keys.

*Re-evaluate:* if/when PostHog ships a CLI-auth OAuth flow scoped to a single personal machine, revisit. Browser-based login (like `@posthog/cli` has) is a separate feature.

### Full user admin — **UI concern**

`PATCH /api/users/:id/`, 2FA management, session token rotation — these belong in the web UI. BossHogg keeps read-only `whoami`.

*Re-evaluate:* never for the settings surface; possibly `bosshogg whoami --scopes` expansion later.

### Dashboard SSE streaming (`/stream_tiles/`) — **wrong transport for a CLI**

Server-Sent Events are designed for a long-lived browser subscription. A one-shot CLI invocation gets no benefit. `bosshogg dashboards refresh` uses the REST `run_insights` endpoint instead.

*Re-evaluate:* if a TUI mode ships, reconsider.

### Session-recording payloads (rrweb) — **not an agent workflow**

Downloading the raw rrweb snapshot is large, binary-adjacent, and useless outside a replay player. Metadata is covered by the `session-recordings` resource.

*Re-evaluate:* if a user requests it for pipeline export, consider a targeted subcommand.

## Explicit feature exclusions (not resource-level)

- **Chart rendering.** The PostHog MCP server can't render charts either (an open issue). We return URLs and JSON; we don't paint. Users reach for the web app or the MCP server when they need a picture.
- **OAuth / browser login in v1.** `@posthog/cli` does this nicely and we may adopt it later. v1 uses personal API keys + env vars + `configure`. See [`conventions.md`](conventions.md).
- **MCP server mode (`bosshogg mcp --stdio`) is a v1.0 M9 stretch goal.** Designed for — the same binary would expose stdio MCP over the same auth + JSON contracts. Likely slips to v1.1 if the plumbing isn't clean.
- **Homebrew formula.** In scope but expected in v1.1 — crates.io comes first.

## Budget

Rough target, pulled from the `lin` playbook comparables:

- **~6,000–8,000 lines** of Rust across commands + client + output. PostHog has more resources than Linear but typed REST responses are terser than Linear's GraphQL query strings.
- **~18–22 command files** (one per resource).
- **40–55 subcommands** at v1. (Linear `lin` shipped with 67; PostHog has a wider resource set but we're cutting deeper on v1 scope.)
- **Single binary, 4 targets** at release (linux x86/arm, macOS x86/arm), 4 distribution channels (crates.io, Homebrew, GitHub Releases, source). Identical to the `lin` playbook.
