# PostHog API Research

Research for `hog` / `bosshogg` — a Rust CLI wrapping the PostHog REST + Query API.
Modeled after the `lin` CLI pattern, but simpler because PostHog is mostly REST
(not GraphQL) with typed JSON responses.

Source: <https://posthog.com/docs/api> and linked resource pages, fetched 2026-04-21.

---

## 1. Authentication

### API key types

| Prefix | Type | Where used |
|---|---|---|
| `phx_` | **Personal API key** | All private `/api/*` endpoints. Up to 10 per user. Scoped. Primary key type for the CLI. |
| `phc_` | **Project API key** (public token) | Only for public POST endpoints (`/i/v0/e`, `/batch`, `/flags`). No auth header needed — passed in body as `api_key`. |
| `phs_` | **Feature flag secure key** | Server-side flag evaluation only. |
| `pha_` / `phr_` | **OAuth access / refresh token** | OAuth app flows. Out of scope for v1 CLI. |

Legacy keys (pre-Feb 2024) use PBKDF2 hashing and can be rolled to SHA256.

### Auth mechanics

Private endpoints accept either:

- `Authorization: Bearer <phx_...>` header (preferred)
- `personal_api_key` field in the request body

**Scopes.** Personal API keys use granular scopes, e.g. `project:read`, `insight:write`,
`feature_flag:read`, `query:read`, `person:write`, `cohort:read`, `dashboard:write`,
`session_recording:read`, `experiment:read`, `survey:read`, `annotation:write`,
`action:read`, `hog_function:write`, `event_definition:read`, `property_definition:read`,
`batch_export:read`, `organization:read`, `user:read`, `notebook:read`,
`external_data_source:read`, `subscription:read`, `activity_log:read`, `group:read`,
`error_tracking:read`, `early_access_feature:read`.

### Regions (base URLs)

| Region | Public (capture/flags) | Private (REST) |
|---|---|---|
| US Cloud | `https://us.i.posthog.com` | `https://us.posthog.com` |
| EU Cloud | `https://eu.i.posthog.com` | `https://eu.posthog.com` |
| Self-hosted | your-domain | your-domain |

CLI config should store `{host, region, api_key, project_id, organization_id}` per
profile. Default region is US.

---

## 2. Conventions

### Pagination

Cursor-style: responses return

```json
{ "count": 1234, "next": "https://.../?cursor=...", "previous": null, "results": [...] }
```

Follow `next` until null. Default page size ~100; some endpoints allow up to 1000.
Some endpoints (groups) use a `cursor` query param explicitly.

Events endpoint caps `offset` at 50 000 and restricts the `before`/`after` window
to 1 year.

### Error shape

```json
{ "type": "error_type", "code": "error_code", "detail": "Human readable", "attr": "field_name" }
```

Status codes: `200/201` success, `204` no-content (delete), `400` validation,
`401` auth, `403` scope, `404` not found, `405` hard-delete refused (many resources
soft-delete via `PATCH {"deleted": true}` — see below), `429` rate-limited,
`503` self-hosted DB unavailable.

### Soft delete

Insights, feature flags, cohorts, annotations, notebooks, subscriptions,
hog functions, error-tracking fingerprints — `DELETE` returns `405`.
Use `PATCH {"deleted": true}` instead. **The CLI should normalize this**: users
run `hog insights delete <id>`, and the client issues the PATCH transparently.

### Rate limits (per team, private endpoints only)

| Bucket | Per minute | Per hour |
|---|---|---|
| Analytics endpoints (insights, dashboards, etc.) | 240 | 1 200 |
| `/query/` (HogQL) | — | 2 400 |
| `/events/values/` | 60 | 300 |
| CRUD (most resources) | 480 | 4 800 |
| Feature flag local evaluation | 600 | — |
| Public POST (`/i/v0/e`, `/batch`, `/flags`) | unlimited | unlimited |

