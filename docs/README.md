# BossHogg Documentation

This folder is the **design-time source of truth** for BossHogg. Implementation will live in `src/`; rationale, scope, and conventions live here. Docs are meant to be read in order the first time, then dipped into by topic.

## Reading order

1. [**Vision & positioning**](vision-and-positioning.md) — What BossHogg is, why it exists, how it sits next to PostHog's official tooling.
2. [**V1 scope**](v1-scope.md) — Explicit in/out list with rationale. Start here before touching the schema.
3. [**Capability surface**](capabilities.md) — Human-readable catalog of resources and subcommands.
4. [**Architecture**](architecture.md) — Project layout, HTTP client, auth, output, config. Adapted from the `lin` CLI playbook for a REST API.
5. [**Conventions**](conventions.md) — JSON output contract, error shape, exit codes, env vars, soft-delete behavior.
6. [**Agent-first design**](agent-first.md) — The skill-over-MCP thesis; when to use BossHogg vs MCP; the Claude Code skill we ship with the repo.
7. [**Naming**](naming.md) — Why `bosshogg`, why not `hog`, why not `phog`.
8. [**PostHog API notes**](api-notes.md) — Non-obvious API quirks BossHogg has to handle: soft-delete, environments-vs-projects migration, rate-limit buckets, HogQL centrality, deprecated endpoints.
9. [**Glossary**](glossary.md) — PostHog terminology, especially for readers (and agents) new to the platform.
10. [**Development**](development.md) — Build, test, toolchain, CI intent.

## Supporting artifacts

- [`../research/`](../research/) — raw research outputs from the kickoff phase. Authoritative on API surface and competitive landscape; the `docs/` files are a refinement of this material.
- [`../research/capability-schema.yaml`](../research/capability-schema.yaml) — machine-readable schema of CLI resources and subcommands. `docs/capabilities.md` is its human-readable twin; keep them in sync.

## Documentation conventions

- **Factual claims about PostHog** should link to posthog.com/docs/… (or note "as of YYYY-MM-DD" if behavior may drift).
- **Design decisions** should answer three questions: *what did we decide, what did we consider, why did we pick this.*
- **Avoid restating API details** that already live in `research/posthog-api.md`; link to it instead.
- **Headings use sentence case.** Command examples use `bosshogg …` (the binary name); marketing prose uses *BossHogg* (the brand).
