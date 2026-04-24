# Cross-product playbooks

Playbooks are decision trees that compose `bosshogg` verbs (and occasionally MCP calls) to execute common workflows spanning multiple PostHog products.

---

## Playbook 1: Safe feature flag rollout

**Use when:** the user wants to ship a new feature behind a flag with guardrails against error-rate or latency regressions.

**Enabling milestones:** M1 (flags, HogQL) — complete.

### Decision tree

```
1. Flag exists?
   ├─ No  → create at 0% disabled (step 1.a)
   └─ Yes → inspect with `bosshogg flag get <key> --json` (step 1.b)

2. Enable + tiny rollout (1%) → step 2

3. Monitor guardrails for 15–30 min → step 3
   ├─ Guardrails clean → step 4 (ramp)
   └─ Guardrail breach → step 5 (rollback)

4. Ramp 10% → monitor → 50% → monitor → 100% → step 6 (declare done)

5. Rollback: `bosshogg flag update <key> --disabled --yes --json`

6. Declare done: annotate (M4+), announce.
```

### Step 1.a: Create the flag at 0% disabled

Put the filter tree in a file so shell escaping stays sane:

```json
// filters.json
{
  "groups": [
    {
      "properties": [],
      "rollout_percentage": 0,
      "variant": null
    }
  ],
  "multivariate": null,
  "payloads": {}
}
```

```bash
bosshogg flag create \
  --key my-new-feature \
  --name "My New Feature" \
  --description "New checkout UI, rolling out safely" \
  --type boolean \
  --filters /tmp/filters.json \
  --disabled \
  --json
```

### Step 1.b: Inspect existing flag

```bash
bosshogg flag get my-new-feature --json | jq '{active, rollout_percentage, filters}'
```

Confirm:
- `active: false` (disabled)
- `rollout_percentage: 0` (no users exposed yet)
- `filters: []` (no targeting rules, open to all)

If the flag exists with active rollout, this playbook is not the right fit — contact the feature owner to agree on a ramp plan.

### Step 2: Enable + 1% rollout

```bash
bosshogg flag update my-new-feature --enabled --rollout 1 --json
```

Confirm the response shows `active: true` and `rollout_percentage: 1`.

### Step 3: Monitor guardrails for 15–30 min

The guardrails are:

1. **Error rate**: `(errors / total) < 2%` over the last 15 min.
2. **P95 latency**: `p95(duration_ms) < baseline * 1.1` (10% slower is acceptable).
3. **4xx error rate** (client bugs): < 1%.

Run each query and note the results:

#### Query 1: Error rate

```sql
SELECT
  countIf(event = '$exception') AS errors,
  countIf(event != '$exception') AS other_events,
  errors / (errors + other_events) AS error_rate
FROM events
WHERE timestamp > now() - INTERVAL 15 MINUTE
```

Target: `error_rate < 0.02` (2%).

#### Query 2: P95 latency

Replace `endpoint_name` and baseline with your app's real values:

```sql
SELECT quantile(0.95)(toFloat64(properties.duration_ms)) AS p95_ms
FROM events
WHERE event = '$request_complete'
  AND timestamp > now() - INTERVAL 15 MINUTE
  AND properties.endpoint = '/api/checkout'
```

Target: `p95_ms < 300` (adjust to your baseline).

#### Query 3: 4xx errors

```sql
SELECT count() AS client_errors
FROM events
WHERE event = '$exception'
  AND timestamp > now() - INTERVAL 15 MINUTE
  AND properties.status_code >= 400 AND properties.status_code < 500
```

Target: `client_errors < (total_requests * 0.01)` (less than 1% of traffic).

Run all three and confirm they pass. If all three guardrails are green, proceed to step 4. If any breach, go to step 5.

### Step 4: Ramp → 10% → 50% → 100%

Once guardrails are clean, ramp the flag in three steps:

**Ramp to 10%:**

```bash
bosshogg flag update my-new-feature --rollout 10 --json
# Wait 15–30 min, re-run guardrail queries from step 3
```

**Ramp to 50%:**

```bash
bosshogg flag update my-new-feature --rollout 50 --json
# Wait 30 min (larger traffic jump), re-run guardrail queries
```

**Ramp to 100%:**

```bash
bosshogg flag update my-new-feature --rollout 100 --json
# Final guardrail check, then move to step 6
```

### Step 5: Rollback

If any guardrail breaches during step 3 or 4, disable immediately:

```bash
bosshogg flag update my-new-feature --disabled --yes --json
```

Confirm response shows `active: false`. Then:

1. Investigate the breach (check deployment logs, error logs, recent code changes).
2. Fix the root cause.
3. Re-run the playbook starting from step 2 (enable + 1% again).

Do not retry at higher percentages without understanding the breach.

### Step 6: Declare done

Flag is at 100% and guardrails have held for 30+ min. Announce:

