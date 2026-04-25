# API coverage

Maintainer-facing reference for what bosshogg covers vs what PostHog's REST API exposes vs what the PostHog MCP server exposes. Use this when triaging "should we add X?" — most "missing" verbs are intentional skips with a documented auth-class reason.

For the canonical bosshogg surface, see [`capabilities.md`](capabilities.md). This file is the comparison view.

## 1. Snapshot versions

- **bosshogg** — `Cargo.toml` v2026.4.6 (28 GA resources, ~232 verbs incl. meta)
- **PostHog OpenAPI schema** — fetched 2026-04-24 from `https://us.posthog.com/api/schema/`, cached at `/tmp/ph-schema.yaml` (948 paths)
- **PostHog MCP** — `github.com/PostHog/mcp` at commit `13aaf2c6e5317e01e61d3af24e7b0744f527ed3e` (main, 2026-01-19), `schema/tool-definitions.json` (44 tools)

Refresh instructions live in section 7.

## 2. At a glance

| Surface | Count | Notes |
|---|---|---|
| PostHog REST paths | 948 | grouped into ~60 top-level resources |
| PostHog MCP tools | 44 | curated agent-friendly subset |
| bosshogg resources | 28 | GA Personal-API-Key-accessible |
| bosshogg verbs (resource) | ~219 | not counting meta |
| bosshogg verbs (meta) | 13 | configure, whoami, doctor, schema, auth, config, use, completion, version |
| MCP tools covered by bosshogg | 40 / 44 | 91% parity |
| MCP-only tools | 4 | inventoried in section 4 |
| bosshogg-only verbs vs MCP | ~170 | inventoried in section 5 |

Auth-boundary breakdown of the gap (verbs PostHog REST exposes but bosshogg deliberately omits): vast majority are `personal-api-key-OK` (closeable gaps), with a documented minority of `session-only`, `paid-only`, and `project-token-only` exclusions. See section 6.

## 3. Coverage matrix — by resource

Endpoint counts come from `/api/schema/` grouped by leading resource segment after stripping `/api`, `/environments/{id}`, `/projects/{id}`, `/organizations/{id}`. MCP tool count is from the manifest. bosshogg verb count from `capabilities.md`.

### Resources bosshogg covers

| Resource | REST endpoints | MCP tools | bosshogg verbs | Coverage status |
|---|---|---|---|---|
| flag | 19 | 6 | 11 | Full |
| insight | 36 | 6 | 9 | Partial — see notes |
| dashboard | 36 | 6 | 9 | Partial — see notes |
| cohort | 8 | 0 | 10 | Full |
| person | 40 | 0 | 7 | Partial — see notes |
| event | 6 | 0 | 4 | Full (via HogQL) |
| event-definition | 8 | 1 | 5 | Partial — see notes |
| property-definition | 4 | 1 | 5 | Full |
| experiment | 17 | 6 | 9 | Partial — see notes |
| survey | 13 | 5 | 13 | Partial — see notes |
| hog-function | 20 | 0 | 11 | Full |
| batch-export | 40 | 0 | 15 | Full |
| session-recording | 12 | 0 | 4 | Read-only + soft-delete |
| error-tracking | 56 | 2 | ~21 (incl. nested issues group) | Partial — see notes |
| role | 4 | 0 | 8 | Full |
| org | 36 | 3 | 4 | Read-only + switch |
| project | 212 (root) | 2 | 5 | Read-only + reset-token |
| action | 4 | 0 | 7 | Full |
| annotation | 2 | 0 | 5 | Full |
| early-access | 2 | 0 | 5 | Full |
| endpoint | 16 | 0 | 9 | Full |
| group | 16 + 7 (types) | 0 | 8 | Full |
| query | 14 | 3 | 8 | Full (central command) |
| capture | (ingest) | 0 | 3 | Full |
| alert | 6 | 0 | 5 | Full |
| dashboard-template | 4 | 0 | 4 | Full (no dedicated "use" endpoint in spec; uses dashboard create wrapper) |
| session-recording-playlist | 8 | 0 | 8 | Full |
| insight-variable | 4 | 0 | 5 | Full |

### Resources bosshogg deliberately omits