`429` responses include standard `Retry-After`. The CLI client should implement
exponential backoff with jitter on 429/503.

### Environments vs projects

PostHog is transitioning: many resources have **both** `/api/projects/:project_id/...`
and `/api/environments/:environment_id/...` forms. New endpoints prefer
`environments`; legacy routes marked "Deprecated: use /api/environments/{id}/
instead." The CLI should target environments where available, with a
`--project` fallback shim. Concretely:

- `events`, `persons`, `session_recordings`, `query`, `dashboards`, `insights`,
  `hog_functions`, `batch_exports`, `subscriptions`, `groups`, `endpoints`
  → prefer **environments**
- `feature_flags`, `cohorts`, `experiments`, `surveys`, `annotations`,
  `actions`, `event_definitions`, `property_definitions`, `early_access_feature`,
  `notebooks` → currently **projects**
- `organizations/:org_id/projects`, `organizations/:org_id/roles`,
  `organizations/:org_id/integrations` → **organization-scoped**

### OpenAPI

Full spec: `https://<host>/api/schema/` (JSON) or `/api/schema/swagger-ui/` —
requires login. Good reference for generating typed serde models.

Max POST body: **20 MB**.

---

## 3. Resource inventory

Status legend: **GA** = stable; **Beta** = docs say beta/early-access/limited;
**Deprecated** = migrate away.

### 3.1 Organizations — GA

Base: `/api/organizations/`

| Op | Endpoint |
|---|---|
| list | `GET /api/organizations/` |
| get | `GET /api/organizations/:id/` |
| create | `POST /api/organizations/` |
| update | `PATCH /api/organizations/:id/` |
| delete | `DELETE /api/organizations/:id/` |

Nested: `:org_id/integrations/`, `:org_id/roles/`, `:org_id/projects/`,
`:org_id/roles/:role_id/role_memberships/`.

Params: `limit`, `offset`. Writable fields: `name`, `logo_media_id`,
`enforce_2fa`, `members_can_invite`, `members_can_use_personal_api_keys`,
`allow_publicly_shared_resources`, `is_ai_data_processing_approved`,
`default_experiment_stats_method`.

### 3.2 Projects — GA

Base: `/api/organizations/:organization_id/projects/`

Standard CRUD plus:

- `GET /activity/`
- `PATCH /add_product_intent/`
- `POST /change_organization/`
- `PATCH /complete_product_onboarding/`
- `PATCH /reset_token/` — **rotates the public project token**
- `PATCH /rotate_secret_token/`
- `POST /generate_conversations_public_token/`
- `GET /is_generating_demo_data/`

Writable: name, timezone, autocapture flags, session recording settings, surveys
opt-in, heatmap opt-in, data attributes, access control level, test-account filters.

### 3.3 Users — GA

- `GET /api/users/` (filters: `email`, `is_staff`, `limit`, `offset`)
- `GET /api/users/:uuid/` (or `/users/@me/`)
- `PATCH /api/users/:uuid/`
- `DELETE /api/users/:uuid/`
- 2FA subresources: status, setup, backup codes, disable

Useful for CLI `hog whoami`.

### 3.4 Events — **Deprecated** (but still works)

Docs explicitly recommend migrating to **batch exports** for bulk reads.
Keep basic read/filter support in CLI for interactive debugging.

- `GET /api/environments/:env_id/events/`
- `GET /api/environments/:env_id/events/:id/`
- `GET /api/environments/:env_id/events/values/`
- (same paths under `/api/projects/:project_id/events/`)

Filters: `event`, `distinct_id`, `person_id`, `before`, `after`, `properties`,
`limit`, `offset`, `format=csv|json`, `select`, `where`.
Limits: offset ≤ 50 000; window < 1 year; default window = last 24h.

For bigger / flexible queries, use **Query API** instead.

### 3.5 Query (HogQL) — GA (core feature)

This is the **centerpiece** of the CLI. Single endpoint, many query kinds.

