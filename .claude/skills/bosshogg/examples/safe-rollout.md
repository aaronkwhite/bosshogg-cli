# Safe feature flag rollout — end-to-end example

This is an annotated transcript of a team rolling out a new checkout UI behind a feature flag, using the safe-rollout playbook from `references/cross-product-playbooks.md`. It shows the exact commands, guardrail queries, and decision points.

## Setup

Team: Acme Corp checkout team (Alice is the oncall engineer).
Feature: new checkout flow (faster, fewer form fields).
Project: PostHog project `999999` (US Cloud).
Time: April 21, 2026, 2:00 PM UTC.

Alice already has PostHog credentials set up:

```bash
$ bosshogg whoami --json
{
  "host": "https://us.posthog.com",
  "project_id": 999999,
  "user_email": "alice@acme.com"
}
```

## Step 1: Create the flag at 0% disabled

Alice creates a filters file to define the initial targeting (no restrictions, open to all):

```bash
# filters.json
cat > /tmp/filters.json << 'EOF'
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
EOF
```

She creates the flag:

```bash
$ bosshogg flag create \
  --key checkout-redesign \
  --name "Checkout Redesign" \
  --description "New checkout UI: 4-step vs 6-step, 40% time reduction in beta" \
  --type boolean \
  --filters /tmp/filters.json \
  --disabled \
  --json
```

Response:

```json
{
  "ok": true,
  "id": 5432,
  "key": "checkout-redesign",
  "name": "Checkout Redesign",
  "active": false,
  "rollout_percentage": 0,
  "created_at": "2026-04-21T14:05:00Z"
}
```

Alice confirms:
- `active: false` (disabled — no users exposed yet)
- `rollout_percentage: 0` (0 users will see it)

## Step 2: Enable + 1% rollout

```bash
$ bosshogg flag update checkout-redesign --enabled --rollout 1 --json
```

Response:

```json
{
  "ok": true,
  "id": 5432,
  "action": "update",
  "active": true,
  "rollout_percentage": 1,
  "updated_at": "2026-04-21T14:07:00Z"
}
```

The flag is now live at 1%. Alice notes the time: 14:07 UTC.

## Step 3: Monitor guardrails (15–30 min)

While the flag rolls out to ~1% of traffic, Alice monitors three guardrails every 5 min.

### Guardrail 1: Error rate

```bash
$ bosshogg query run "SELECT
  countIf(event = '\$exception') AS errors,
  countIf(event != '\$exception') AS other_events,
  errors / (errors + other_events) AS error_rate
FROM events
WHERE timestamp > now() - INTERVAL 15 MINUTE" --json
```

Output at 14:12:

```json
{
  "columns": ["errors", "other_events", "error_rate"],
  "types": ["UInt64", "UInt64", "Float64"],
  "results": [[2, 1048, 0.0019]]
}
```

Error rate: 0.19% ✓ (target: < 2%)

### Guardrail 2: P95 latency

Alice's baseline for the `/api/checkout` endpoint is 150 ms. She's OK with up to 165 ms (10% slower).

```bash
$ bosshogg query run "SELECT quantile(0.95)(toFloat64(properties.duration_ms)) AS p95_ms
FROM events
WHERE event = '\$request_complete'
  AND timestamp > now() - INTERVAL 15 MINUTE
  AND properties.endpoint = '/api/checkout'" --json
```

Output at 14:12:

```json
{
  "columns": ["p95_ms"],
  "types": ["Float64"],
  "results": [[152.3]]
}
```

P95 latency: 152 ms ✓ (target: < 165 ms)

### Guardrail 3: 4xx error rate

```bash
$ bosshogg query run "SELECT count() AS client_errors
FROM events
WHERE event = '\$exception'
  AND timestamp > now() - INTERVAL 15 MINUTE
  AND properties.status_code >= 400 AND properties.status_code < 500" --json
```

Output at 14:12:

```json
{
  "columns": ["client_errors"],
  "types": ["UInt64"],
  "results": [[1]]
}
```

1 4xx error in 15 min vs ~1050 total requests = 0.095% ✓ (target: < 1%)

**All three guardrails are green.** Alice monitors once more at 14:25, sees the same green results, and proceeds to ramp at 14:30.

## Step 4: Ramp → 10% → 50% → 100%

### Ramp to 10%

```bash
$ bosshogg flag update checkout-redesign --rollout 10 --json
```

Response:

```json
{
  "ok": true,
  "action": "update",
  "rollout_percentage": 10,
  "updated_at": "2026-04-21T14:31:00Z"
}
```

Alice checks guardrails at 14:35, 14:40, 14:45:

- Error rate: 0.21% ✓
- P95 latency: 154 ms ✓
- 4xx errors: 0.08% ✓

**Proceed to 50%.**

### Ramp to 50%

```bash
$ bosshogg flag update checkout-redesign --rollout 50 --json
```

Response:

```json
{
  "ok": true,
  "action": "update",
  "rollout_percentage": 50,
  "updated_at": "2026-04-21T14:46:00Z"
}
```

This is a bigger jump. Alice monitors every 2 min for the next 30 min.

At 14:52, she sees:

- Error rate: 0.18% ✓
- P95 latency: 156 ms ✓
- 4xx errors: 0.09% ✓

At 15:10, final check before 100%:

- Error rate: 0.20% ✓
- P95 latency: 157 ms ✓
- 4xx errors: 0.10% ✓

**All green. Proceed to 100%.**

### Ramp to 100%

```bash
$ bosshogg flag update checkout-redesign --rollout 100 --json
```

Response:

```json
{
  "ok": true,
  "action": "update",
  "rollout_percentage": 100,
  "updated_at": "2026-04-21T15:11:00Z"
}
```

Final guardrail check at 15:25 (30 min after hitting 100%):

```bash
# (same queries as before, checking last 30 min)
```

Results:

- Error rate: 0.19% ✓
- P95 latency: 155 ms ✓
- 4xx errors: 0.09% ✓

**All three guardrails held. Rollout is complete.**

## Step 5 (Hypothetical): Rollback scenario

If a guardrail had breached (e.g., error rate spiked to 3.5%), Alice would immediately disable:

```bash
$ bosshogg flag update checkout-redesign --disabled --yes --json
```

Response:

```json
{
  "ok": true,
  "action": "update",
  "active": false,
  "updated_at": "2026-04-21T15:30:00Z"
}
```

Then she would:

1. Investigate (check server logs, recent deploys, customer reports).
2. File an incident ticket.
3. Fix the root cause.
4. Re-run the playbook starting from step 2 (enable + 1%).

## Step 6: Declare done

Flag is at 100%, all guardrails have held for 30+ minutes, no complaints in Slack.

Alice announces in the checkout team channel:

> Checkout redesign is now fully rolled out. Flag: `checkout-redesign`. Results:
> - No increase in error rates (0.19% vs baseline 0.18%).
> - P95 latency is stable (157 ms vs baseline 150 ms, well within tolerance).
> - No new 4xx errors.
>
> Beta testers reported a 35% time-to-complete reduction. Full success!
>
> Link: https://us.posthog.com/project/999999/feature_flags/checkout-redesign

Alice also creates an annotation in the flag (M4+ feature):

```bash
bosshogg flag annotate checkout-redesign \
  --message "Fully rolled out. Metrics in #checkout-team Slack." \
  --json
```

Done. The rollout took 1 hour and 23 minutes from flag creation to 100% with zero regressions.
