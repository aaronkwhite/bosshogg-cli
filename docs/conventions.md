# Conventions

The contract BossHogg commits to. Agents, CI pipelines, and humans should be able to rely on these without reading the source.

## Output

### `--json`

- Present on every resource subcommand.
- **Default to JSON when stdout isn't a TTY.** Piping always produces JSON; only interactive use gets tables and colors.
- Output is **compact** (no pretty-printing). Roughly 30% fewer tokens for agent consumers.
- **Never wraps the payload in an envelope.** No `{"data": {"flags": …}}`. The top-level shape of list commands is:

```json
{
  "count": 42,
  "next_cursor": "cHR...",
  "results": [ ... ]
}
```

For singular `get`:

```json
{ "id": 123, "key": "my-flag", ... }
```

For mutations:

```json
{ "ok": true, "id": 123, "action": "update" }
```

All JSON goes through `output::print_json()`. Command code never calls `serde_json::to_string` directly.

### Table output

- Used only in TTY mode.
- Column order is documented per-resource in the skill's `references/commands.md`.
- `NO_COLOR` env var respected.
- Tables never show sensitive fields (API keys, webhook secrets, full tokens). Those surface only in `--json` and in `get` detail views with an explicit `--reveal` flag.

### Detail views

- `bosshogg <r> get <id>` in a TTY renders a labeled, human-friendly layout. Example for a flag:

```
ID:          123
Key:         my-feature
Active:      true
Rollout:     25%
Filters:     … (JSON excerpt)
Dependents:  4 flag(s)
Description: (markdown-rendered)
```

- The same command with `--json` returns the raw typed struct.

### Dates and timestamps

- Input: accept ISO-8601 (`2026-04-01`), RFC3339 (`2026-04-01T12:00:00Z`), and relative (`7d`, `2h`, `30m`) for `--since` / `--before` / `--after` flags.
- Output: RFC3339 UTC. Tables abbreviate to relative for readability; JSON always emits RFC3339.

## Errors

### Shape

Errors in JSON mode always use this shape:

```json
{
  "error": true,
  "code": "RATE_LIMITED",
  "message": "Exceeded 2400 query requests/hour",
  "hint": "Wait 47 seconds or pass --async to queue",
  "retry_with": ["--async"],
  "retry_after_s": 47
}
```

Fields:

| Field | Type | Required | Notes |
|---|---|---|---|
| `error` | bool (always `true` for errors) | yes | Distinguishes from success envelopes. |
| `code` | screaming-snake-case string | yes | Stable across versions. See catalog below. |
| `message` | string | yes | Human-readable, single sentence. |
| `hint` | string | no | Actionable next step. Free-form. |
| `retry_with` | array of CLI flag strings | no | Suggest concrete command-line changes. |
| `retry_after_s` | integer | no | Present only on `RATE_LIMITED`. |

### Error code catalog

| Code | Exit | Meaning |
|---|---|---|
| `AUTH_MISSING` | 10 | No API key could be resolved. |
| `AUTH_INVALID` | 11 | Key rejected by PostHog. |
| `AUTH_SCOPE` | 12 | Key is missing a required scope (e.g., `query:read`). |
| `NOT_FOUND` | 20 | Resource doesn't exist or isn't visible to this key. |
| `BAD_REQUEST` | 30 | Invalid input; caller should inspect `message` and `hint`. |
| `CONFLICT` | 31 | Edit collided with concurrent change or uniqueness constraint. |
| `VALIDATION` | 32 | Pre-flight client-side validation failed (e.g., bad date format). |
| `RATE_LIMITED` | 40 | 429 from PostHog; see `retry_after_s`. |
| `UPSTREAM` | 50 | 5xx from PostHog; transient. |
| `NETWORK` | 51 | Connection / DNS / TLS error. |
| `TIMEOUT` | 52 | Request exceeded `--timeout`. |
| `SCHEMA_DRIFT` | 60 | Response didn't match our typed struct. Upgrade `bosshogg` or file an issue. |
| `INTERNAL` | 70 | Unexpected client bug. Panic hook turns this into a structured error. |
| `CONFIG` | 71 | Config is missing or unparseable — typically a missing `POSTHOG_CLI_*` env var or unset context. Message names the specific fix. |

Exit codes are stable. Scripts can rely on them.

### TTY error rendering

- Non-JSON mode prints a single human-friendly line in red, with the hint on the next line (dim). Full error chain surfaces only with `--debug`.
- `--debug` also prints the failing HTTP request (URL, method, redacted auth header) and response body (truncated to 200 chars) to stderr.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | Success. |
| 1 | Generic failure (only when no more specific code applies). |
| 2 | Clap argument error. |
| 10–12 | Auth (see above). |
| 20 | Not found. |
| 30–32 | Client-side / validation errors. |
| 40 | Rate limited. |
| 50–52 | Upstream / network / timeout. |
| 60 | Schema drift. |
| 70 | Internal bug. |
| 130 | Interrupted by SIGINT. |

## Environment variables

BossHogg reuses `@posthog/cli`'s variable names where they exist, so CI configured for the official CLI drops in:

| Var | Purpose | Fallback accepted |
|---|---|---|
| `POSTHOG_CLI_TOKEN` | Personal API key | `POSTHOG_CLI_API_KEY`, `POSTHOG_API_KEY` |
| `POSTHOG_CLI_HOST` | Base URL | `POSTHOG_HOST` |
| `POSTHOG_CLI_PROJECT_ID` | Project override | `POSTHOG_PROJECT_ID` |
| `POSTHOG_CLI_ENV_ID` | Environment override | `POSTHOG_ENV_ID` |
| `POSTHOG_CLI_ORG_ID` | Org override | `POSTHOG_ORG_ID` |
| `BOSSHOGG_PROFILE` | Named profile override | — |
| `BOSSHOGG_CONFIG` | Override config file path | — |
| `NO_COLOR` | Disable color | standard |
| `RUST_LOG` / `BOSSHOGG_LOG` | `tracing` env filter | — |
| `DO_NOT_TRACK` | Set to `1` to disable anonymous self-tracking telemetry. Equivalent to `bosshogg config analytics off`. | standard |