- `POST /api/environments/:env_id/query/` — run a query (sync or async)
- `GET /api/environments/:env_id/query/:id/` — fetch async query status/result
- `DELETE /api/environments/:env_id/query/:id/` — cancel async query
- `GET /api/environments/:env_id/query/:id/log/` — execution log (24h retention)
- `POST /api/environments/:env_id/query/check_auth_for_async/`
- `GET /api/environments/:env_id/query/draft_sql/` — turn natural-language-ish into SQL
- `POST /api/environments/:env_id/query/upgrade/` — migrate serialized query to latest schema

Request body:

```json
{
  "query": { "kind": "HogQLQuery", "query": "SELECT event, count() FROM events WHERE timestamp > now() - interval 1 day GROUP BY event" },
  "async": false,
  "refresh": "blocking"
}
```

Query `kind` values (typed unions in serde):
`HogQLQuery`, `EventsQuery`, `PersonsQuery`, `ActorsQuery`, `SessionsTimelineQuery`,
`TrendsQuery`, `FunnelsQuery`, `RetentionQuery`, `PathsQuery`, `StickinessQuery`,
`LifecycleQuery`, `InsightVizNode`, `DatabaseSchemaQuery`, `WebOverviewQuery`,
`WebStatsTableQuery`, `SessionAttributionExplorerQuery`, and experiment / LLM
trace variants.

Response for HogQL:

```json
{ "results": [[...row...]], "columns": ["event","cnt"], "types": ["String","UInt64"], "hasMore": false, "timings": [...] }
```

Async responses include `query_status` with `complete`, `error`, `results`,
`start_time`, `end_time`, progress (bytes read, rows, CPU time).

**Scope:** `query:read`. **Rate limit:** 2 400/hr.

### 3.6 Endpoints (materialized HogQL) — GA but newer

Reusable, named HogQL queries callable as REST endpoints.

- `GET /api/environments/:env_id/endpoints/`
- `POST /api/environments/:env_id/endpoints/`
- `GET/PATCH/DELETE /api/environments/:env_id/endpoints/:name/`
- `POST /api/environments/:env_id/endpoints/:name/materialization_preview/`
- `GET /api/environments/:env_id/endpoints/:name/materialization_status/`
- `GET /api/environments/:env_id/endpoints/:name/openapi.json/`
- `GET /api/environments/:env_id/endpoints/:name/run/` — execute

Useful for the CLI: register a saved SQL query, run it with params.

### 3.7 Insights — GA

- `GET/POST /api/environments/:env_id/insights/`
- `GET/PATCH/DELETE /api/environments/:env_id/insights/:id/`
- `GET /insights/:id/activity/`
- `GET /insights/:id/analyze/`
- `GET/POST /insights/:id/suggestions/`
- `POST /insights/generate_metadata/` (AI-generated names)
- `POST /insights/bulk_update_tags/`
- `POST /insights/cancel/`
- Sharing: `GET/POST /insights/:id/sharing/`,
  `POST/DELETE /insights/:id/sharing/passwords/`,
  `POST /insights/:id/sharing/refresh/`
- Alerts: `GET /insights/:id/thresholds/`, `GET /insights/:id/thresholds/:id/`

List params: `basic`, `format`, `limit`, `offset`, `refresh`, `short_id`.
Create/update fields: `name`, `derived_name`, `query`, `order`, `dashboards`,
`description`, `tags`, `favorited`, `deleted`.

Soft delete.

### 3.8 Dashboards — GA

- `GET/POST /api/environments/:env_id/dashboards/`
- `GET/PATCH/DELETE /api/environments/:env_id/dashboards/:id/`
- Tile management: `POST .../copy_tile/`, `PATCH .../move_tile/`, `POST .../reorder_tiles/`
- `GET .../run_insights/` — refresh all tiles
- `POST .../snapshot/`, `GET .../stream_tiles/` (SSE), `POST .../analyze_refresh_result/`
- `POST .../generate_metadata/`
- `POST /dashboards/bulk_update_tags/`
- `POST /dashboards/create_from_template_json/`
- Sharing subresource (same shape as insights)

