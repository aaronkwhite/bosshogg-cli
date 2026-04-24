# MCP gaps: bosshogg vs PostHog MCP decision matrix

Use `bosshogg` for reads and simple writes. Reach for the PostHog MCP when the task needs a tool `bosshogg` does not expose (chart rendering, wizards, notebook authoring). This matrix is the authoritative tiebreaker.

Format: **Task | Use this | Command | Fallback if unavailable**.

| Task | Use this | Command | Fallback if unavailable |
|---|---|---|---|
| List feature flags | `bosshogg` | `bosshogg flag list --active --json` | `mcp__posthog__feature-flag-list` |
| Toggle a feature flag | `bosshogg` | `bosshogg flag update <key> --enabled --json` (or `--disabled`) | `mcp__posthog__feature-flag-update` |
| Create a feature flag | `bosshogg` | `bosshogg flag create --key ... --filters-file ... --disabled --json` | `mcp__posthog__feature-flag-create` |
| Run ad-hoc HogQL | `bosshogg` | `bosshogg query run --file query.sql --json` (auto-LIMIT 100) | `mcp__posthog__sql-query` |
| Ground a HogQL query in the schema | `bosshogg` | `bosshogg schema hogql --json` (cache via `scripts/hogql-schema-dump.sh`) | `mcp__posthog__data-schema` |
| Read an insight definition | `bosshogg` | `bosshogg insight get <id> --json` (M3+) | `mcp__posthog__insight-get` |
| Render an insight as a chart (PNG/URL) | **MCP** | `mcp__posthog__insight-render-chart` | Web UI URL `https://us.posthog.com/project/<pid>/insights/<short_id>` |
| List cohorts | `bosshogg` | `bosshogg cohort list --json` (M3+) | `mcp__posthog__cohort-list` |
| Create or edit a cohort | `bosshogg` | `bosshogg cohort create --filters-file ...` (M3+) | `mcp__posthog__cohort-create` |
| Inspect a person by distinct id or email | `bosshogg` | `bosshogg person get <email_or_id> --json` (M4+) | `mcp__posthog__person-get` |
| Fetch session replay metadata | `bosshogg` | `bosshogg session-recording get <id> --json` (M8+) | `mcp__posthog__session-recording-get` |
| Download a session replay's rendered rrweb blob | **MCP or @posthog/cli** | `mcp__posthog__session-recording-render` (when available) | `bosshogg session-recording get --snapshot --out file.jsonl` (M8+, never stdout) |
| List error-tracking fingerprints | `bosshogg` | `bosshogg error-tracking fingerprints list --json` (M8+) | `mcp__posthog__error-tracking-list` |
| Upload sourcemaps / dSYM / ProGuard | **`@posthog/cli`** | `npx @posthog/cli sourcemap upload ...` | No equivalent in `bosshogg` — out of scope by design |
| Launch the Max AI wizard | **MCP or web** | `mcp__posthog__max-chat` (if configured) | Web UI — wizard is not exposed over CLI |
| Manage hog functions (list, enable, disable) | `bosshogg` | `bosshogg hog-function list --json` (M7+) | `mcp__posthog__hog-function-list` |
| Invoke a hog function (debug) | `bosshogg` | `bosshogg hog-function invoke <id> --input-file event.json --json` (M7+) | `mcp__posthog__hog-function-invoke` |
| Run an experiment readout | `bosshogg` + HogQL | `bosshogg experiment get <id> --json` then HogQL for exposure/variant split (M6+) | `mcp__posthog__experiment-readout` |
| Author a PostHog Notebook | **MCP or web** | `mcp__posthog__notebook-create` | Web UI — notebook authoring is not exposed over CLI (v1.x target) |
| Fire a test event | `bosshogg` (debug only) | `bosshogg capture event --event test_event --distinct-id user_1 --prop foo=bar --json` (M8+) | `curl` with project API key against `/capture/` |
| Identify a user (debug only) | `bosshogg` (debug only) | `bosshogg capture identify --distinct-id user_1 --set email=aaron@example.com --json` (M8+) | Direct SDK call (`posthog-rs` for prod) |
| Fetch activity log for a specific resource | **REST via `auth token`** | `curl -H "Authorization: Bearer $(bosshogg auth token)" "$HOST/api/projects/<pid>/activity_log/?scope=FeatureFlag&item_id=<id>"` | `mcp__posthog__activity-log-query` |
| List orgs the key can access | `bosshogg` | `bosshogg org list --json` | N/A |
| Switch active org | `bosshogg` | `bosshogg org switch <id>` | `export POSTHOG_ORG_ID=...` |
| Switch active project | `bosshogg` | `bosshogg project switch <id>` | `export POSTHOG_CLI_PROJECT_ID=...` |
| Reset project token (rotate phc_ key) | `bosshogg` | `bosshogg project reset-token <id>` | N/A (do not do this in web UI without coordination) |
| Check which projects a key can see | `bosshogg` | `bosshogg whoami --json` | `mcp__posthog__workspace-info` |
| Run `bosshogg` preflight | `bosshogg` | `bosshogg doctor --json` (or `scripts/doctor.sh`) | None — this check is CLI-only |
| List dashboards | `bosshogg` | `bosshogg dashboard list --json` | `mcp__posthog__dashboard-list` |
| Refresh all insights on a dashboard | `bosshogg` | `bosshogg dashboard refresh <id> --json` | `mcp__posthog__dashboard-refresh` |
| Add / move / reorder a dashboard tile | `bosshogg` | `bosshogg dashboard tiles add <id> --insight <id>` (also `move`/`copy`/`reorder`) | `mcp__posthog__dashboard-tile-*` |
| Create / update an insight | `bosshogg` | `bosshogg insight create --filters-file ... --json` | `mcp__posthog__insight-create` |
| Cohort member inspection | `bosshogg` | `bosshogg cohort members <id> --json` | `mcp__posthog__cohort-members` |
| List persons in a cohort | `bosshogg` | `bosshogg cohort members <id> --limit 100 --json` | `mcp__posthog__cohort-members` |
| Group analytics (accounts/companies) | `bosshogg` | `bosshogg group find --group-type-index 0 --group-key acme --json` | `mcp__posthog__group-get` |
| Query recent events for a user | `bosshogg` | `bosshogg event list --distinct-id user_123 --limit 50 --json` (tunnels through HogQL) | `bosshogg query run "SELECT * FROM events WHERE distinct_id='user_123' LIMIT 50"` |
| Tail events in real time | `bosshogg` | `bosshogg event tail --event button_clicked --limit 20` | None — MCP lacks a polling loop |
| Create / manage actions (event matchers) | `bosshogg` | `bosshogg action create --name ... --steps-file ...` | `mcp__posthog__action-create` |
| Add a release annotation | `bosshogg` | `bosshogg annotation create --content "v2.0" --date-marker 2026-04-21T00:00Z` | `mcp__posthog__annotation-create` |
| Audit taxonomy (event + property definitions) | `bosshogg` | `bosshogg event-definition list --search checkout` / `bosshogg property-definition list --type event` | `mcp__posthog__event-definition-list` |
| Materialized HogQL endpoints (save + run) | `bosshogg` | `bosshogg endpoint create --name top-events --query-file q.sql` then `bosshogg endpoint run top-events` | `mcp__posthog__endpoint-run` |
| Launch an experiment | `bosshogg` | `bosshogg experiment create --name ... --feature-flag-key ... --parameters-file p.json` | `mcp__posthog__experiment-create` |
| Survey CRUD | `bosshogg` | `bosshogg survey list --archived=false --json` | `mcp__posthog__survey-list` |
| Early-access feature program | `bosshogg` | `bosshogg early-access create --name ... --stage beta ...` | `mcp__posthog__early-access-create` |
| Batch export to S3 / BigQuery / Snowflake | `bosshogg` | `bosshogg batch-export create --name ... --destination-file d.json --interval hour` | `mcp__posthog__batch-export-create` |
| Pause / unpause a batch export | `bosshogg` | `bosshogg batch-export pause <id> --yes` | `mcp__posthog__batch-export-update` |
| Backfill historical events for a batch export | `bosshogg` | `bosshogg batch-export backfills create <id> --start-at 2026-01-01` | `mcp__posthog__batch-export-backfill` |
| Subscription to deliver a dashboard | `bosshogg` | `bosshogg subscription create --title ... --target-type email --target-value ... --frequency daily --dashboard-id <id>` | `mcp__posthog__subscription-create` |
| Enterprise RBAC role management | `bosshogg` | `bosshogg role create --name ReadOnly --feature-flags-access-level 1` | `mcp__posthog__role-create` |
| Error-tracking assignment / grouping rules | `bosshogg` | `bosshogg error-tracking assignment-rules create --filters-file ... --assignee-id <uid>` | `mcp__posthog__error-tracking-rule` |
| Resolve source file / line to a GitHub link | `bosshogg` | `bosshogg error-tracking resolve-github --organization ... --repo ... --file src/main.rs --line 42` | Manual GitHub URL construction |