| Resource | REST endpoints | MCP tools | Recommendation | Reason |
|---|---|---|---|---|
| notebooks | 15 | 0 | Skip — beta | Vendor flagged "breaking changes without announcement" |
| external_data_sources | 32 | 0 | Skip — beta | Connector churn; warehouse beta |
| external_data_schemas | 14 | 0 | Skip — beta | Same as above |
| warehouse_saved_queries | 24 | 0 | Skip — beta | Warehouse beta |
| warehouse_tables | 10 | 0 | Skip — beta | Warehouse beta |
| llm_analytics | 33 | 1 | Add — v1.x candidate | `$ai_*` event surface; bosshogg's positioning leans agent-first |
| llm_skills | 8 | 0 | Skip — internal | PostHog Max-internal product |
| llm_prompts | 5 | 0 | Skip — internal | PostHog Max-internal product |
| max_tools | 1 | 0 | Skip — internal | PostHog Max-internal |
| mcp_server_installations | 9 | 0 | Skip — internal | PostHog's own MCP catalog |
| mcp_servers / mcp_tools | 2 | 0 | Skip — internal | Same |
| integrations | 45 | 0 | Skip — web-UI affordance | OAuth flows / external service connectors |
| hog_flows | 22 | 0 | Skip — beta | Successor to legacy plugins; not GA |
| logs | 31 | 0 | Skip — beta | Logs product not GA via API |
| tasks | 23 | 0 | Skip — internal | Internal workflow engine |
| visual_review | 15 | 0 | Skip — internal | Web-UI feature |
| file_system | 18 | 0 | Skip — web-UI affordance | Project tree state |
| conversations | 14 | 0 | Skip — internal | Max conversation transcripts |
| product_tours | 7 | 0 | Skip — web-UI affordance | Onboarding tours |
| ~~alerts~~ | 6 | 0 | ~~Add — v1.x candidate~~ **Implemented in v2026.4.6** | |
| subscriptions | 8 | 0 | Skip — paid-only | Removed in v2026.4.2 (HTTP 402) |
| ~~session_recording_playlists~~ | 8 | 0 | ~~Add — v1.x candidate~~ **Implemented in v2026.4.6** | |
| app_metrics | 8 | 0 | Skip — replaced | Legacy plugin metrics → covered by `hog-function metrics` |
| elements | 8 | 0 | Skip — niche | Autocapture element queries; rarely useful from CLI |
| ~~dashboard_templates~~ | 4 | 0 | ~~Add — v1.x candidate~~ **Implemented in v2026.4.6** | |
| heatmaps | 4 | 0 | Skip — web-UI affordance | Heatmap rendering is visual |
| ~~insight_variables~~ | 4 | 0 | ~~Add — v1.x candidate~~ **Implemented in v2026.4.6** | |
| comments | 4 | 0 | Skip — web-UI affordance | Per-resource discussion threads |
| change_requests | 5 | 0 | Skip — paid-only | Approval workflows (Teams plan) |
| approval_policies | 2 | 0 | Skip — paid-only | Same |
| advanced_activity_logs | 3 | 0 | Skip — paid-only | Enterprise audit log |
| oauth_applications | 1 | 0 | Skip — web-UI affordance | OAuth app registration is admin-UI work |
| project_secret_api_keys | 6 | 0 | Skip — admin-UI affordance | Key rotation already covered narrowly via `project reset-token` |

## 4. Gap inventory (per-gap detail)

Each gap is a verb or endpoint bosshogg doesn't expose. Grouped by recommendation, sorted within each group with read paths first.

### 4a. Add — v1.x candidates