Refresh strategies: `blocking`, `force_blocking`, `force_cache`, `async`, `lazy_async`.

### 3.9 Feature flags — GA

Base: `/api/projects/:project_id/feature_flags/`

Standard CRUD (soft delete), plus:

- `GET /:id/activity/`
- `GET /:id/dependent_flags/`
- `POST /:id/create_static_cohort_for_flag/`
- `POST /:id/dashboard/`

List filters: `active` (`STALE`, `true`, `false`), `type`
(`boolean|experiment|multivariant|remote_config`),
`evaluation_runtime` (`client|server|both`), `tags`, `search`.

Evaluation endpoints (public, project token):

- `POST /flags?v=2` — modern endpoint (replaces `/decide`)
- `POST /decide?v=3` — legacy but still live

`/flags` is **public** (POST with `api_key` + `distinct_id` in body), not a
private `/api/` route.

### 3.10 Cohorts — GA

- `GET/POST /api/projects/:project_id/cohorts/`
- `GET/PATCH/DELETE /.../cohorts/:id/` (soft delete)
- `GET /.../cohorts/:id/activity/`
- `GET /.../cohorts/activity/`
- `GET /.../cohorts/:id/persons/` (the members)
- `GET /.../cohorts/:id/calculation_history/`
- `PATCH /.../cohorts/:id/add_persons_to_static_cohort/`
- `PATCH /.../cohorts/:id/remove_person_from_static_cohort/`

Create: `name`, `description`, `is_static`, `filters`, `person_ids`.

### 3.11 Persons — GA (read-recommended, write via capture)

Base: `/api/environments/:env_id/persons/`

- list (`distinct_id`, `email`, `search`, `properties`, `format`, `limit`, `offset`)
- get by id
- `PATCH /:id/` — update person
- `GET /:id/activity/`
- `GET /:id/properties_timeline/`
- `POST /:id/update_property/`, `POST /:id/delete_property/`
- `POST /:id/split/` — undo merges

Docs note: for create/update, prefer the capture API (`$identify`, `$set`,
`$set_once`). The CLI should expose read + delete; writes via a `hog identify`
helper that posts to `/i/v0/e`.

### 3.12 Session recordings — GA

- `GET/PATCH/DELETE /api/environments/:env_id/session_recordings/`
- `GET/PATCH/DELETE /api/environments/:env_id/session_recordings/:id/`
- Sharing subresource

Filters: `distinct_id`, `person`, `limit`, `offset`. Retrieving the actual
recording payload (rrweb snapshots) is a separate streaming endpoint — skip for v1
(large, binary-ish). List + metadata is fine.

### 3.13 Experiments — GA

- Standard CRUD under `/api/projects/:project_id/experiments/`
- `POST /:id/archive/`
- `POST /:id/duplicate/`
- `POST /:id/copy_to_project/`
- `POST /:id/create_exposure_cohort_for_experiment/`

### 3.14 Surveys — GA

- Standard CRUD under `/api/projects/:project_id/surveys/`
- `GET /:id/activity/`
- `GET /:id/archived-response-uuids/`
- `POST /:id/duplicate_to_projects/`
- `POST /:id/responses/:response_uuid/archive/`

List filters: `archived`, `search`, `limit`, `offset`.

### 3.15 Annotations — GA

Standard CRUD (soft delete) under `/api/projects/:project_id/annotations/`.
Fields: `content`, `date_marker`, `scope`, `dashboard_item`, `dashboard_id`,
`creation_type`.

### 3.16 Actions — GA

Standard CRUD (soft delete) under `/api/projects/:project_id/actions/`, plus
`GET /:id/references/` and `POST /actions/bulk_update_tags/`.

### 3.17 Event definitions — GA

