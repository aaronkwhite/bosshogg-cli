#!/usr/bin/env bash
# scripts/hogql-schema-dump.sh [--refresh]
#
# Caches `bosshogg schema hogql` per-project under ~/.cache/bosshogg/schema-<pid>.json
# with a 24-hour TTL. Reuses cached schema across calls; call with --refresh to
# invalidate.
#
# Usage:
#   CACHE=$(scripts/hogql-schema-dump.sh)
#   jq '.tables | length' "$CACHE"
#
# On success, prints the cache path on stdout and summary to stderr.
# On failure, exits non-zero.
#
# Exit codes:
#   0  — cache is fresh (or was just refreshed) and valid
#   1  — fetch failed
#   10 — missing dependency or no active project

set -euo pipefail

REFRESH=0
if [[ "${1:-}" == "--refresh" ]]; then
  REFRESH=1
fi

if ! command -v bosshogg >/dev/null 2>&1; then
  printf 'SCHEMA: FAIL — bosshogg not on PATH\n' >&2
  exit 10
fi

if ! command -v jq >/dev/null 2>&1; then
  printf 'SCHEMA: FAIL — jq not on PATH\n' >&2
  exit 10
fi

# Resolve cache dir (XDG-aware)
CACHE_DIR="${XDG_CACHE_HOME:-$HOME/.cache}/bosshogg"
mkdir -p "$CACHE_DIR"
chmod 700 "$CACHE_DIR" 2>/dev/null || true

# Resolve project id via whoami
WHOAMI="$(bosshogg whoami --json 2>/dev/null || true)"
if ! printf '%s' "$WHOAMI" | jq -e '.project_id' >/dev/null 2>&1; then
  printf 'SCHEMA: FAIL — `bosshogg whoami` did not return project_id. Run `bosshogg configure` first.\n' >&2
  exit 10
fi
PROJECT="$(printf '%s' "$WHOAMI" | jq -r '.project_id')"

CACHE_FILE="${CACHE_DIR}/schema-${PROJECT}.json"
CACHE_MAX_AGE_SECONDS=$((24 * 60 * 60)) # 24 hours

# Determine whether cache is fresh
needs_refresh=1
if [[ "$REFRESH" -eq 0 && -f "$CACHE_FILE" ]]; then
  if command -v stat >/dev/null 2>&1; then
    # macOS and GNU stat differ — try both
    if mtime=$(stat -f %m "$CACHE_FILE" 2>/dev/null); then
      :
    elif mtime=$(stat -c %Y "$CACHE_FILE" 2>/dev/null); then
      :
    else
      mtime=0
    fi
    now=$(date +%s)
    age=$(( now - mtime ))
    if [[ "$age" -lt "$CACHE_MAX_AGE_SECONDS" ]]; then
      needs_refresh=0
    fi
  fi
fi

if [[ "$needs_refresh" -eq 1 ]]; then
  printf 'SCHEMA: refreshing cache for project %s\n' "$PROJECT" >&2
  TMP="$(mktemp "${CACHE_DIR}/schema-${PROJECT}.XXXXXX.json")"
  if ! bosshogg schema hogql --json > "$TMP" 2>/dev/null; then
    printf 'SCHEMA: FAIL — `bosshogg schema hogql` failed\n' >&2
    rm -f "$TMP"
    exit 1
  fi
  # Validate JSON
  if ! jq -e '.tables' < "$TMP" >/dev/null 2>&1; then
    printf 'SCHEMA: FAIL — schema JSON missing .tables field\n' >&2
    rm -f "$TMP"
    exit 1
  fi
  mv -f "$TMP" "$CACHE_FILE"
  chmod 600 "$CACHE_FILE" 2>/dev/null || true
else
  printf 'SCHEMA: using fresh cache for project %s\n' "$PROJECT" >&2
fi

# Summarize to stderr
TABLE_COUNT="$(jq '.tables | length' "$CACHE_FILE")"
printf 'SCHEMA: %s tables available at %s\n' "$TABLE_COUNT" "$CACHE_FILE" >&2

# Emit path on stdout so callers can pipe it:
#   CACHE=$(scripts/hogql-schema-dump.sh) && jq '.tables[0]' "$CACHE"
printf '%s\n' "$CACHE_FILE"
