# Feature flags reference

All commands below use `--json` so output is parser-friendly. Drop `--json` for a human-friendly table when running by hand.

## Table of contents

1. [List flags](#list-flags)
2. [Get one flag](#get-one-flag)
3. [Create a flag](#create-a-flag)
4. [Toggle enabled/disabled](#toggle-enableddisabled)
5. [Set rollout percentage](#set-rollout-percentage)
6. [Update filters](#update-filters)
7. [Update JSON payloads (multivariate)](#update-json-payloads-multivariate)
8. [Evaluate for a distinct id](#evaluate-for-a-distinct-id)
9. [Soft-delete](#soft-delete)
10. [Common errors](#common-errors)
11. [Filters file schema](#filters-file-schema)

## List flags

List all flags (paginated, auto-following by default):

```bash
bosshogg flag list --json
```

Filter to active, by type, by tag, or by search string:

```bash
bosshogg flag list --active --json
bosshogg flag list --type boolean --json
bosshogg flag list --type multivariate --json
bosshogg flag list --tag checkout --json
bosshogg flag list --search "new-billing" --json
```

Combine filters — they AND together:

```bash
bosshogg flag list --active --tag checkout --json
```

Cap the result set with `--limit`, resume with `--cursor`:

```bash
bosshogg flag list --limit 200 --json
bosshogg flag list --cursor "cHR..." --json
```

Output shape:

```json
{
  "count": 42,
  "next_cursor": null,
  "results": [
    {
      "id": 123,
      "key": "checkout-redesign",
      "name": "Checkout Redesign",
      "description": "...",
      "type": "boolean",
      "active": true,
      "rollout_percentage": 50,
      "created_at": "2026-01-15T10:30:00Z",
      "created_by": "...",
      "updated_at": "2026-04-01T12:45:00Z",
      "updated_by": "..."
    }
  ]
}
```

## Get one flag

Fetch a flag by key (resolved locally against the project cache) or by numeric id:

```bash
bosshogg flag get checkout-redesign --json
bosshogg flag get 123 --json
```

Output shape:

```json
{
  "id": 123,
  "key": "checkout-redesign",
  "name": "Checkout Redesign",
  "description": "...",
  "type": "boolean",
  "active": true,
  "rollout_percentage": 50,
  "created_at": "2026-01-15T10:30:00Z",
  "created_by": "...",
  "updated_at": "2026-04-01T12:45:00Z",
  "updated_by": "...",
  "filters": [...],
  "variants": [
    {
      "id": 456,
      "key": "control",
      "rollout_percentage": 50
    },
    {
      "id": 457,
      "key": "treatment",
      "rollout_percentage": 50
    }
  ]
}
```

## Create a flag

```bash
bosshogg flag create \
  --key checkout-redesign \
  --name "Checkout Redesign" \
  --description "New checkout UI" \
  --type boolean \
  --json
```

Optional flags for initial state:

```bash
bosshogg flag create \
  --key my-flag \
  --type boolean \
  --enabled \
  --rollout 10 \
  --json
```

For multivariate flags, specify `--type multivariate` and provide variants via `--variants-file`:

```bash
bosshogg flag create \
  --key exp-new-cta \
  --type multivariate \
  --variants-file /tmp/variants.json \
  --json
```

`variants.json` schema:

```json
[
  {
    "key": "control",
    "name": "Control CTA (Legacy)",
    "rollout_percentage": 50
  },
  {
    "key": "treatment_v1",
    "name": "New Orange CTA",
    "rollout_percentage": 50
  }
]
```

Output shape: same as `flag get`.

## Toggle enabled/disabled

Enable a flag (1% rollout by default if no rollout is set):

```bash
bosshogg flag update my-flag --enabled --yes --json
```

Disable a flag (requires `--yes`):

```bash
bosshogg flag update my-flag --disabled --yes --json
```

The CLI will warn before disabling a flag with active rollout. Read the warning, and add `--yes` to confirm.

## Set rollout percentage

Increase or decrease rollout from 0–100:

```bash
bosshogg flag update my-flag --rollout 25 --json
```

Confirm before ramping:

```bash
# Confirm before increasing rollout from 10% to 50%
bosshogg flag update my-flag --rollout 50 --yes --json
```

The CLI requires `--yes` only when you are increasing rollout above the current value (not when decreasing or staying at the same value).

## Update filters

Flags can have targeting rules (filters). Example: target only users with a `plan` property of `pro`.

Fetch the current filters first:

```bash
bosshogg flag get my-flag --json | jq .filters
```

Write updated filters to a file (see [Filters file schema](#filters-file-schema) below), then apply:

```bash
bosshogg flag update my-flag --filters /tmp/filters.json --json
```

If the new filters are incompatible with the current rollout setup, the CLI will warn. Inspect and re-run with `--yes` to proceed.

## Update JSON payloads (multivariate)

For multivariate flags, each variant can carry a JSON payload. Update a variant's payload:

```bash
bosshogg flag update my-flag --variant my-variant --payload '{"color":"blue","size":"lg"}' --json
```

Or pass a JSON file:

```bash
bosshogg flag update my-flag --variant my-variant --payload-file /tmp/payload.json --json
```

The JSON is validated as an object at update time.

## Evaluate for a distinct id

Evaluate a flag for a specific user/distinct_id (M1 feature):

```bash
bosshogg flag evaluate my-flag --distinct-id user-123 --json
```

Output shape:

```json
{
  "flag_key": "my-flag",
  "distinct_id": "user-123",
  "is_enabled": true,
  "variant": null
}
```

For multivariate flags:

```json
{
  "flag_key": "my-flag",
  "distinct_id": "user-123",
  "is_enabled": true,
  "variant": "treatment_v1"
}
```

Requires the flag to have a project token in the active context (set via `bosshogg configure`).

## Soft-delete

Flags are soft-deleted (archived), not hard-deleted. Soft-delete a flag:

```bash
bosshogg flag delete my-flag --yes --json
```

Requires `--yes`. Soft-deleted flags remain queryable but do not appear in `flag list` by default. To see archived flags:

```bash
bosshogg flag list --archived --json
```

## Common errors

| Symptom | Cause | Fix |
|---|---|---|
| `"code": "NOT_FOUND"` | Flag key/id does not exist. | Verify `bosshogg whoami` shows the right project. Retry with correct key/id. |
| `"code": "BAD_REQUEST"` | Filters file is malformed, or variant payload is not JSON. | Inspect the hint. Fix JSON and retry. |
| `"code": "CONFLICT"` | Another editor changed the flag concurrently. | Re-fetch with `flag get`, reapply your changes, and retry. |
| `"code": "VALIDATION"` | Rollout % is out of range, or filters are nonsensical. | Read the hint. Adjust and retry. |
| `"code": "AUTH_SCOPE"` | Personal API key is missing `flag:write` or similar. | Re-issue the key with the scope added. |

## Filters file schema

A filters file is a JSON array of rule groups. Rules are AND'd within a group; groups are OR'd at the top level.

```json
[
  {
    "properties": [
      {
        "key": "plan",
        "value": ["pro"],
        "operator": "exact"
      }
    ]
  },
  {
    "properties": [
      {
        "key": "email",
        "value": ["@example.com"],
        "operator": "icontains"
      }
    ]
  }
]
```

Supported operators: `exact`, `icontains`, `regex`, `gt`, `gte`, `lt`, `lte`, `is_set`, `is_not_set`.

Example: "target users with plan='pro' OR email containing '@example.com'":

```bash
bosshogg flag update my-flag --filters /tmp/filters.json --json
```