- Standard CRUD under `/api/projects/:project_id/event_definitions/`
- `GET /:id/metrics/`
- `POST /bulk_update_tags/`
- `GET /by_name/`
- `GET /golang/`, `/python/`, `/typescript/` — language-specific listings

### 3.18 Property definitions — GA

- `GET /api/projects/:project_id/property_definitions/`
  (filters: `event_names`, `type` = `event|person|group|session`, `search`, `limit`, `offset`)
- `GET /:id/`, `PATCH /:id/`, `DELETE /:id/`
- `POST /bulk_update_tags/`
- `GET /seen_together/` — check event↔property co-occurrence

Writable: `description`, `tags`, `property_type`, `verified`, `hidden`.

### 3.19 Groups — GA (if group analytics enabled)

Base: `/api/environments/:env_id/groups/` (and projects variant)

- `GET /groups/` (needs `group_type_index`; cursor pagination)
- `POST /groups/`
- `GET /groups/find/`
- `GET /groups/property_definitions/`, `/property_values/`
- `POST /groups/update_property/`, `/delete_property/`
- `GET /groups/related/`
- `GET /groups/activity/`

### 3.20 Hog functions — GA (new-ish, Oct 2024 launch → stable)

PostHog's modern CDP destination / transformation / SMS / Slack / webhook system.
This is the **webhook story** now (plus more). Replaces the old plugins API.

- `GET/POST /api/environments/:env_id/hog_functions/`
- `GET/PATCH/DELETE /api/environments/:env_id/hog_functions/:id/` (soft delete)
- `POST /:id/invocations/` — test-run with fake event payload
- `POST /:id/enable_backfills/`
- `GET /:id/logs/`
- `GET /:id/metrics/` (params: `interval` hour/day/week, `breakdown`)

Filters: `type` (destination/transformation/source/etc.), `enabled`,
`created_at`, `updated_by`, `search`.

### 3.21 Batch exports — GA

- `GET/POST /api/environments/:env_id/batch_exports/`
- `GET /:id/`
- Backfills subresource: list/create, `POST .../backfills/:id/cancel/`
- Runs subresource: list, get, `GET /:id/logs/`, `POST /:id/cancel/`, `POST /:id/retry/`

Intervals, destinations (S3, BigQuery, Snowflake, Postgres, Redshift, HTTP),
HogQL filters supported. Replaces the old `events` export pattern.

### 3.22 Subscriptions — GA

Scheduled delivery of dashboards/insights via email, Slack, webhook.

- `GET/POST/PATCH/DELETE /api/environments/:env_id/subscriptions/`
- `POST /:id/test-delivery/`
- `GET /:id/deliveries/` (premium)

Soft delete.

### 3.23 Roles — GA (enterprise)

- `GET/POST/PATCH/DELETE /api/organizations/:org_id/roles/`
- Nested `/role_memberships/`

### 3.24 Early access features — GA

Small resource for managing opt-in feature program.
CRUD under `/api/projects/:project_id/early_access_feature/`.

### 3.25 Capture / batch (public) — GA

- `POST /i/v0/e` — single event
- `POST /batch` — array of events (≤ 20 MB)
- Must include `api_key` (project token), `distinct_id`, `event`, optional
  `properties`, `timestamp`. Auth via project token in body, no Bearer header.

CLI use: `hog capture <event> --property k=v` for debugging.

### 3.26 Flags evaluation (public) — GA

- `POST /flags?v=2` (modern) or `POST /decide?v=3` (legacy)
- Body: `api_key`, `distinct_id`, `groups`, `person_properties`, `group_properties`
- Returns evaluated flags keyed by name, plus variants

### 3.27 Notebooks — **Early access / Beta**

Docs warn: "The API can have breaking changes without announcement."
18 endpoints including kernel / code execution (Jupyter-like). **Skip for v1.**

### 3.28 Data warehouse / external data sources — Beta

