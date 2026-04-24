# Security Policy

## Reporting a vulnerability

Email **aaron@aaronkwhite.com** with **"bosshogg security"** in the subject line.

Do not open a public GitHub issue for vulnerabilities. Include a description, reproduction
steps, and any relevant environment details. PGP available on request.

**Response SLA.** Single-maintainer project. Best-effort acknowledgement within 72 hours;
no bug bounty.

## In scope

- Secret leakage — API keys or tokens appearing in `--debug` output, log files, or
  structured error bodies
- Auth bypass — a code path that issues authenticated requests without a valid key
- HTTPS enforcement gaps — a release build that follows a redirect to `http://` or
  otherwise transmits credentials in cleartext
- Path traversal in file inputs (`--filters-file`, `--query-file`, `--out`, etc.)
- Supply chain — dependency compromise affecting published release artifacts (crates.io,
  GitHub Releases tarballs, Homebrew formula)

## Out of scope

- **PostHog backend vulnerabilities.** Report those to
  [PostHog's security team](https://posthog.com/security) directly.
- **`test-harness` feature code paths.** `BOSSHOGG_ALLOW_HTTP` and related HTTP-bypass
  logic are compiled out of release builds. Issues confined to that feature flag are not
  exploitable in any published artifact.
- **Denial of service via expensive HogQL queries.** PostHog's server-side rate limits
  (2400 query requests/hour) are the intended mitigation. BossHogg applies auto-`LIMIT 100`
  as a courtesy but makes no DoS guarantees.

## Security properties this project commits to

These are enforced in every release build:

- **HTTPS only.** `reqwest` is configured with `.https_only(true)`. The
  `BOSSHOGG_ALLOW_HTTP` bypass is feature-gated behind `test-harness` and not present in
  release binaries.
- **Auth header redaction.** `Authorization:` headers are replaced with
  `Bearer <redacted>` in `--debug` output. Error bodies are truncated to 200 chars. No
  tokens or PII leak to logs.
- **Config file permissions.** `~/.config/bosshogg/config.toml` is written with
  `mode(0o600)`. The `configure` command verifies the API key against PostHog before
  writing — a failed auth never persists credentials to disk.
- **Destructive-op gating.** Hard deletes and `bosshogg capture` require `--yes` or
  interactive TTY confirmation. No accidental bulk mutations.
- **Snapshot never-stdout.** Session recording snapshot blobs are suppressed from stdout
  by default; `--out <file>` is required to write the raw blob.

Provenance attestations (`actions/attest-build-provenance`) are attached to GitHub
Releases tarballs from v2026.4.8 onward.
