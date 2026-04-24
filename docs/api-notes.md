# PostHog API notes

Non-obvious details about PostHog's API that BossHogg has to handle correctly. Authoritative source is [`../research/posthog-api.md`](../research/posthog-api.md); this doc pulls out the surprises.

## Environments vs projects — mid-migration

PostHog is moving from `/api/projects/:project_id/…` to `/api/environments/:env_id/…`. During the migration, many resources are **dual-mounted** — both paths work. A few are only available on one side.

Client rule: **prefer `/api/environments/:env_id/…` when both exist.** The config profile carries both `project_id` and `env_id`; the client picks the right path per resource based on a static route table.

Affected (environment-path preferred): `query`, `endpoints`, `insights`, `dashboards`, `persons`, `events`, `session-recordings`, `hog-functions`, `batch-exports`, `subscriptions`, `groups`.

Affected (project-path only): `feature_flags`, `cohorts`, `actions`, `annotations`, `experiments`, `surveys`, `event_definitions`, `property_definitions`, `error_tracking/*`, `early_access_feature`.

Org-scoped: `organizations`, `roles`.

## Soft-delete is near-universal

The following resources refuse hard `DELETE` (return **405 Method Not Allowed**) and require `PATCH {"deleted": true}`:

- `insights`, `feature_flags`, `cohorts`, `annotations`, `notebooks`, `subscriptions`, `hog_functions`, `actions`, `error_tracking/fingerprints`

Resources that accept hard `DELETE`:

- `persons`, `early_access_feature`, `experiments`, `surveys`, `event_definitions`, `property_definitions`, `session_recordings`, `endpoints`, `batch_exports`, `groups/*` (partial), `roles`

BossHogg client normalizes: `bosshogg <r> delete <id>` always issues a `PATCH` for soft-delete resources. The mapping is a static table in `client/mod.rs`; a test asserts it matches the soft-delete list in [`conventions.md`](conventions.md).

## Events endpoint is deprecated

`GET /api/environments/:env_id/events/` still works but is officially deprecated in favor of HogQL queries and batch exports. It also:

- Caps `offset` at 50,000.
- Caps the time window at 1 year.
- Lacks the flexible filtering of HogQL.

BossHogg routes `bosshogg events list` through an `EventsQuery` via `POST /query/`. An escape hatch (`--direct`) would hit the legacy endpoint; deferred unless someone asks.

## HogQL / Query API is central and has its own rate bucket

- Endpoint: `POST /api/environments/:env_id/query/`.
- Query kinds: `HogQLQuery`, `EventsQuery`, `TrendsQuery`, `FunnelsQuery`, plus more specific ones (retention, paths, stickiness, lifecycle, etc.).
- Supports **sync** (blocking) and **async** (enqueue + poll) execution. Async is recommended for queries that might exceed a few seconds.
- **Separate rate bucket**: 2400/hr. Distinct from the other buckets:
  - Analytics reads: 240/min, 1200/hr
  - CRUD writes: 480/min, 4800/hr
  - Public capture/flag eval: unlimited
- **Rate limits are team-wide, not per-key**. Rotating API keys doesn't help.

Async polling pattern:

1. `POST /query/` with `async_: true` → returns `{query_status: {id: "uuid", ...}}`.
2. `GET /query/:id/` → returns status + `results` when `complete`.
3. `DELETE /query/:id/` → cancel.

BossHogg's `Client::query` wraps this automatically behind `--async`.

## Query response includes column types — use them

A query response looks like:

```json
{
  "results": [[...], [...]],
  "columns": ["event", "count()", "$browser"],
  "types": ["String", "UInt64", "Nullable(String)"],
  "hogql": "SELECT ...",
  "timings": { ... }
}
```

BossHogg emits JSON with the raw arrays; table rendering uses `columns` as headers. Typed rendering (right-align numbers, parse dates) is v1.x polish, not v1.

## Endpoints: materialized HogQL as REST

`/api/environments/:env_id/endpoints/` is a powerful PostHog feature — **named, saved HogQL queries** callable as their own REST endpoints, with a materialization cache and auto-generated OpenAPI specs.

Shape:

- `POST /endpoints/` with `{name, description, query}` → create a named query.
- `GET /endpoints/:name/run/` → execute it (sync or async).
- `POST /endpoints/:name/materialization_preview/` + `GET /materialization_status/` → precompute into ClickHouse for speed.
- `GET /endpoints/:name/openapi.json/` → OpenAPI spec for the endpoint.

`bosshogg endpoints` exposes all of this. Endpoints-as-code workflows (git-versioned YAML definitions) are **already** partly implemented in `@posthog/cli`'s `exp endpoints` — we match naming to stay compatible.