- `GET/POST /api/environments/:env_id/external_data_sources/`
- CRUD on sources, schema refresh/bulk-update, webhook mgmt, sync jobs, reload
- `GET /.../database_schema/`
- CDC prerequisites validation

Status: actively evolving, schemas change. Read-only list/get **might** be safe
in v1; creation and schema management should be out. Connectors (Stripe, Salesforce,
Hubspot, Ashby, etc.) added regularly.

### 3.29 Error tracking — Beta (no explicit banner but recent)

- Assignment rules: list/create/get/update/delete/reorder
- Fingerprints: list/get (soft delete only)
- Grouping rules: list/create/get
- `POST /resolve_github/`, `/resolve_gitlab/` — source link resolution

Status: no "beta" label in docs, but feature is newer (2024). Reasonable to
include basic list/get.

### 3.30 LLM observability

Referenced in PostHog's feature list but no dedicated public API docs page
(404 on `/docs/api/llm-observability`). Traces are captured as events
(`$ai_generation`, `$ai_trace`) and queried via the Query API / HogQL. No
dedicated CRUD endpoints beyond events. **Skip as a separate resource.**

### 3.31 Plugins (legacy apps) — **Deprecated → Hog functions**

No current `/docs/api/plugins` or `/docs/api/plugin-config` page (both 404).
Plugins framework superseded by hog functions. **Skip for v1.**

---

## 4. What the CLI actually needs (summary)

**Must-have (v1):**

1. `query` — run HogQL, table/JSON output. Central workflow.
2. `flags` — list/get/toggle/rollout-percentage.
3. `insights` — list/get (by `short_id`), run (via `refresh`), delete.
4. `dashboards` — list/get/refresh.
5. `persons` — list/search/get/delete.
6. `cohorts` — list/get/members.
7. `events` — list/tail (acknowledging it's deprecated; wrap over `/query/`
   with an `EventsQuery` under the hood where possible).
8. `projects` / `orgs` / `whoami` — context switching.
9. `capture` — fire a test event (public endpoint).
10. `annotations` — list/create (handy for release marks).
11. `actions`, `event-definitions`, `property-definitions` — metadata lookups,
    useful for agent discovery.
12. `experiments` — list/get/results.
13. `surveys` — list/get.
14. `session-recordings` — list/get metadata (not payloads).
15. `hog-functions` — list/get/enable/disable/invoke (test).

**Nice-to-have (v1.x):** batch-exports, subscriptions, endpoints (saved HogQL),
groups, roles, early-access-features, error-tracking (read-only).

**Skip for v1:** notebooks (beta), data-warehouse source management
(beta, schema churn), plugins (deprecated), OAuth tokens, user admin
(`PATCH /users/`, 2FA), dashboard SSE streaming, session recording payload download,
LLM observability as a separate resource.

---

## 5. Notable surprises

- **Events API is deprecated.** Use the Query API (HogQL `SELECT ... FROM events`)
  or batch exports. The CLI should funnel "event search" through HogQL.
- **HogQL is GA and unified.** Every analytics surface in PostHog is implementable
  on top of `/api/environments/:env_id/query/`. Makes the client much simpler.
- **Environments vs projects split is mid-migration.** New endpoints use
  environments; many legacy ones still project-scoped. CLI needs both resolvers.
- **Soft delete is near-universal.** Client should translate `DELETE` → `PATCH
  {"deleted": true}` transparently.
- **Hog functions replaced plugins entirely.** Webhooks, destinations, and
  transforms all live here.
- **`/flags` replaced `/decide`.** Use `v=2` for flag evaluation in new code;
  `/decide?v=3` still works but legacy.
- **Notebooks API is explicitly unstable** — warned in their own docs. Skip.
- **Data warehouse endpoints exist and are broad** but the feature is still
  moving fast; safe surface is limited.
- **Rate limits are team-wide**, not per-user/per-key. The CLI must surface
  `429` clearly to avoid stepping on a team's other tooling.
