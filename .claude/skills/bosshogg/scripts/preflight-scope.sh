#!/usr/bin/env bash
# scripts/preflight-scope.sh <scope>
#
# Probes a PostHog scope via a benign GET, parses a 403 body for the missing
# scope name, and emits a remediation hint.
#
# Usage:
#   scripts/preflight-scope.sh feature_flag:read
#   scripts/preflight-scope.sh query:read
#   scripts/preflight-scope.sh insight:read
#
# Exit codes:
#   0  — scope is present (the benign request succeeded)
#   1  — scope is missing (403 returned with AUTH_SCOPE code)
#   2  — probe failed for a reason other than scope (network, auth_invalid)
#   10 — bad usage (missing arg) or missing dependencies

set -euo pipefail

SCOPE="${1:-}"

if [[ -z "$SCOPE" ]]; then
  printf 'usage: %s <scope>\n' "$0" >&2
  printf 'example scopes: feature_flag:read, query:read, insight:read, person:read\n' >&2
  exit 10
fi

if ! command -v bosshogg >/dev/null 2>&1; then
  printf 'PREFLIGHT: FAIL — bosshogg not on PATH\n' >&2
  exit 10
fi

if ! command -v jq >/dev/null 2>&1; then
  printf 'PREFLIGHT: FAIL — jq not on PATH\n' >&2
  exit 10
fi

if ! command -v curl >/dev/null 2>&1; then
  printf 'PREFLIGHT: FAIL — curl not on PATH\n' >&2
  exit 10
fi

# Resolve host + project + bearer
WHOAMI="$(bosshogg whoami --json 2>/dev/null || true)"
if ! printf '%s' "$WHOAMI" | jq -e '.project_id' >/dev/null 2>&1; then
  printf 'PREFLIGHT: FAIL — `bosshogg whoami` did not return a project. Run `bosshogg configure` first.\n' >&2
  exit 2
fi

HOST="$(printf '%s' "$WHOAMI" | jq -r '.host')"
PROJECT="$(printf '%s' "$WHOAMI" | jq -r '.project_id')"
TOKEN="$(bosshogg auth token 2>/dev/null || true)"

if [[ -z "$TOKEN" ]]; then
  printf 'PREFLIGHT: FAIL — no bearer available. Run `bosshogg configure` or set POSTHOG_CLI_TOKEN.\n' >&2
  exit 2
fi

# Map scope -> benign probe endpoint
case "$SCOPE" in
  feature_flag:read)
    URL="${HOST}/api/projects/${PROJECT}/feature_flags/?limit=1"
    ;;
  feature_flag:write)
    # No benign write probe; the read scope is a necessary prerequisite anyway
    URL="${HOST}/api/projects/${PROJECT}/feature_flags/?limit=1"
    ;;
  query:read)
    # SELECT 1 is the cheapest HogQL
    URL="${HOST}/api/environments/${PROJECT}/query/"
    ;;
  insight:read)
    URL="${HOST}/api/projects/${PROJECT}/insights/?limit=1"
    ;;
  cohort:read)
    URL="${HOST}/api/projects/${PROJECT}/cohorts/?limit=1"
    ;;
  person:read)
    URL="${HOST}/api/projects/${PROJECT}/persons/?limit=1"
    ;;
  dashboard:read)
    URL="${HOST}/api/projects/${PROJECT}/dashboards/?limit=1"
    ;;
  session_recording:read)
    URL="${HOST}/api/environments/${PROJECT}/session_recordings/?limit=1"
    ;;
  error_tracking:read)
    URL="${HOST}/api/projects/${PROJECT}/error_tracking/issue/?limit=1"
    ;;
  experiment:read)
    URL="${HOST}/api/projects/${PROJECT}/experiments/?limit=1"
    ;;
  survey:read)
    URL="${HOST}/api/projects/${PROJECT}/surveys/?limit=1"
    ;;
  hog_function:read)
    URL="${HOST}/api/projects/${PROJECT}/hog_functions/?limit=1"
    ;;
  batch_export:read)
    URL="${HOST}/api/environments/${PROJECT}/batch_exports/?limit=1"
    ;;
  *)
    printf 'PREFLIGHT: FAIL — unknown scope %s (this wrapper does not know a probe for it)\n' "$SCOPE" >&2
    printf 'Probing anyway with a generic projects/:pid read...\n' >&2
    URL="${HOST}/api/projects/${PROJECT}/"
    ;;
esac

# Perform probe
if [[ "$SCOPE" == "query:read" ]]; then
  BODY='{"query":{"kind":"HogQLQuery","query":"SELECT 1"}}'
  HTTP_CODE="$(curl -sS -o /tmp/bosshogg-preflight.$$ -w '%{http_code}' \
    -X POST \
    -H "Authorization: Bearer ${TOKEN}" \
    -H 'Content-Type: application/json' \
    --data "$BODY" \
    "$URL")"
else
  HTTP_CODE="$(curl -sS -o /tmp/bosshogg-preflight.$$ -w '%{http_code}' \
    -H "Authorization: Bearer ${TOKEN}" \
    "$URL")"
fi

RESPONSE_BODY="$(cat /tmp/bosshogg-preflight.$$ 2>/dev/null || true)"
rm -f /tmp/bosshogg-preflight.$$

case "$HTTP_CODE" in
  200|201|204)
    printf 'PREFLIGHT: OK — scope %s is present\n' "$SCOPE"
    exit 0
    ;;
  401)
    printf 'PREFLIGHT: FAIL — key is invalid or revoked (HTTP 401)\n' >&2
    printf 'Re-issue the personal API key at %s/settings/user-api-keys and update the context.\n' "$HOST" >&2
    exit 2
    ;;
  403)
    MISSING="$(printf '%s' "$RESPONSE_BODY" | jq -r '.detail // .message // "unknown"' 2>/dev/null || true)"
    printf 'PREFLIGHT: FAIL — scope %s is MISSING (HTTP 403)\n' "$SCOPE" >&2
    printf 'Server said: %s\n' "$MISSING" >&2
    printf '\nRemediation:\n' >&2
    printf '  1. Visit %s/settings/user-api-keys\n' "$HOST" >&2
    printf '  2. Create a new personal API key with scope `%s` added\n' "$SCOPE" >&2
    printf '  3. Update the active context:\n' >&2
    printf '       bosshogg config set-context $(bosshogg config current-context) --key-from-stdin\n' >&2
    printf '     (then paste the new key)\n' >&2
    exit 1
    ;;
  429)
    printf 'PREFLIGHT: FAIL — rate limited (HTTP 429). Wait and retry.\n' >&2
    exit 2
    ;;
  5*)
    printf 'PREFLIGHT: FAIL — PostHog upstream error (HTTP %s). Transient; retry.\n' "$HTTP_CODE" >&2
    exit 2
    ;;
  *)
    printf 'PREFLIGHT: FAIL — unexpected HTTP %s\n' "$HTTP_CODE" >&2
    printf 'Body: %s\n' "$(printf '%s' "$RESPONSE_BODY" | head -c 200)" >&2
    exit 2
    ;;
esac
