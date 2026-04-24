#!/bin/bash
# scripts/doctor.sh
#
# Wraps `bosshogg doctor --json` and emits a human-friendly, classified summary.
#
# Real JSON shape from `bosshogg doctor --json`:
#   {
#     "checks": [{"name": "...", "ok": bool, "message": "...", "remediation": "..."}],
#     "summary": {"ok": bool, "passed": N, "failed": N}
#   }
#
# Exit codes:
#   0  — all checks passed (summary.ok == true)
#   1  — one or more checks failed (summary.ok == false)
#   10 — bosshogg binary not on PATH or jq not on PATH
#   11 — `bosshogg doctor` did not emit valid JSON

set -euo pipefail

if ! command -v bosshogg >/dev/null 2>&1; then
  printf 'DOCTOR: FAIL — bosshogg binary not on PATH\n' >&2
  printf 'Install with: cargo install bosshogg\n' >&2
  printf '     or:      brew install aaronkwhite/tap/bosshogg\n' >&2
  exit 10
fi

if ! command -v jq >/dev/null 2>&1; then
  printf 'DOCTOR: FAIL — jq not on PATH (required by this wrapper)\n' >&2
  printf 'Install with: brew install jq  (or apt-get install jq)\n' >&2
  exit 10
fi

# Run doctor, capturing output; bosshogg doctor exits non-zero when checks fail
# but still emits valid JSON — capture both cases.
OUTPUT="$(bosshogg doctor --json 2>/dev/null)" || true

# Validate that the output is JSON at all.
if ! printf '%s' "$OUTPUT" | jq -e . >/dev/null 2>&1; then
  printf 'DOCTOR: FAIL — bosshogg doctor did not emit JSON\n' >&2
  printf '%s\n' "$OUTPUT" >&2
  exit 11
fi

# Validate the expected shape: must have .summary.ok
if ! printf '%s' "$OUTPUT" | jq -e '.summary.ok != null' >/dev/null 2>&1; then
  printf 'DOCTOR: FAIL — bosshogg doctor JSON missing .summary.ok field\n' >&2
  printf '%s\n' "$OUTPUT" >&2
  exit 11
fi

# Pass raw JSON to stdout so callers can pipe: doctor.sh | jq '.summary'
printf '%s\n' "$OUTPUT"

SUMMARY_OK="$(printf '%s' "$OUTPUT" | jq -r '.summary.ok')"
PASSED="$(printf '%s' "$OUTPUT" | jq -r '.summary.passed')"
FAILED="$(printf '%s' "$OUTPUT" | jq -r '.summary.failed')"

if [ "$SUMMARY_OK" = "true" ]; then
  printf 'DOCTOR: OK — %s checks passed, %s failed\n' "$PASSED" "$FAILED" >&2
  exit 0
else
  printf 'DOCTOR: FAIL — %s checks passed, %s failed\n' "$PASSED" "$FAILED" >&2
  printf '%s' "$OUTPUT" | jq -r '
    .checks[]
    | select(.ok == false)
    | "  [FAIL] " + .name + ": " + .message +
      (if .remediation then "\n         fix: " + .remediation else "" end)
  ' >&2
  exit 1
fi