## When in doubt: the cascade

1. Try `bosshogg <verb> --help`. If a matching verb exists, use it.
2. If not, check this matrix. If the task has "MCP" in the "Use this" column, use the named MCP tool (always fully qualified with `mcp__posthog__` prefix).
3. If neither exists, drop to HogQL via `bosshogg query run`.
4. Last resort: raw REST via `bosshogg auth token`.

## Why bosshogg over MCP for the common case

- **Idle token cost.** PostHog MCP loads ~44k tokens of tool definitions per session. BossHogg's skill frontmatter is ~200 tokens; the body loads only when the skill triggers.
- **Stable JSON contract.** `bosshogg` JSON is compact, envelope-free, and versioned. MCP tool response shapes are documented but not contract-tested client-side.
- **Scriptability.** `bosshogg flag list --json | jq '.results[] | select(.active)'` also runs in cron, CI, and Makefiles. MCP does not.
- **Error codes.** `bosshogg` emits stable screaming-snake-case codes (`AUTH_SCOPE`, `RATE_LIMITED`) with `retry_after_s`. MCP errors are free-form strings.

## Why MCP for chart-rendering, wizards, and notebooks

- **Chart rendering** needs a server to draw pixels. The web UI and MCP can; the CLI returns only the definition.
- **Max AI wizard** is conversational and UI-coupled.
- **Notebook authoring** relies on a rich-text editor; a CLI wrapper would be lossy.
- **Sourcemap uploads** belong to `@posthog/cli` by explicit division of labor.

## Cross-product tasks

When the task spans multiple products (e.g., "why did conversion drop"), read `references/cross-product-playbooks.md`. Those decision trees compose `bosshogg` verbs plus, where necessary, MCP calls — you do not choose one tool upfront, you follow the tree.