- Which teams/channels (Slack, email).
- Feature name, key, and rollout completion time.
- Link to the flag in PostHog: `https://us.posthog.com/project/<pid>/feature_flags/<key>`.
- (M4+) Leave an annotation in the flag's activity log for audit trail.

---

## Playbook 2: Debug a specific user

**Use when:** a user reports an issue or you need to inspect why a user saw or did not see a specific feature.

**Enabling milestones:** M1 (HogQL, persons stub) → M4 (full person table).

### Decision tree

```
1. Identify the user → get distinct_id or email

2. Look up the person
   ├─ Found → inspect properties (M4+)
   └─ Not found → check distinct_id is correct

3. Fetch user's recent events

4. Check if user matches flag filters (if applicable)

5. Fetch flag variants the user saw (M4+)

6. (Optional) inspect session replay (M8+), error traces (M8+)
```

### Step 1: Identify the user

Ask the user for any of: email, user id, account uuid, phone. Map it to their `distinct_id` in your app:

- If your app has distinct_id standardization (e.g., `user_123`), use it directly.
- If they gave you an email (`aaron@example.com`), you may need to query for the distinct_id first:

```sql
SELECT distinct distinct_ids
FROM persons
WHERE properties.email = 'aaron@example.com'
LIMIT 5
```

Pick the most recent distinct_id if multiple appear.

### Step 2: Look up the person (M4+ feature)

```bash
bosshogg person get aaron@example.com --json
```

Or by distinct_id:

```bash
bosshogg person get user_123 --json
```

Output includes `properties` (merged from all identify calls), `created_at`, and `distinct_ids`.

For M1, `bosshogg person get` is stubbed; use HogQL instead:

```sql
SELECT id, distinct_ids, properties
FROM persons
WHERE distinct_ids[1] = 'user_123'
```

### Step 3: Fetch user's recent events

```sql
SELECT
  timestamp,
  event,
  properties.$current_url,
  properties.$browser
FROM events
WHERE distinct_id = 'user_123'
ORDER BY timestamp DESC
LIMIT 100
```

Scan the list for the event you're debugging. Note the timestamp and any unusual properties.

### Step 4: Check if user matches flag filters

If the issue is "user didn't see feature X", check the flag's filter rules:

```bash
bosshogg flag get my-feature --json | jq '.filters'
```

Against the person's properties from step 2 or 3:

```json
{
  "properties": [
    {
      "key": "plan",
      "operator": "exact",
      "value": ["pro"]
    }
  ]
}
```

Does `properties.plan == "pro"` in the user's data? If not, the flag is correctly filtering them out. If yes but they still do not see the feature, move to step 5.

### Step 5: Fetch flag variants the user saw