## Auth resolution precedence

Highest to lowest:

1. `--api-key <key>` flag.
2. `--profile <name>` flag (picks config profile).
3. `POSTHOG_CLI_TOKEN` / `POSTHOG_CLI_API_KEY` / `POSTHOG_API_KEY` env var.
4. Default profile in config file.
5. `.env` / `.env.local` in the current working directory.

We do **not** search `current_exe().parent()` for `.env` files — explicit lesson from the `lin` security review. See [`../research/posthog-rust-sdk.md`](../research/posthog-rust-sdk.md) for related auth patterns from `posthog-rs`.

## Soft-delete normalization

These resources refuse `DELETE` (return 405) and require `PATCH {"deleted": true}`:

- `insights`, `feature-flags`, `cohorts`, `annotations`, `notebooks`, `subscriptions`, `hog-functions`, `actions`, `error-tracking-fingerprints`

Client behavior: `bosshogg <resource> delete <id>` always issues a `PATCH {"deleted": true}` for soft-delete resources. Users never see a 405. Hard-delete resources (where applicable) use actual `DELETE`. The mapping lives in `client/mod.rs` as a static list.

## Rate-limit handling

- Default: retry with exponential backoff (1 s, 2 s, 4 s; max 3 attempts).
- Honor `Retry-After` header when present.
- On final failure, return `{"code": "RATE_LIMITED", "retry_after_s": N}`.
- The **query** bucket is separate (2400/hr). Distinguish in error messages so users can diagnose which bucket they hit.
- Rate limits are **team-wide, not per-key** — mention this in the hint so users don't rotate keys hoping to fix it.

## Pagination

PostHog uses cursor-style `{count, next, previous, results}`.

- Default: auto-follow `next` until the list is complete or `--limit` is reached.
- `--no-paginate` returns only the first page and includes `next_cursor` for manual continuation.
- `--limit N` caps rows, regardless of page boundary.
- `--cursor <c>` resumes from a previous `next_cursor`.
- JSON mode always emits a top-level `next_cursor` when there's more data.

## Interactive vs non-interactive

- `is_interactive()` = `Term::stdout().is_term()`. One function, used everywhere.
- In non-interactive mode:
  - Never prompt. If a required argument is missing, error with `code: VALIDATION, hint: "pass --<flag>"`.
  - Destructive actions (`delete`, `archive`) auto-confirm.
  - `--json` is implied.
  - Fuzzy-select pickers are disabled.

## JSON schema publication

The skill ships `references/schemas.json` listing the top-level shape of every command's `--json` output. Agents can fetch it for type-safe consumption. When a command's shape changes, the schema file changes in the same PR.

## Safety-critical output rules

Baked into the client. Non-negotiable even when agents pass `--full` or try to bypass.

### HogQL auto-`LIMIT`

`bosshogg query run` injects `LIMIT 100` if the parsed query has no `LIMIT` clause and `--no-limit` wasn't passed. Prevents accidental `SELECT * FROM events` flooding the context window. The injection is reported in the `--debug` trace so users see it.

### Session-recording snapshots never hit stdout

`bosshogg session-recording get --snapshot <id>` (when that command lands in M8) requires `--out <file>`. Snapshots are compressed rrweb JSONL — megabytes per session. If `--out` is missing AND stdout is not a TTY, the command errors with `VALIDATION` code and hint `"Pass --out <file> — snapshots can be megabytes"`. If stdout IS a TTY, it prints only summary metadata (duration, URL, console errors, click count) and tells the user how to get the raw snapshot.

### LLM trace bodies default summarized

For the LLM-observability workflow (queries via `bosshogg query` through the `$ai_*` event convention), helper subcommands (when they exist) default to returning model name, cost, latency, token counts, and prompt/response *summaries* — never the full message bodies. `--full` opts in explicitly.

### Error bodies truncated

API error response bodies are clipped to 200 chars in debug output and logs. Prevents accidental token/PII leakage. Lifted from the `lin` playbook's security review.

### HTTPS only

`reqwest` client is configured with `.https_only(true)`. Rejects redirects to non-HTTPS. Prevents a PostHog response that redirects to `http://` from being followed.

### Auth header redaction

`--debug` prints HTTP requests with `Authorization:` redacted to `Bearer <redacted>`. The full token is only emitted by `bosshogg auth token` on explicit request.

## Write inputs

Shell-escape hell is a real problem for agents piping JSON. BossHogg accepts file-based input for anything non-trivial:

- `--filters-file <path>` for flag filters, cohort queries, insight definitions
- `--description-file <path>` for markdown descriptions
- `--payload-file <path>` for flag JSON payloads
- `--query-file <path>` for HogQL (also `--file` shorthand on `bosshogg query run`)

Stdin also works: `bosshogg query run --file -` or piping from `cat` / `jq`.

## Versioning and deprecations

- **CalVer** `YYYY.MM.PATCH` — same as the `lin` playbook, accepted by crates.io.
- **Deprecated subcommands** stay functional for at least one minor version, print a one-line warning to stderr, and hide from `--help` under their replacement's section.
- **JSON schema changes**: additive (new fields) are non-breaking; removing or renaming fields is a SemVer-major bump.