| Verb / endpoint | Source | Auth class | Reason |
|---|---|---|---|
| `dashboard tiles add (existing-insight binding)` — equivalent of MCP `add-insight-to-dashboard` | MCP | personal-api-key-OK | bosshogg's `dashboard tiles add` is the same call but the parity gap with MCP is naming; consider renaming or aliasing for transferability. |
| ~~`error-tracking issues list`~~ — equivalent of MCP `list-errors` | MCP | personal-api-key-OK | **Closed in v2026.4.5** as `error-tracking issues list`. Maps to `GET /api/environments/{proj}/error_tracking/issues/`. |
| ~~`error-tracking issues get`~~ (issue detail with stack/breadcrumbs) — equivalent of MCP `error-details` | MCP | personal-api-key-OK | **Closed in v2026.4.5** as `error-tracking issues get <id>`. |
| `query nl-to-hogql` — equivalent of MCP `query-generate-hogql-from-question` | MCP | unknown — needs probe | Live-dogfood removed `query draft-sql` in v2026.4.3 (session-only). MCP exposes a different path (Max). Probe `/api/projects/:id/max_tools/` before re-attempting. |
| ~~`survey stats <id>`~~ and ~~`survey project-stats`~~ — equivalents of MCP `survey-stats` / `surveys-global-stats` | MCP | personal-api-key-OK | **Closed in v2026.4.5** as `survey stats <id>` and `survey project-stats`. |
| ~~`experiment results <id>`~~ — equivalent of MCP `experiment-results-get` | MCP | personal-api-key-OK | **Closed in v2026.4.4** — wraps `/experiments/{id}/timeseries_results/?metric_uuid=...`. |
| ~~`llm-analytics costs --since`~~ — equivalent of MCP `get-llm-total-costs-for-project` | MCP | personal-api-key-OK | **Closed in v2026.4.4** as `query ai-costs --since <Nd>` (HogQL aggregate over `$ai_generation`). |
| `llm-analytics list / generations / traces` | REST | personal-api-key-OK | 33 endpoints; positions bosshogg for the `$ai_*` use case. Scope a minimal subset first. |
| ~~`alert list / get / create / update / delete`~~ | REST | personal-api-key-OK | **Closed in v2026.4.6.** Hard DELETE (204). Path: `/api/projects/{proj}/alerts/`. |
| ~~`session-recording-playlist list / get / create / update / delete`~~ | REST | personal-api-key-OK | **Closed in v2026.4.6.** Plus `recordings`, `add-recording`, `remove-recording`. Uses `{short_id}` in URL per spec. |
| ~~`dashboard-template list / get / create / use`~~ | REST | personal-api-key-OK | **Closed in v2026.4.6.** No dedicated "use/instantiate" endpoint in OpenAPI spec; `use` verb wraps `POST /dashboards/` with `use_template=<id>`. DELETE returns 405 (soft-delete via PATCH). |
| ~~`insight-variable list / get / create / update / delete`~~ | REST | personal-api-key-OK | **Closed in v2026.4.6.** Hard DELETE. Path: `/api/projects/{proj}/insight_variables/`. |
| `query-tab-state` (saved query history) | REST | unknown — needs probe | Useful for agent context recall; verify auth. |
| `docs-search` — equivalent of MCP `docs-search` | MCP | n/a (public) | Wraps PostHog docs search. Trivial; consider as a `bosshogg help search` or skill-side instead of CLI verb. |

### 4b. Skip — auth-blocked (session-only)

Documented for posterity. Removed in v2026.4.2 / v2026.4.3 after live 403 dogfood.

| Verb | Removed in | Reason |
|---|---|---|
| `dashboard tiles move` | v2026.4.2 | `move_tile` endpoint is session-cookie auth only. |
| `dashboard tiles copy` | v2026.4.2 | `copy_tile` session-cookie only. |
| `dashboard tiles reorder` | v2026.4.2 | `reorder_tiles` session-cookie only. |
| `event-definition metrics` | v2026.4.2 | Session-cookie only. |
| `event-definition tag add/remove` | v2026.4.3 | Session-cookie only. |
| `property-definition tag add/remove` | v2026.4.3 | Session-cookie only. |
| `query draft-sql` | v2026.4.3 | NL→HogQL helper; PostHog gates server-side. |

### 4c. Skip — paid feature (HTTP 402)

| Resource / verb | Auth class | Reason |
|---|---|---|
| `subscription *` (entire resource) | paid-only | PostHog Teams/Enterprise; removed in v2026.4.2. |
| `change_requests` | paid-only | Approval workflows. |
| `approval_policies` | paid-only | Same. |
| `advanced_activity_logs` | paid-only | Enterprise audit log. |

### 4d. Skip — web-UI affordance

| Resource | Reason |
|---|---|
| `integrations` (45 endpoints) | OAuth flows, external service connectors — not CLI-shaped. |
| `file_system` | Project tree state for the web UI. |
| `comments` | Per-resource discussion threads. |
| `heatmaps` | Visual rendering. |
| `oauth_applications` | Admin UI registration. |
| `product_tours` | Onboarding UI. |
| `desktop_recordings`, `visual_review`, `customer_journeys`, `customer_profile_configs`, `user_interviews`, `dataset_items`, `datasets`, `signals` | All web-UI / internal product surfaces. |

### 4e. Skip — beta or internal

| Resource | Reason |
|---|---|
| `notebooks` | Vendor flagged "breaking changes without announcement". |
| `external_data_sources`, `external_data_schemas`, `warehouse_*` | Warehouse beta; connector churn. |
| `hog_flows` | Successor to legacy plugins; not GA. |
| `logs` | Logs product not GA via API. |
| `tasks` | Internal workflow engine. |
| `llm_skills`, `llm_prompts`, `max_tools`, `conversations` | PostHog Max internal product surface. |
| `mcp_server_installations`, `mcp_servers`, `mcp_tools` | PostHog's own MCP catalog. |

## 5. bosshogg-only capabilities

Things bosshogg ships that MCP doesn't. (Roughly 150 verbs delta; this is the categorical summary.)