(M4+ feature — for M1, skip or check your app's local flag evaluation logs.)

```bash
bosshogg flag evaluate my-feature --distinct-id user_123 --json
```

Output:

```json
{
  "flag_key": "my-feature",
  "distinct_id": "user_123",
  "is_enabled": true,
  "variant": "treatment_v1"
}
```

- If `is_enabled: false`, the user does not match the flag's rollout or filters. Re-check step 4.
- If `is_enabled: true` and `variant: null` (boolean flag), the user matches — check the app's event log to see if the code ran.
- If `is_enabled: true` and `variant: "treatment_v1"`, the user was assigned the variant. Check the app for variant-specific code.

### Step 6: Inspect session replay and errors (M8+)

If the user reported a UI crash, error, or unexpected behavior:

**Session replay** (M8+):

```bash
bosshogg session-recording list --distinct-id user_123 --json
```

Pick the session matching the report's timestamp. View in the web UI: `https://us.posthog.com/project/<pid>/session_recordings/<session_id>`.

**Error tracking** (M8+):

```bash
bosshogg error-tracking fingerprints list --distinct-id user_123 --json
```

Inspect individual error fingerprints and traces.

---

## Playbook 3: Why did conversion drop?

**Use when:** a key conversion metric (sign-ups, purchases, trial activations) has dipped and you need to find where in the funnel users are falling off and why.

**Enabling milestones:** M3 (insights, cohorts) + M4 (persons, events, actions, annotations) + M8 (session-recording, error-tracking).

### Decision tree

```
1. Confirm the top-line drop via a trend insight (analytics)

2. Narrow with a funnel insight — find the worst drop-off step

3. Break down that step by property (country, plan, device) to isolate the segment

4. Build a cohort of users who reached step N but not step N+1

5. Check activity log — any flag/experiment changes in the window?

6. Pull replays of drop-off sessions for visual evidence

7. Check error tracking for spikes in the same window
```

### Step 1: Confirm the top-line drop

```bash
# Run a trend query for your conversion event (e.g. 'signed_up')
bosshogg query run "
  SELECT
    toStartOfDay(timestamp) AS day,
    count() AS conversions
  FROM events
  WHERE event = 'signed_up'
    AND timestamp > now() - INTERVAL 30 DAY
  GROUP BY day
  ORDER BY day
" --json | jq '.results'
```

Cross-check with the saved insight if one already exists:

```bash
# List insights to find your conversion metric dashboard
bosshogg insight list --search "conversion" --json | jq '.[].name'
bosshogg insight get <short_id> --json | jq '{name, last_refresh}'
```

### Step 2: Funnel breakdown — find the drop-off step

Locate or create the funnel insight. If it already exists:

```bash
bosshogg insight get <funnel_short_id> --json | jq '.result[].action_id, .result[].count'
```

If you need ad-hoc funnel data via HogQL:

```sql
SELECT
  step_reached,
  count() AS users
FROM (
  SELECT
    distinct_id,
    maxIf(1, event = 'page_view')      AS s1,
    maxIf(1, event = 'signup_started') AS s2,
    maxIf(1, event = 'signup_completed') AS s3,
    s1 + s2 + s3 AS step_reached
  FROM events
  WHERE timestamp > now() - INTERVAL 14 DAY
  GROUP BY distinct_id
)
GROUP BY step_reached
ORDER BY step_reached
```

```bash
bosshogg query run --file /tmp/funnel.sql --json | jq '.results'
```

Identify the step with the steepest count drop — that is your target.

### Step 3: Break down by segment property

Once you know the failing step (e.g. step 2→3), break it by a property:

```bash
bosshogg query run "
  SELECT
    properties.\$country AS country,
    countIf(event = 'signup_completed') AS converted,
    countIf(event = 'signup_started')   AS started,
    round(converted / started, 3)       AS rate
  FROM events
  WHERE event IN ('signup_started', 'signup_completed')
    AND timestamp > now() - INTERVAL 14 DAY
  GROUP BY country
  ORDER BY rate ASC
  LIMIT 20
" --json | jq '.results'
```

Try other dimensions: `properties.$browser`, `properties.plan`, `properties.$os`, `properties.$device_type`.

### Step 4: Build a cohort of drop-off users

Create a static cohort of users who started but did not complete the failing step. This enables replay sampling and targeted messaging.

```bash
# First build the cohort filter JSON
cat > /tmp/dropoff-cohort.json <<'EOF'
{
  "groups": [
    {
      "match_type": "AND",
      "properties": [
        {"key": "signup_started", "type": "behavioral", "value": "performed", "event_type": "events", "time_value": 14, "time_interval": "day"},
        {"key": "signup_completed", "type": "behavioral", "value": "not_performed", "event_type": "events", "time_value": 14, "time_interval": "day"}
      ]
    }
  ]
}
EOF

bosshogg cohort create \
  --name "Signup Drop-offs — last 14d" \
  --description "Users who started signup but did not complete — built $(date -I)" \
  --filters /tmp/dropoff-cohort.json \
  --json
```

Note the `id` from the response for downstream steps.

```bash
# Verify membership
bosshogg cohort members <cohort_id> --json | jq 'length'
```

### Step 5: Check the activity log for flag/experiment changes

Flag rollouts and experiment assignments often cause step-change drops. Check what changed in the window:

```bash
# Recent annotation changes (human-created events)
bosshogg annotation list --json | jq '[.[] | select(.date_marker > "2026-05-01")] | .[].content'

# Check any recent flag changes via activity query
bosshogg query run "
  SELECT
    timestamp,
    properties.flag_key,
    properties.action,
    distinct_id
  FROM events
  WHERE event = '\$feature_flag_called'
    AND timestamp > now() - INTERVAL 14 DAY
  GROUP BY timestamp, properties.flag_key, properties.action, distinct_id
  LIMIT 50
" --json | jq '.results'
```

If a specific flag key correlates with the drop timing, inspect it:

```bash
bosshogg flag get <flag_key> --json | jq '{name, active, rollout_percentage, filters}'
```

### Step 6: Pull replays of drop-off sessions

Sample 5–10 session recordings from the drop-off cohort:

```bash
bosshogg session-recording list \
  --filter "distinct_id_in_cohort=<cohort_id>" \
  --json | jq '.[0:10] | .[].id'
```

For each session ID, open in the web UI to watch:

```
https://us.posthog.com/project/<pid>/session_recordings/<session_id>
```

Look for: UI freezes, rage clicks, broken form fields, redirect loops, blank screens.

### Step 7: Check error tracking for correlated spikes

```bash
bosshogg error-tracking fingerprints list --json | \
  jq '[.[] | select(.volume > 10)] | sort_by(.volume) | reverse | .[0:5] | .[].fingerprint'
```

For each high-volume fingerprint, check if the spike aligns with the conversion drop window:

```bash
bosshogg error-tracking fingerprints get <fingerprint> --json | \
  jq '{fingerprint, first_seen, last_seen, volume, sample_event}'
```

**Footnotes:**
- If the funnel insight doesn't exist yet, create one via `bosshogg insight create --filters-file <file>` (the insight type is set inside the filters JSON via `"insight": "FUNNELS"`) and let it run one refresh cycle.
- Cohort calculation is async; if `members` returns 0, wait 60 seconds and retry — PostHog queues the calculation.
- Session recordings are only available for sessions that opted into recording; if the cohort has members but no recordings, check the recording configuration.

---

## Playbook 4: Ship a tracking event

**Use when:** shipping a new user interaction (button click, form submit, API call, background job) and you need to define the event, capture it from the SDK, and wire it to a dashboard.

**Enabling milestones:** M3 (insights, dashboards, cohorts) + M4 (events, actions, annotations) + M5 (event-definitions, property-definitions) + M7 (hog-functions for CDP).

### Decision tree

```
1. Agree on the naming convention and write the spec

2. Capture from the SDK (or bosshogg capture for server-side testing)

3. Create the event-definition record (taxonomy)

4. Create property-definitions for key properties

5. Build a trend insight to confirm volume

6. Add the insight to the relevant dashboard

7. Create a cohort of users who fired the event

8. (Optional) route to a CDP destination via hog-function
```

### Step 1: Naming convention + spec

BossHogg follows PostHog's recommended naming pattern: `object_action` in snake_case.

| Layer | Example |
|-------|---------|
| Page view (auto) | `$pageview` |
| Object + action | `checkout_started` |
| Object + action + qualifier | `checkout_started_guest` |

Write a spec file (keep it in your repo at `analytics/events/<event_name>.md`):

```
Event: checkout_started
Description: User clicked "Start Checkout" on the cart page.
Properties:
  - cart_value (number): total cart value in cents
  - item_count (number): number of distinct SKUs
  - checkout_type (string): "guest" | "returning" | "express"
  - $current_url (auto-captured)
Triggered by: CartPage onCheckoutClick handler
```

### Step 2: Capture the event from code and test with bosshogg

After wiring the SDK call in your codebase, use `bosshogg capture` to fire a test event without needing a browser:

```bash
printf '{"cart_value": 4999, "item_count": 2, "checkout_type": "returning"}' > /tmp/checkout-props.json
bosshogg capture event \
  --event checkout_started \
  --distinct-id "test-user-$(date +%s)" \
  --properties-file /tmp/checkout-props.json \
  --yes \
  --json
```

Confirm it landed:

```bash
bosshogg query run "
  SELECT timestamp, distinct_id, properties
  FROM events
  WHERE event = 'checkout_started'
  ORDER BY timestamp DESC
  LIMIT 5
" --json | jq '.results'
```

### Step 3: Create the event-definition (taxonomy)

The event-definition gives the event a display name and description in the PostHog UI, and enables verification in the data management tab.

```bash
bosshogg event-definition by-name checkout_started --json
# → If found, note the `id` and update; if 404, the event hasn't been seen yet
#   (it will be created automatically once an event fires; update after first capture)

# Once seen, update the definition
bosshogg event-definition update <event_def_id> \
  --name "Checkout Started" \
  --description "User clicked Start Checkout on the cart page. Includes cart_value, item_count, and checkout_type." \
  --json
```

Tag for discoverability:

```bash
bosshogg event-definition tag <event_def_id> --add checkout --json
bosshogg event-definition tag <event_def_id> --add funnel --json
```

### Step 4: Create property-definitions

```bash
# Numeric property
bosshogg property-definition list --type event --search "cart_value" --json | jq '.[].id'

bosshogg property-definition update <prop_def_id> \
  --name "Cart Value (cents)" \
  --description "Total cart value in cents at checkout start." \
  --json

# Enum / string property
bosshogg property-definition update <checkout_type_def_id> \
  --name "Checkout Type" \
  --description "guest | returning | express" \
  --json
```

### Step 5: Build a trend insight to confirm volume

```bash
# The insight type goes inside the filters JSON (e.g. "insight": "TRENDS").
# `bosshogg insight create` has no --type flag; the kind is encoded in filters.
cat > /tmp/checkout-insight.json <<'EOF'
{
  "name": "Checkout Started — daily volume",
  "description": "Trend of checkout_started events, last 30 days.",
  "filters": {
    "insight": "TRENDS",
    "events": [{"id": "checkout_started", "name": "Checkout Started", "type": "events", "order": 0}],
    "display": "ActionsLineGraph",
    "date_from": "-30d"
  }
}
EOF

bosshogg insight create \
  --filters-file /tmp/checkout-insight.json \
  --json | jq '{id, short_id, name}'
```

### Step 6: Add the insight to your dashboard

```bash
# Find the right dashboard
bosshogg dashboard list --json | jq '.[] | {id, name}'

# Add a tile (use --insight for the insight numeric id; no --color flag)
bosshogg dashboard tiles add <dashboard_id> \
  --insight <insight_id> \
  --json
```

Leave an annotation noting the event launch date:

```bash
bosshogg annotation create \
  --content "checkout_started event live — wired in CartPage" \
  --date-marker "$(date -I)" \
  --json
```

### Step 7: Create a cohort of converters (optional but recommended)

```bash
cat > /tmp/checkout-cohort.json <<'EOF'
{
  "groups": [
    {
      "match_type": "AND",
      "properties": [
        {"key": "checkout_started", "type": "behavioral", "value": "performed", "event_type": "events", "time_value": 30, "time_interval": "day"}
      ]
    }
  ]
}
EOF

bosshogg cohort create \
  --name "Checkout Starters — last 30d" \
  --filters /tmp/checkout-cohort.json \
  --json | jq '.id'
```

### Step 8: Route to a CDP destination (optional)

If this event should flow to a downstream CDP (e.g. Segment, Amplitude, Braze):

```bash
# List available hog-function templates
bosshogg hog-function list --json | jq '.[] | select(.type == "destination") | {id, name}'

# Create a destination function triggered by checkout_started.
# Required: --name and --template-id. Use --inputs-file for template inputs.
# (There is no --enabled or --hog-file flag; enable separately after create.)
bosshogg hog-function create \
  --name "checkout_started → Amplitude" \
  --template-id <amplitude_template_id> \
  --inputs-file /path/to/amplitude_inputs.json \
  --json

# Then enable it:
bosshogg hog-function enable <new_function_id> --yes --json
```

**Footnotes:**
- Event definitions are auto-created on first ingestion; allow 60–120 seconds for the PostHog ingestion pipeline to populate them before running `by-name`.
- Property definitions are auto-created per event type when first seen. Run `property-definition list --type event --search <name>` after 2 minutes to confirm.
- `bosshogg capture` uses the project token (`phc_...`), not the personal API key. Confirm `bosshogg config current-context` has `project_token` set.

---

## Playbook 5: Debug an LLM app (AI observability)

**Use when:** an LLM feature is exhibiting high cost, latency, or unexpected outputs and you need to identify the root cause — slow traces, expensive generations, linked exceptions, or a specific prompt version.

**Enabling milestones:** M1 (HogQL) + M4 (events, error-tracking) + M8 (session-recording, error-tracking).

### Decision tree

```
1. List expensive or failing AI traces via HogQL on $ai_generation / $ai_trace

2. Drill into a specific trace timeline

3. Resolve the linked exception (if any) to get a stack trace

4. Pull the session replay for the user who hit the bad trace

5. Identify the prompt version in use at the time

6. Create an eval to prevent the regression
```

### Step 1: List expensive or failing AI traces

Find generations that exceeded your cost or latency threshold:

```bash
bosshogg query run "
  SELECT
    distinct_id,
    properties.\$ai_trace_id,
    properties.\$ai_latency     AS latency_s,
    properties.\$ai_total_cost_usd AS cost_usd,
    properties.\$ai_model,
    properties.\$ai_http_status,
    timestamp
  FROM events
  WHERE event = '\$ai_generation'
    AND timestamp > now() - INTERVAL 7 DAY
    AND (
      toFloat64OrZero(properties.\$ai_latency) > 5
      OR toFloat64OrZero(properties.\$ai_total_cost_usd) > 0.01
      OR toInt32OrZero(properties.\$ai_http_status) >= 400
    )
  ORDER BY cost_usd DESC
  LIMIT 20
" --json | jq '.results'
```

Find failed traces (non-200 HTTP status or explicit error flag):

```bash
bosshogg query run "
  SELECT
    properties.\$ai_trace_id,
    properties.\$ai_http_status,
    properties.\$ai_error,
    properties.\$ai_model,
    timestamp,
    distinct_id
  FROM events
  WHERE event = '\$ai_generation'
    AND timestamp > now() - INTERVAL 2 DAY
    AND toInt32OrZero(properties.\$ai_http_status) >= 400
  ORDER BY timestamp DESC
  LIMIT 50
" --json | jq '.results'
```

### Step 2: Drill into a specific trace timeline

Pick a `$ai_trace_id` from step 1 and reconstruct the full span timeline:

```bash
export TRACE_ID="<trace_id_from_step_1>"

bosshogg query run "
  SELECT
    timestamp,
    event,
    properties.\$ai_span_name,
    properties.\$ai_latency,
    properties.\$ai_input_tokens,
    properties.\$ai_output_tokens,
    properties.\$ai_total_cost_usd,
    properties.\$ai_http_status
  FROM events
  WHERE properties.\$ai_trace_id = '${TRACE_ID}'
  ORDER BY timestamp
" --json | jq '.results'
```

This reconstructs the full trace as an ordered list of spans — you can see exactly where latency or cost is concentrated.

### Step 3: Resolve the linked exception

If the trace includes a failed span or the user reported an error, check error tracking:

```bash
# Find errors matching this user / timeframe
bosshogg error-tracking fingerprints list --json | \
  jq '.[] | select(.last_seen > "2026-05-01") | {fingerprint, volume, last_seen}'

# Inspect a specific fingerprint
bosshogg error-tracking fingerprints get <fingerprint> --json | \
  jq '{fingerprint, first_seen, last_seen, sample_event}'
```

Cross-reference the distinct_id from the trace with error events:

```bash
bosshogg query run "
  SELECT
    timestamp,
    properties.\$exception_type,
    properties.\$exception_message,
    properties.\$exception_stack_trace_raw
  FROM events
  WHERE event = '\$exception'
    AND distinct_id = '<user_distinct_id>'
    AND timestamp > now() - INTERVAL 24 HOUR
  ORDER BY timestamp DESC
  LIMIT 10
" --json | jq '.results'
```

### Step 4: Pull the session replay

Find and open the recording for the affected user:

```bash
bosshogg session-recording list \
  --distinct-id "<user_distinct_id>" \
  --json | jq '.[0:5] | .[].{id, start_time, end_time}'
```

Open in the web UI:

```
https://us.posthog.com/project/<pid>/session_recordings/<session_id>
```

Seek to the timestamp of the failed generation to watch exactly what the user experienced.

### Step 5: Identify the prompt version

Check the event properties for prompt version metadata (if your app captures it):

```bash
bosshogg query run "
  SELECT
    properties.\$ai_trace_id,
    properties.\$ai_model,
    properties.prompt_version,
    properties.prompt_template_id,
    timestamp
  FROM events
  WHERE event = '\$ai_generation'
    AND properties.\$ai_trace_id = '${TRACE_ID}'
  LIMIT 1
" --json | jq '.results[0]'
```

If `prompt_version` is missing from your events, this is an instrumentation gap — add it to your SDK capture call alongside the generation.

### Step 6: Create an eval to prevent regression

Once you understand the failure mode, create an eval entry to detect it in future prompt changes. Document it in your eval file and open an issue or PR:

```bash
# Record the bad prompt + response as a test case
cat >> evals/llm-regressions.json <<'EOF'
{
  "trace_id": "<TRACE_ID>",
  "model": "claude-opus-4-7",
  "prompt_version": "v2.3",
  "failure_mode": "hallucinated_function_name",
  "expected": "only call tools listed in the system prompt",
  "actual": "called nonexistent tool 'get_user_balance'",
  "added": "2026-05-01",
  "ticket": "https://github.com/org/repo/issues/42"
}
EOF
```

**Footnotes:**
- `$ai_generation` and `$ai_trace` are PostHog's LLM observability event names. Your SDK must be configured with `posthog-ai` or equivalent instrumentation for these to appear.
- If `$ai_total_cost_usd` is missing, your SDK version may be older. Check `posthog-ai` docs for the minimum version that captures cost.
- Trace IDs are user-defined; ensure your app passes the same `trace_id` across all spans in a single LLM request.

---

## Playbook 6: Incident notebook

**Use when:** an incident is in progress or just resolved and you need to assemble the evidence — errors, deployment marker, session replays, and log snippets — into a shareable notebook for postmortem review.

**Enabling milestones:** M3 (insights) + M4 (annotations) + M8 (session-recording, error-tracking) + subscription (M7) for sharing.

### Decision tree

```
1. Identify the error fingerprint(s) driving the incident

2. Locate the deployment annotation marking the deploy that caused it

3. Pull 3 representative session replays from the incident window

4. Extract log snippets via HogQL

5. Assemble the notebook (markdown file or PostHog notebook)

6. Share via subscription or direct link
```

### Step 1: Identify the error fingerprint(s)

```bash
bosshogg error-tracking fingerprints list --json | \
  jq '[.[] | select(.volume > 50)] | sort_by(.volume) | reverse | .[0:5]'
```

For each fingerprint, get its details:

```bash
bosshogg error-tracking fingerprints get <fingerprint> --json | \
  jq '{fingerprint, volume, first_seen, last_seen, sample_event}'
```

Check if an assignment rule exists (did anyone claim it?):

```bash
bosshogg error-tracking assignment-rules list --json | \
  jq '.[] | select(.fingerprint == "<fingerprint>")'
```

### Step 2: Locate the deployment annotation

```bash
bosshogg annotation list --json | \
  jq '[.[] | select(.date_marker >= "2026-05-01" and .date_marker <= "2026-05-02")] | .[].{date_marker, content}'
```

If the annotation doesn't exist yet, create it now (even retroactively):

```bash
bosshogg annotation create \
  --content "Deploy v3.4.2 — rolled out checkout-v2 feature flag to 100%" \
  --date-marker "2026-05-01T14:30:00" \
  --json
```

### Step 3: Pull 3 representative session replays

```bash
# Get sessions from the incident window that had errors
bosshogg query run "
  SELECT DISTINCT
    properties.\$session_id,
    distinct_id,
    min(timestamp) AS session_start
  FROM events
  WHERE event = '\$exception'
    AND timestamp BETWEEN '2026-05-01 14:00:00' AND '2026-05-01 18:00:00'
    AND properties.\$session_id IS NOT NULL
  GROUP BY properties.\$session_id, distinct_id
  ORDER BY session_start ASC
  LIMIT 10
" --json | jq '.results[0:3]'
```

Then fetch and note each recording:

```bash
for SESSION_ID in <id1> <id2> <id3>; do
  bosshogg session-recording get "${SESSION_ID}" --json | \
    jq '{id, start_time, end_time, distinct_id, viewed}'
done
```

Recording URLs for the notebook:

```
https://us.posthog.com/project/<pid>/session_recordings/<id1>
https://us.posthog.com/project/<pid>/session_recordings/<id2>
https://us.posthog.com/project/<pid>/session_recordings/<id3>
```

### Step 4: Extract log snippets

Pull server-side error events with stack traces:

```bash
bosshogg query run "
  SELECT
    timestamp,
    distinct_id,
    properties.\$exception_type,
    properties.\$exception_message,
    left(properties.\$exception_stack_trace_raw, 500) AS stack_top
  FROM events
  WHERE event = '\$exception'
    AND timestamp BETWEEN '2026-05-01 14:00:00' AND '2026-05-01 18:00:00'
  ORDER BY timestamp
  LIMIT 20
" --json | jq '.results' > /tmp/incident-errors.json
```

### Step 5: Assemble the notebook

Create a markdown notebook in your postmortem repo:

```bash
cat > /tmp/incident-$(date +%Y%m%d).md <<'EOF'
# Incident Notebook — 2026-05-01 Checkout Failure

## Summary
- **Incident window:** 2026-05-01 14:30 – 18:00 UTC
- **Severity:** P1 — 100% checkout failure rate
- **Root cause:** Deploy v3.4.2 enabled checkout-v2 flag at 100%; race condition in cart lock

## Error fingerprints
| Fingerprint | Volume | First seen | Last seen |
|-------------|--------|-----------|-----------|
| `<fp1>`     | 1,204  | 14:31      | 17:58     |
| `<fp2>`     | 88     | 14:45      | 16:22     |

## Deployment marker
`Deploy v3.4.2` annotated at 14:30 UTC (PostHog annotation id: <id>)

## Session replays
1. [User A session](<replay_url_1>) — blank checkout screen
2. [User B session](<replay_url_2>) — infinite spinner on payment step
3. [User C session](<replay_url_3>) — JS exception during cart hydration

## Log snippet
```json
<paste from /tmp/incident-errors.json>
```

## Timeline
- 14:30 — Deploy v3.4.2 shipped
- 14:31 — First $exception events logged
- 14:45 — PagerDuty alert fired
- 15:02 — Feature flag rolled back via `bosshogg flag update checkout-v2 --disabled --yes`
- 18:00 — Error rate returned to baseline

## Action items
- [ ] Fix race condition in CartLock (ticket #42)
- [ ] Add pre-deploy guardrail for checkout error rate
- [ ] Update safe-rollout playbook to include error-rate check before 100% ramp
EOF
```

### Step 6: Share the notebook

Email or Slack the postmortem link. Optionally create a PostHog subscription to distribute the key insight:

```bash
bosshogg subscription create \
  --insight-id <error_trend_insight_id> \
  --target-type email \
  --target-value "oncall@example.com,cto@example.com" \
  --frequency weekly \
  --json
```

Test delivery:

```bash
bosshogg subscription test-delivery <subscription_id> --yes --json
```

**Footnotes:**
- Recording exports (video/blob) require `--out <file>` to avoid stdout pollution. The snapshot blob is suppressed by default.
- Annotations are project-scoped; if your incident spans multiple projects, create annotations in each.
- If the error fingerprint volume is still increasing, do NOT proceed to postmortem — stabilize first.

---

## Playbook 7: GDPR deletion (right to erasure)

**Use when:** a user submits a GDPR Article 17 (right to erasure) request and you need to fully delete them from PostHog, purge their cohort membership, and verify no data remains.

**Enabling milestones:** M3 (cohorts) + M4 (persons) + M8 (session-recording, error-tracking).

**Warning:** person deletion in PostHog is a hard delete. It cannot be undone. All events associated with the `distinct_id` remain in ClickHouse (PostHog stores events separately from persons) but the person record, merged IDs, and properties are permanently removed.

### Decision tree

```
1. Locate the person by email → get distinct_id

2. Hard-delete the person record (--yes required)

3. Verify deletion via activity log

4. Remove from any static cohorts where they are an explicit member

5. Verify flag evaluation no longer resolves for this distinct_id
```

### Step 1: Locate the person

```bash
# Find distinct_id by email
bosshogg person list --email "user@example.com" --json | jq '.[].distinct_ids'
```

Or via HogQL if `person list` doesn't support email search directly:

```bash
bosshogg query run "
  SELECT id, distinct_ids, properties.email
  FROM persons
  WHERE properties.email = 'user@example.com'
  LIMIT 5
" --json | jq '.results'
```

Note all `distinct_ids` — `bosshogg person get` and `bosshogg person delete` take a `distinct_id`, not the UUID.

```bash
export DISTINCT_ID="<primary-distinct-id>"
```

Confirm the person's properties before deletion:

```bash
bosshogg person get "${DISTINCT_ID}" --json | jq '{id, distinct_ids, properties, created_at}'
```

### Step 2: Hard-delete the person

```bash
bosshogg person delete "${DISTINCT_ID}" --yes --json
```

Expected response: `{"deleted": true, "id": "<uuid>"}` with exit code 0.

If the command prompts for confirmation without `--yes`, add `--yes` or type `y` when prompted. Do not skip this gate — it protects against accidental deletions.

### Step 3: Verify deletion via activity log

Confirm the person no longer exists:

```bash
bosshogg person get "${DISTINCT_ID}" --json; echo "exit=$?"
# Expected: {"error":true,"code":"NOT_FOUND",...}; exit=20
```

Check the activity log for the deletion event (PostHog records a deletion audit entry):

```bash
bosshogg query run "
  SELECT timestamp, event, distinct_id, properties
  FROM events
  WHERE event = '\$delete'
    AND distinct_id = '${DISTINCT_ID}'
  ORDER BY timestamp DESC
  LIMIT 5
" --json | jq '.results'
```

### Step 4: Remove from static cohorts

Static cohorts can hold explicit person references. `cohort remove-person` requires the person's UUID (not distinct_id). Obtain the UUID from the HogQL query in step 1 above (`id` field), then purge from all static cohorts:

```bash
# List all static cohorts
bosshogg cohort list --json | jq '[.[] | select(.is_static == true)] | .[].{id, name}'

export PERSON_UUID="<uuid-from-step-1>"

# For each static cohort, attempt removal by UUID (safe to run even if not a member)
for COHORT_ID in <id1> <id2> <id3>; do
  bosshogg cohort remove-person "${COHORT_ID}" --person-id "${PERSON_UUID}" --yes --json && \
    echo "Removed from cohort ${COHORT_ID}" || \
    echo "Not in cohort ${COHORT_ID} (OK)"
done
```

### Step 5: Verify flag evaluation no longer resolves

After deletion, the distinct_id should no longer match any person-property-based flag rules:

```bash
# Evaluate all active flags for the deleted distinct_id
bosshogg flag list --active --json | jq '.[].key' | while read FLAG_KEY; do
  RESULT=$(bosshogg flag evaluate "${FLAG_KEY}" --distinct-id "${DISTINCT_ID}" --json 2>&1)
  echo "${FLAG_KEY}: ${RESULT}"
done
```

For flags that rely on person properties (e.g., `plan == "pro"`), `is_enabled` should now be `false` because the person record has been deleted and property-based filter evaluation fails open to `false`.

Document the verification:

```bash
cat > /tmp/gdpr-deletion-record-$(date +%Y%m%d).txt <<EOF
GDPR Deletion Record
====================
Email:           user@example.com
Person UUID:     ${PERSON_UUID}
Distinct ID:     ${DISTINCT_ID}
Deleted at:      $(date -u +%Y-%m-%dT%H:%M:%SZ)
Verified by:     $(bosshogg whoami --json | jq -r '.email')
Person GET exit: 20 (NOT_FOUND)
Cohorts purged:  <list cohort IDs checked>
Flags verified:  $(bosshogg flag list --active --json | jq 'length') active flags checked
EOF
echo "Record written to /tmp/gdpr-deletion-record-$(date +%Y%m%d).txt"
```

**Footnotes:**
- PostHog's person delete is a hard delete of the person record but does NOT retroactively delete the person's events from ClickHouse. If your legal obligation requires event-level deletion, contact PostHog support — this requires a backend data erasure request.
- `distinct_id` can be multi-valued (merged IDs). Delete the person UUID — PostHog handles cascading deletion of all merged distinct_ids automatically.
- For EU Cloud (`eu.posthog.com`), the same commands apply; ensure your context is set to the EU region (`bosshogg config current-context` → host should be `https://eu.posthog.com`).
- `cohort remove-person` only works on static cohorts (`is_static == true`). Dynamic cohorts recalculate on a schedule and will naturally exclude the deleted person on next recalculation.
- If the user has multiple PostHog projects (e.g., staging + prod), repeat this playbook for each project context.