## Notebooks — explicit "unstable" warning

Per PostHog docs: *"The API can have breaking changes without announcement."* Also includes a kernel / code-execution surface that complicates auth and error handling. **Skipped in v1** — see [`v1-scope.md`](v1-scope.md).

## Data warehouse external sources — beta, churny

Connector set changes (new integrations added, schemas evolve, CDC prerequisites shift). Read-only `list` / `get` might be safe eventually; creation and schema management definitely wait.

## Plugins → Hog Functions

`/api/…/plugins/` and `/api/…/plugin_config/` still exist in code but `/docs/api/plugins` returns 404. PostHog has fully migrated the public surface to **Hog Functions** — destinations, transformations, webhooks, Slack/SMS outputs, the whole CDP pipeline.

BossHogg exposes `bosshogg hog-functions`; never `plugins`.

## Feature-flag evaluation: `/flags?v=2`, not `/decide`

- `POST /decide?v=3` is **deprecated**.
- `POST /flags?v=2` is the current evaluation endpoint.
- Uses the **project token** (`phc_…`), not the personal API key.

`bosshogg flags evaluate --distinct-id <id>` uses `/flags?v=2`.

## Capture endpoint: project token, not personal key

`bosshogg capture event/batch/identify` uses:

- `POST /i/v0/e` (single event)
- `POST /batch` (batch events)
- `phc_…` project token in the body as `api_key`, **not** in an `Authorization` header.

This is intentionally a different auth path. Debug/dev use only — for production ingestion, users should reach for `posthog-rs`.

## Authentication — key types at a glance

| Prefix | Type | Used for | CLI uses? |
|---|---|---|---|
| `phx_…` | Personal API key | Admin / query API, `Authorization: Bearer` | **Primary** |
| `phc_…` | Project token | Public capture + `/flags?v=2` evaluation | Only `capture` / `flags evaluate` |
| `phs_…` | Feature flag secure key | Server-side flag overrides | Not used in v1 |
| `pha_…` | OAuth app access token | Third-party integrations | Never — see [`v1-scope.md`](v1-scope.md) |
| `phr_…` | OAuth app refresh token | Integrations | Never |

Personal key scopes matter. PostHog lets you scope a personal key to read-only, write-only, or per-resource access. BossHogg's `whoami` reports the scopes; commands that fail with `403` surface `code: AUTH_SCOPE` with the missing scope name.

## Cloud regions

| Region | Admin/query host | Public ingestion host |
|---|---|---|
| US | `https://us.posthog.com` | `https://us.i.posthog.com` |
| EU | `https://eu.posthog.com` | `https://eu.i.posthog.com` |
| Self-hosted | `$BOSSHOGG_HOST` | `$BOSSHOGG_HOST` |

Keys are region-scoped. A US key won't authenticate against the EU host. BossHogg's profile file stores `host` and `region` together to avoid ambiguity.

## Error response shape

PostHog usually returns structured errors:

```json
{
  "type": "validation_error",
  "code": "required",
  "detail": "This field is required.",
  "attr": "name"
}
```

BossHogg maps these to internal `BosshoggError` variants and then to the user-facing `{error, code, hint, retry_with}` shape documented in [`conventions.md`](conventions.md).

Unauthenticated/permission errors may come back as HTML or an auth middleware page, not JSON. Client must detect content-type before JSON-parsing.

## Pagination

Cursor-style, per the DRF convention PostHog uses:

```json
{
  "count": 124,
  "next": "https://us.posthog.com/api/...?cursor=cHRzOjE1...",
  "previous": null,
  "results": [ ... ]
}
```

The `next` URL is absolute; don't re-prefix the host. Some resources (`groups`) use cursor + limit explicitly without offset support.

## Timezone and time windows

- Responses are UTC (RFC3339).
- Many filter params accept ISO-8601 dates/datetimes. `before` and `after` are inclusive on both ends.
- HogQL queries accept timezone-aware timestamps; server-side defaults to the project's configured timezone.

BossHogg normalizes all displayed times to the user's local timezone in TTY mode and to RFC3339 UTC in JSON mode. Input parsing accepts relative strings (`7d`, `-2h`) via `chrono`/`time`.

## References

- Full API doc: [`../research/posthog-api.md`](../research/posthog-api.md)
- PostHog's own API overview: https://posthog.com/docs/api
- Query API: https://posthog.com/docs/api/query
- Hog Functions: https://posthog.com/docs/api/hog-functions
- Feature flags: https://posthog.com/docs/api/feature-flags
- Endpoints: https://posthog.com/docs/api/endpoints
- Notebooks (with beta warning): https://posthog.com/docs/api/notebooks