- **HogQL execution as a first-class command** — `query run`, `query hogql`, `query events|trends|funnel`, `query status|cancel|log`. MCP exposes `query-run` and `insight-query` only.
- **Full hog-function CRUD + ops** — `enable`, `disable`, `invoke`, `logs`, `metrics`, `enable-backfills`. MCP has zero hog-function tools.
- **Full batch-export coverage** — 15 verbs across `create/update/delete/pause/unpause` + nested `backfills` and `runs`. MCP: zero.
- **Full role / RBAC management** — `add-member`, `remove-member`, role CRUD. MCP: zero.
- **Endpoint (materialized HogQL) management** — including `materialize-preview` and `openapi`. MCP: zero.
- **Group-types and group property mutations** — MCP: zero.
- **Cohort membership ops** — `add-person`, `remove-person`, `members`, `calculation-history`. MCP: zero.
- **Person property mutations and split** — `update-property`, `delete-property`, `split`, `activity`. MCP: zero.
- **Action references and tags** — MCP: zero.
- **Annotation, early-access, capture, error-tracking nested resources** — MCP: zero.
- **Multi-context auth** — `bosshogg config set-context / use-context`, `bosshogg use <name>`. Kubectl/gh-style.
- **Agent operability** — `bosshogg doctor` (preflight), `bosshogg schema hogql` (grounded HogQL for LLMs), `bosshogg auth token` (escape hatch), structured error envelope, JSON-first output with stable schemas, deterministic exit codes, `--limit`/`--no-paginate` for context-budget control.
- **Soft-delete normalization** — universal `PATCH {deleted: true}` routing where REST returns 405 on hard DELETE.

The prior MCP-coverage analysis enumerated this in finer detail. This doc treats it as the high-level summary.

## 6. Auth-boundary categories (persistent reference)

Four classes of endpoint, derived from live dogfood. When probing a new endpoint, classify it before wiring the verb.

| Class | Token | Status code on mismatch | Examples | bosshogg policy |
|---|---|---|---|---|
| Personal API Key OK | `phx_…` | n/a | Vast majority of GA endpoints | Default. Wire it. |
| Session-cookie only | (browser cookie) | 403 | `dashboard tiles move/copy/reorder` (removed v2026.4.2), `event-definition tag` (removed v2026.4.3), `property-definition tag` (removed v2026.4.3), `query draft-sql` (removed v2026.4.3) | Skip. Document. |
| Paid feature | `phx_…` (auth OK, plan check fails) | 402 | `subscription *` (removed v2026.4.2), `change_requests`, `approval_policies`, `advanced_activity_logs` | Skip. Document. |
| Project token only | `phc_…` (not `phx_`) | 401 / 403 | `flag evaluate` (uses `POST /flags?v=2`), `capture event/batch/identify` | Wire it but route through the project-token resolution path, not the personal-key path. |

**Probe protocol** before adding any new verb:

1. `bosshogg auth token` to grab the active personal key.
2. `curl -sS -o /dev/null -w "%{http_code}\n" -H "Authorization: Bearer $TOKEN" "$HOST/api/projects/$PROJECT_ID/<endpoint>"` for read paths, or POST a minimal payload for writes.
3. If 200/201: `personal-api-key-OK`. Wire it.
4. If 403: `session-cookie only`. Skip with comment.
5. If 402: `paid-only`. Skip with comment.
6. If 401 with a `phc_`-shaped error: `project-token only`. Route via project token resolution.

## 7. Re-running this analysis

When to re-run:

- Every minor `YYYY.M.0` release of bosshogg.
- Whenever PostHog ships a new resource (watch their changelog or `/api/schema/` diff).
- Whenever the PostHog MCP repo's `schema/tool-definitions.json` changes.

### Refresh the OpenAPI schema

```bash
set -a; source .env.local; set +a
curl -sS "https://us.posthog.com/api/schema/" \
  -H "Authorization: Bearer $POSTHOG_API_KEY" \
  -o /tmp/ph-schema.yaml
wc -l /tmp/ph-schema.yaml
```

### Refresh the MCP tool manifest

```bash
gh api repos/PostHog/mcp/git/refs/heads/main --jq '.object.sha'
gh api repos/PostHog/mcp/contents/schema/tool-definitions.json --jq '.content' \
  | base64 -d > /tmp/mcp-tools.json
python3 -c "import json; d=json.load(open('/tmp/mcp-tools.json')); print(len(d), 'tools'); [print(k) for k in sorted(d)]"
```

### Source-of-truth for the bosshogg surface

`docs/capabilities.md` and `research/capability-schema.yaml` are the human + machine views. Keep them in sync; they drive section 3 above.

### Prior baseline analysis

The first MCP-coverage pass (which produced the 11-MCP-only / 155-bosshogg-only baseline) lives in the maintainer-only research directory. This doc supersedes it and refreshes the snapshot to 2026-04-23.
