# HogQL reference

All queries here use `bosshogg query run`. For examples see the safe-rollout playbook in `examples/safe-rollout.md`.

## Table of contents

1. [Ground the query in the schema first](#ground-the-query-in-the-schema-first)
2. [Inline, file, or stdin](#inline-file-or-stdin)
3. [Auto-LIMIT behavior](#auto-limit-behavior)
4. [Async queries](#async-queries)
5. [Common recipes](#common-recipes)
6. [Events table cheat sheet](#events-table-cheat-sheet)
7. [Persons and groups](#persons-and-groups)
8. [Sessions](#sessions)
9. [$ai_* LLM observability](#ai_-llm-observability)
10. [Anti-patterns](#anti-patterns)

## Ground the query in the schema first

Before writing any non-trivial HogQL, cache the schema:

```bash
scripts/hogql-schema-dump.sh
```

The script runs `bosshogg schema hogql --json` and writes it to `~/.cache/bosshogg/schema-<project_id>.json`. Re-read the schema after switching projects (`bosshogg use <context>` or a `POSTHOG_CLI_PROJECT_ID` change).

From the cached JSON you can list tables and columns:

```bash
jq '.tables | map(.name)' ~/.cache/bosshogg/schema-999999.json
jq '.tables[] | select(.name=="events") | .columns' ~/.cache/bosshogg/schema-999999.json
```

Event properties are dynamic. The schema dump captures the union of observed keys — real-time property discovery is one `bosshogg query run` away:

```bash
bosshogg query run "SELECT distinct JSONExtractKeysAndValues(properties, 'String') FROM events WHERE event = '\$pageview' LIMIT 1" --json
```

## Inline, file, or stdin

Inline (short queries only — shell escaping gets painful fast):

```bash
bosshogg query run "SELECT count() FROM events" --json
```

File (recommended for anything multi-line):

```bash
bosshogg query run --file queries/funnel.sql --json
```

Stdin:

```bash
cat queries/funnel.sql | bosshogg query run --file - --json
jq -r '.query' build_query.json | bosshogg query run --file - --json
```

## Auto-LIMIT behavior

`bosshogg query run` injects `LIMIT 100` when the parsed query has no `LIMIT`. This prevents accidental `SELECT * FROM events` from flooding context.

- Query already has `LIMIT N` — left alone.
- Query has no `LIMIT` — `LIMIT 100` appended.
- You passed `--no-limit` — nothing injected.
- `--debug` mode reports the injection in a stderr trace line.

If you need more than 100 rows, bump the query-level limit:

```sql
SELECT distinct_id, count() AS n
FROM events
WHERE event = '$pageview' AND timestamp > now() - INTERVAL 1 DAY
GROUP BY distinct_id
ORDER BY n DESC
LIMIT 1000
```

`--no-limit` should be rare — reserve it for bulk exports piped to a file.

## Async queries

Queries that will run more than a few seconds should be dispatched async so the CLI polls instead of holding an HTTP connection:

```bash
bosshogg query run --file big-funnel.sql --async --json
```

Async mode returns the query id immediately, then polls until `complete: true`. The final JSON shape matches the sync shape:

```json
{
  "columns": ["distinct_id", "count"],
  "types": ["String", "Int64"],
  "results": [["user_1", 42], ["user_2", 17]],
  "hogql": "SELECT distinct_id, count() FROM events GROUP BY distinct_id LIMIT 100",
  "query_status": {
    "id": "q_01HRZ...",
    "complete": true,
    "end_time": "2026-04-21T14:33:00Z",
    "elapsed_ms": 4120
  }
}
```

Set a custom poll deadline:

```bash
bosshogg query run --file big-funnel.sql --async --timeout 300 --json
```

## Common recipes

### Pageviews over the last 7 days, per day

```sql
SELECT
  toStartOfDay(timestamp) AS day,
  count() AS n
FROM events
WHERE event = '$pageview'
  AND timestamp > now() - INTERVAL 7 DAY
GROUP BY day
ORDER BY day
```

### Top 20 distinct users by event volume in the last hour

```sql
SELECT distinct_id, count() AS n
FROM events
WHERE timestamp > now() - INTERVAL 1 HOUR
GROUP BY distinct_id
ORDER BY n DESC
LIMIT 20
```

### Recent events for one user

```sql
SELECT timestamp, event, properties.$current_url, properties.$browser
FROM events
WHERE distinct_id = 'user_abc123'
ORDER BY timestamp DESC
LIMIT 50
```

### Signup funnel (pageview → signup_started → signup_completed)

```sql
SELECT
  countIf(event = '$pageview') AS pageviews,
  countIf(event = 'signup_started') AS starts,
  countIf(event = 'signup_completed') AS completions,
  completions / nullIf(starts, 0) AS completion_rate
FROM events
WHERE timestamp > now() - INTERVAL 1 DAY
```

### Error rate by deployment (last 30 min)

Used by the safe-rollout playbook as a guardrail:

```sql
SELECT
  countIf(event = '$exception') AS errors,
  countIf(event != '$exception') AS other_events,
  errors / nullIf(errors + other_events, 0) AS rate
FROM events
WHERE timestamp > now() - INTERVAL 30 MINUTE
```

### P95 latency for a specific endpoint

```sql
SELECT quantile(0.95)(toFloat64(properties.duration_ms)) AS p95_ms
FROM events
WHERE event = '$request_complete'
  AND properties.endpoint = '/api/checkout'
  AND timestamp > now() - INTERVAL 15 MINUTE
```

### Retention cohort (users who did X on day N-7 and again on day N)

```sql
WITH
  day_minus_7 AS (
    SELECT distinct_id FROM events
    WHERE event = 'page_viewed' AND toDate(timestamp) = today() - 7
    GROUP BY distinct_id
  ),
  today_seen AS (
    SELECT distinct_id FROM events
    WHERE event = 'page_viewed' AND toDate(timestamp) = today()
    GROUP BY distinct_id
  )
SELECT count(distinct d7.distinct_id) AS retained
FROM day_minus_7 d7
INNER JOIN today_seen t ON t.distinct_id = d7.distinct_id
```

## Events table cheat sheet

Core columns on `events`:

| Column | Type | Notes |
|---|---|---|
| `event` | String | The event name — `$pageview`, `$autocapture`, custom. |
| `timestamp` | DateTime64 | UTC. |
| `distinct_id` | String | The current user id (may shift over identify). |
| `person_id` | UUID | Stable person id after identify. |
| `session_id` | String | Nullable before identify. |
| `properties` | JSON | Event properties. Access via `properties.foo` or `JSONExtract*(properties, 'foo', 'String')`. |
| `elements_chain` | String | Autocapture element chain — use sparingly. |
| `uuid` | UUID | Event uuid. Useful for deduplication. |

Common reserved properties (prefixed `$`):

- `$pageview`, `$pageleave`, `$autocapture`, `$opt_in`, `$identify`, `$groupidentify`
- `$current_url`, `$host`, `$pathname`
- `$browser`, `$os`, `$device_type`
- `$ip`, `$geoip_city_name`, `$geoip_country_code`
- `$session_id`, `$window_id`

## Persons and groups

`persons` table — one row per person_id, with merged properties:

```sql
SELECT id, distinct_ids, properties.email, created_at
FROM persons
WHERE properties.email = 'aaron@example.com'
```

`groups` (group analytics) — per group type:

```sql
SELECT group_type_index, group_key, properties.name
FROM groups
WHERE group_type_index = 0
  AND group_key IN ('acme-co', 'example-co')
```

## Sessions

`sessions` table (derived) — one row per session:

```sql
SELECT
  session_id,
  min(timestamp) AS started_at,
  max(timestamp) AS ended_at,
  count() AS event_count,
  argMin(properties.$current_url, timestamp) AS entry_url
FROM events
WHERE timestamp > now() - INTERVAL 1 HOUR
GROUP BY session_id
ORDER BY started_at DESC
```

## `$ai_*` LLM observability

PostHog's LLM observability schema emits `$ai_generation`, `$ai_trace`, and `$ai_metric` events. Default summaries only — do not dump full message bodies through `bosshogg query run` unless you have scoped the query tightly.

Cost per trace, last 24 hours:

```sql
SELECT
  properties.$ai_trace_id AS trace_id,
  sum(toFloat64(properties.$ai_total_cost_usd)) AS cost_usd,
  sum(toInt64(properties.$ai_input_tokens)) AS input_tokens,
  sum(toInt64(properties.$ai_output_tokens)) AS output_tokens
FROM events
WHERE event = '$ai_generation'
  AND timestamp > now() - INTERVAL 1 DAY
GROUP BY trace_id
ORDER BY cost_usd DESC
LIMIT 50
```

Failed generations last hour:

```sql
SELECT
  timestamp,
  properties.$ai_trace_id,
  properties.$ai_model,
  properties.$ai_error
FROM events
WHERE event = '$ai_generation'
  AND properties.$ai_is_error = true
  AND timestamp > now() - INTERVAL 1 HOUR
ORDER BY timestamp DESC
LIMIT 50
```

**Do not select message bodies without a `LIMIT 10` or similar.** Prompts can be megabytes.

## Anti-patterns

- **`SELECT *` on events without a `LIMIT`.** The CLI will auto-`LIMIT 100` — fine for exploration, but asking for `SELECT *` signals you do not know what you are looking for. Pick columns explicitly.
- **String-concatenating user input into HogQL.** Use the filters arg or build the query server-side from validated inputs. Injection is a real risk if you are piping from an untrusted source.
- **Ignoring the `types` array in results.** HogQL returns `{"columns": [...], "types": [...], "results": [...]}` — the types tell you whether `2026-04-21` came back as a String or a DateTime. Do not assume.
- **Writing queries without grounding.** Always consult `scripts/hogql-schema-dump.sh` first. Column names like `latency_ms` vs `duration_ms` vary by project.
- **Polling sync queries in a loop.** If a query is slow, use `--async` once; do not re-run it every few seconds.
