# bosshogg org + project reference

## When to use

Use `bosshogg org` and `bosshogg project` to inspect and switch the active PostHog organization or project stored in your config context. Neither command creates or deletes orgs/projects — they are read and scope-management commands only (except `reset-token`, which is destructive).

Use `bosshogg configure` to set up a new context from scratch. Use these commands afterward to switch scope without re-running the wizard.

---

## `bosshogg org` quick reference

| Subcommand | What it does | Example |
|---|---|---|
| `list` | List all orgs the API key can access | `bosshogg org list --json` |
| `get <id>` | Fetch one org by UUID or slug | `bosshogg org get acme` |
| `current` | Show the org currently active in config | `bosshogg org current --json` |
| `switch <id>` | Set active org in config (no API call) | `bosshogg org switch org-uuid-abc` |

**`org list` example output:**
```json
{
  "count": 2,
  "results": [
    {"id": "org-uuid-1", "name": "Acme Corp", "slug": "acme", ...},
    {"id": "org-uuid-2", "name": "Beta Inc",  "slug": "beta",  ...}
  ]
}
```

**`org switch` writes config locally** — no network call. The next API command automatically picks up the new org_id.

---

## `bosshogg project` quick reference

| Subcommand | What it does | Example |
|---|---|---|
| `list` | List projects in the active org | `bosshogg project list --json` |
| `get <id-or-name>` | Fetch one project (numeric id or name) | `bosshogg project get 999999` or `bosshogg project get "My App"` |
| `current` | Show the project currently active in config | `bosshogg project current --json` |
| `switch <id>` | Set active project in config (no API call) | `bosshogg project switch 999999` |
| `reset-token <id>` | Rotate the phc_ project token | `bosshogg --yes project reset-token 999999` |

**`project get` accepts names:** if the identifier is not numeric, bosshogg lists all projects in the active org and filters by name. Requires `org_id` to be set.

---

## `bosshogg project reset-token` safety

`reset-token` rotates the `phc_...` project API token that SDK clients use for event capture and feature-flag evaluation.

**Effect is immediate and irreversible:**
- The old token stops working the moment the PATCH lands.
- Every SDK, server-side integration, or CI environment sending events or evaluating flags with the old token will begin receiving 401 errors.

**When to use:**
- Token was leaked or compromised.
- Rotating credentials as part of a security incident response.
- Decommissioning an integration intentionally.

**Never run without coordination.** Before rotating:
1. Identify all consumers (SDKs, CI, server envs).
2. Prepare the new token value (returned in the response).
3. Deploy the new token to all consumers.
4. Only then run `reset-token`.

The `--yes` flag skips the interactive TTY confirm. In automation, always pass `--yes` explicitly; never pipe stdin.

---

## Relationship to `bosshogg configure`

| Task | Command |
|---|---|
| First-time setup (new context) | `bosshogg configure` |
| Switch org within an existing context | `bosshogg org switch <id>` |
| Switch project within an existing context | `bosshogg project switch <id>` |
| Inspect current scope | `bosshogg org current` / `bosshogg project current` |
| Show user identity + scopes | `bosshogg whoami` |

`bosshogg whoami` shows the API key's owner and default org/team from PostHog's perspective. `bosshogg org current` / `bosshogg project current` show what bosshogg itself will scope subsequent commands to.

---

## Example workflows

### 1. Scope subsequent flag commands to a specific project

```sh
# Discover what's available
bosshogg org list --json | jq '.results[] | {id, name}'
bosshogg org switch org-uuid-acme

bosshogg project list --json | jq '.results[] | {id, name}'
bosshogg project switch 999999

# Now flag commands use project 999999
bosshogg flag list --active --json
```

### 2. Inspect the current scope before running a destructive operation

```sh
bosshogg org current --json     # confirm org
bosshogg project current --json # confirm project
bosshogg flag delete my-old-flag --yes
```

### 3. Rotate a compromised project token safely

```sh
# 1. Identify the project
bosshogg project get 999999 --json | jq '{id, name, api_token}'

# 2. Rotate (prints new token)
bosshogg --yes project reset-token 999999

# 3. Update all consumers immediately after
#    e.g.: export POSTHOG_API_KEY=phc_<new_token>
```
