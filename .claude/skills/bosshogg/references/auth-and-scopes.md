# Auth and scopes reference

PostHog uses granular scopes on personal API keys. All setup paths lead to `bosshogg doctor --json`.

## Table of contents

1. [Context management](#context-management)
2. [Key types](#key-types)
3. [Scopes catalog](#scopes-catalog)
4. [The 403 remediation flow](#the-403-remediation-flow)
5. [Environment variables](#environment-variables)
6. [EU Cloud and self-hosted](#eu-cloud-and-self-hosted)
7. [CI setup](#ci-setup)
8. [Key hygiene](#key-hygiene)

## Context management

Contexts are kubectl-style: named bundles of `(host, project_id, env_id, key)` stored in `~/.config/bosshogg/config.toml` with `0600` perms.

Create a context interactively:

```bash
bosshogg configure
```

Create non-interactively:

```bash
bosshogg config set-context prod \
  --host https://us.posthog.com \
  --project 999999 \
  --env 999999 \
  --key-from-env POSTHOG_CLI_TOKEN
```

`--key-from-env` stores the *name* of the env var, not the value — the key is resolved at call time, not committed to disk. For workstations where a plaintext key is acceptable:

```bash
bosshogg config set-context dev \
  --host https://us.posthog.com \
  --project 999999 \
  --key-from-stdin < ~/.secrets/posthog-dev.key
```

List, inspect, and switch:

```bash
bosshogg config get-contexts
bosshogg config current-context
bosshogg config use-context prod
bosshogg use prod          # shorthand
```

Remove a context:

```bash
bosshogg config delete-context dev --yes
```

Configs travel well: copy `~/.config/bosshogg/config.toml` between machines (after redacting keys if any were stored plaintext).

## Key types

PostHog has three key types; the CLI uses one of them.

| Key type | Prefix | Used by `bosshogg`? |
|---|---|---|
| Personal API key | `phx_` | **Yes.** Carries granular scopes. This is what `bosshogg configure` asks for. |
| Project API key | `phc_` | No — that's for SDK ingestion (`posthog-rs`, `posthog-js`). |
| Organization API key (legacy) | `phs_` | No — deprecated; do not use. |

Issue a personal API key at `https://us.posthog.com/settings/user-api-keys` (or EU equivalent). Select the minimum scopes your workflow requires.

## Scopes catalog

Personal API keys carry scopes in `<resource>:<verb>` form. Common scopes:

| Scope | Enables |
|---|---|
| `feature_flag:read` | `bosshogg flag list`, `get`, `evaluate` |
| `feature_flag:write` | `bosshogg flag create`, `update`, `delete` |
| `query:read` | `bosshogg query run` (HogQL), `bosshogg schema hogql` |
| `insight:read` | `bosshogg insight list`, `get`, `refresh` (M3+) |
| `insight:write` | `bosshogg insight create`, `update`, `delete` (M3+) |
| `dashboard:read` | `bosshogg dashboard list`, `get` (M3+) |
| `dashboard:write` | `bosshogg dashboard create`, `update`, `delete` (M3+) |
| `cohort:read` | `bosshogg cohort list`, `get` (M3+) |
| `cohort:write` | `bosshogg cohort create`, `update`, `delete` (M3+) |
| `person:read` | `bosshogg person get`, `list` (M4+) |
| `person:write` | `bosshogg person delete`, property mutations (M4+) |
| `event_definition:read` | `bosshogg event-definition list`, `get` (M5+) |
| `event_definition:write` | `bosshogg event-definition update`, `delete` (M5+) |
| `property_definition:read` | `bosshogg property-definition list`, `get` (M5+) |
| `property_definition:write` | `bosshogg property-definition update`, `delete` (M5+) |
| `action:read` / `action:write` | Actions (M4+) |
| `annotation:read` / `annotation:write` | Annotations (M4+) |
| `experiment:read` / `experiment:write` | Experiments (M6+) |
| `survey:read` / `survey:write` | Surveys (M6+) |
| `hog_function:read` / `hog_function:write` | Hog functions (M7+) |
| `batch_export:read` / `batch_export:write` | Batch exports (M7+) |
| `session_recording:read` | `bosshogg session-recording list`, `get` (M8+) |
| `session_recording:write` | `bosshogg session-recording update`, `delete` (M8+) |
| `error_tracking:read` / `error_tracking:write` | Error tracking (M8+) |
| `notebook:read` / `notebook:write` | Notebooks (v1.x) |
| `organization_admin` | Org-level reads/writes (rarely needed from CLI) |

Principle of least privilege: create one key per agent/workflow. Example split:

- **Read-only CI key**: `query:read`, `feature_flag:read`, `insight:read`.
- **Rollout key**: `feature_flag:read`, `feature_flag:write`, `query:read` (for guardrail queries).
- **Debug key**: `person:read`, `query:read`, `session_recording:read`, `error_tracking:read`.

**PostHog has no scope-introspection endpoint** — see [PostHog#25865](https://github.com/PostHog/posthog/issues/25865). `bosshogg` cannot tell you what a key can do before you use it; it can only react to a 403.

## The 403 remediation flow

When a call fails with 403, `bosshogg` parses the response and emits:

```json
{
  "error": true,
  "code": "AUTH_SCOPE",
  "message": "Personal API key is missing required scope",
  "hint": "This action requires scope `feature_flag:write`. Issue a new key with that scope at https://us.posthog.com/settings/user-api-keys.",
  "retry_with": []
}
```

Agent behavior:

1. Read `hint`. It names the exact missing scope.
2. Direct the user to re-issue the key with that scope added (PostHog does not support editing a key's scopes after creation — you issue a new key and replace it).
3. Once the user has a fresh key, update the context:

   ```bash
   bosshogg config set-context prod --key-from-stdin < ~/Downloads/new-key.txt
   ```

4. Retry the original command.

To preempt a 403 on an important workflow, the skill provides:

```bash
scripts/preflight-scope.sh feature_flag:read
```

The script runs a benign read (`GET /api/projects/:pid/feature_flags/?limit=1`) and, if the response is 403, parses the missing scope out of the body and prints a remediation line. A zero exit code means "the key can do this".

## Environment variables

All bosshogg commands honor these, with the first-set winning:

| Var | Purpose | Fallback |
|---|---|---|
| `POSTHOG_CLI_TOKEN` | Personal API key | `POSTHOG_CLI_API_KEY`, `POSTHOG_API_KEY` |
| `POSTHOG_CLI_HOST` | Base URL | `POSTHOG_HOST` |
| `POSTHOG_CLI_PROJECT_ID` | Project override | `POSTHOG_PROJECT_ID` |
| `POSTHOG_CLI_ENV_ID` | Environment override | `POSTHOG_ENV_ID` |
| `POSTHOG_CLI_ORG_ID` | Org override | `POSTHOG_ORG_ID` |
| `BOSSHOGG_PROFILE` | Named profile override | — |
| `BOSSHOGG_CONFIG` | Override config file path | — |

Resolution precedence (high to low):

1. `--api-key <key>` flag
2. `--profile <name>` flag
3. `POSTHOG_CLI_TOKEN` / `POSTHOG_CLI_API_KEY` / `POSTHOG_API_KEY`
4. Default profile in `~/.config/bosshogg/config.toml`
5. `.env` / `.env.local` in cwd

`bosshogg` does **not** search `current_exe().parent()` for `.env` (lesson from the `lin` security review — a credential-hijack vector).

## EU Cloud and self-hosted

EU:

```bash
bosshogg config set-context eu \
  --host https://eu.posthog.com \
  --project <eu-project-id>
```

Self-hosted:

```bash
bosshogg config set-context onprem \
  --host https://posthog.your-domain.com \
  --project <id>
```

`bosshogg doctor` verifies the host answers and reports the detected region. If the key was issued on a different region than the host, you will see `region_mismatch` — re-issue on the right region.

Older self-hosted instances may not implement `/api/environments/:eid/query/` (the modern HogQL path). If `bosshogg query run` returns 404, pass `--legacy-endpoints` to fall back to `/api/projects/:pid/query/`.

## CI setup

Minimum env for CI:

```yaml
env:
  POSTHOG_CLI_TOKEN: ${{ secrets.POSTHOG_CLI_TOKEN }}
  POSTHOG_CLI_PROJECT_ID: "999999"
  POSTHOG_CLI_HOST: "https://us.posthog.com"
```

Install `bosshogg` in CI:

```yaml
- name: Install bosshogg
  run: cargo install bosshogg
```

Or pin a release binary for speed:

```yaml
- name: Install bosshogg
  run: |
    curl -sSfL https://github.com/aaronkwhite/bosshogg-cli/releases/download/v2026.4.0/bosshogg-x86_64-unknown-linux-gnu.tar.gz \
      | tar -xzC /usr/local/bin
    bosshogg --version
```

Run a smoke check at the start of every job:

```yaml
- name: Verify PostHog auth
  run: bosshogg doctor --json | jq -e '.status == "ok"'
```

## Key hygiene

- Store keys in a secret manager (1Password, AWS Secrets Manager, GitHub Actions secrets).
- One key per workflow. Do not share one omnibus key.
- Rotate on a schedule (quarterly minimum) and on any key exposure.
- `bosshogg auth token` emits the resolved bearer on stdout. Use it only for one-off escape-hatch curls — never pipe into persistent logs.
- Revoked keys fail with `AUTH_INVALID`; issue a new one and update the context.
