# Glossary

PostHog terminology, especially the bits agents and newcomers often conflate.

## Identity

**Person** — the merged record of one user across all their `distinct_id`s. PostHog creates persons automatically when you capture an event with a new `distinct_id`, and merges them via `$identify` events. Persons have properties (set on the person) distinct from event properties.

**Distinct ID** — the ID you pass to `capture()`. Can be anonymous (device ID, cookie) or authenticated (user ID). One person can have many distinct IDs — they get merged via `$identify`. BossHogg's `bosshogg persons get <distinct_id>` resolves the distinct ID to the underlying person.

**User (PostHog user)** — different from a "person." A *user* is someone with a PostHog account who logs into the PostHog app. `bosshogg whoami` returns the PostHog user; `bosshogg persons …` operates on the people your product is tracking. Don't conflate.

**Group** — a first-class entity *other than* a person: an account, company, team, board, etc. Group analytics treat groups as the unit of aggregation ("show me DAU by account"). Groups have their own properties; events can be attributed to both a person and one or more groups.

## Events

**Event** — a single captured action: `{event: "button_clicked", distinct_id: "...", timestamp: "...", properties: {...}}`. Immutable after capture.

**Event definition** — metadata about an event *name* (not a specific event). `button_clicked` has one event definition with a description, tags, "verified" status, etc.

**Property definition** — metadata about a property *name*, scoped by its origin (event, person, group, session). Used for autocompletion and taxonomy management in the PostHog UI.

**Action** — a named matcher over events. Example: *"completed signup"* could match any of `signup_submit`, `registration_complete`, or `trial_start`. Actions appear in insights as a single named metric.

**Autocapture** — PostHog's automatic event capture for clicks, form submits, and pageviews. The events it produces show up as `$autocapture`, `$pageview`, `$pageleave`, `$set`, `$identify`, etc. — the `$` prefix denotes PostHog-internal event names.

## Analysis

**Insight** — a saved analysis. Insights have a *kind* (`TRENDS`, `FUNNELS`, `RETENTION`, `PATHS`, `STICKINESS`, `LIFECYCLE`, `HogQLQuery`) and carry a query definition. Each insight has a numeric `id` and a shorter `short_id` string usable in URLs.

**Dashboard** — a collection of insight tiles. Tiles reference insights (they don't contain them). Dashboards have their own sharing/subscription configuration; the insights they show remain independently editable.

**Tile** — an arrangement of one insight inside a dashboard (position, size). Editing a tile moves/resizes the insight in that dashboard; editing the insight changes the underlying analysis everywhere it's shown.

**Cohort** — a group of people matching a definition. *Static* cohorts have a fixed member list; *dynamic* cohorts are re-evaluated against the current state of persons + events. Cohorts can filter insights, segment experiments, and scope feature flags.

**Annotation** — a timestamped note pinned to charts. Release markers, incident windows, campaign launches. Shown as vertical lines on trend charts.

## Feature flags

**Feature flag** — a toggle + filter definition that the PostHog SDK evaluates server-side or client-side to decide a user's experience. Flags are either *boolean* (`on`/`off`) or *multivariate* (return one of several variants, each with a rollout percentage).

**Flag key** — the human-readable identifier (`new-onboarding-v2`). Don't confuse with the numeric `id`. BossHogg accepts both everywhere.

**Flag filter (`filters`)** — the JSON blob that defines who gets the flag. Properties include conditions (person property matches, cohort membership, group property), rollout percentages, and variant definitions. BossHogg exposes this as `--filters-file` for edits; the schema is fluid and best edited as JSON.

**Flag payload** — per-variant JSON payload returned alongside the variant name. Used for passing configuration per user. BossHogg supports `--payload-file`.

**Flag evaluation** — `POST /flags?v=2` decides what variant (or boolean) a given `distinct_id` gets. Takes person properties and group properties as input. `bosshogg flags evaluate --distinct-id <id>` hits this endpoint.

## Experiments

**Experiment** — a PostHog-orchestrated A/B test built on top of a multivariate feature flag. Experiments define exposure events, primary and secondary metrics, recommended run time, and a recommended sample size.

**Exposure cohort** — the set of people who actually saw a given experiment variant. Used for downstream analysis independent of the experiment's primary metric.

## Sessions and recordings

**Session** — a bounded stretch of activity from one person. PostHog defines session boundaries automatically (default 30-minute inactivity).

**Session recording** — rrweb-captured playback of a user's session. PostHog exposes metadata (`id`, `distinct_id`, `duration`, `start_time`, …) via API; the actual rrweb payload is a separate download. BossHogg exposes metadata only in v1.

## Surveys

**Survey** — an in-product prompt (multiple choice, open text, NPS, rating) shown to users matching a definition. Responses are captured as events. BossHogg exposes CRUD + activity + duplicate + response-archive.

## Pipeline

**Hog Function** — the modern CDP building block. Hog-scripted destinations, transformations, webhooks, Slack/SMS outputs. Replaces the old Plugins framework entirely. Has a scripting language (Hog), a schema, invocation logs, metrics, and backfill support.

**Batch export** — a scheduled job that ships events to S3, BigQuery, Snowflake, Postgres, Redshift, etc. Has runs, logs, retries, and backfills.

**Subscription** — a scheduled delivery of a dashboard or insight to email, Slack, or a webhook. Different from batch exports (which ship raw events); subscriptions ship rendered analytics.

## Error tracking

**Fingerprint** — a PostHog-generated stable identifier for a grouped set of errors (same stack, same type, same location).

**Assignment rule** — automatically routes matching errors to a user or team.

**Grouping rule** — overrides PostHog's default error-fingerprinting for a specific error shape.

## HogQL

**HogQL** — PostHog's ClickHouse-backed SQL dialect. Mostly standard SQL with PostHog-aware functions: `person()`, `event.$browser`, `properties.*`, etc. Supports arrays, maps, `argMin`/`argMax`, and window functions.

**Hog (the language)** — a programming language (not SQL) used inside Hog Functions for transforms, destination scripts, and CDP logic. Different from HogQL. PostHog ships `bin/hog` as its interpreter — which is why **our CLI is not called `hog`** (see [`naming.md`](naming.md)).

**Endpoint (materialized HogQL)** — a named, saved HogQL query exposed as its own REST endpoint, with a materialization cache. Git-versionable via YAML definitions.

## Scopes

**Project** — PostHog's unit of product separation. Each project has its own events, persons, flags, etc. Projects have numeric IDs (often referred to as `team_id` in older API paths, for historical reasons).

**Environment** — a scope *inside* a project, typically `production` / `staging`. PostHog is migrating the API from `/api/projects/:id/…` to `/api/environments/:id/…` — see [`api-notes.md`](api-notes.md). Conceptually: projects are what used to be called "teams"; environments are the newer split inside a project.

**Organization** — the top-level billing/permissions container. One org can contain many projects.

## Agent-side terms

**Skill** (Claude Code) — a named capability with an on-disk definition (`SKILL.md` + supporting files). The frontmatter loads at session start; the body loads when the skill is invoked. BossHogg ships one at `.claude/skills/bosshogg/`.

**MCP server** — Model Context Protocol server. Exposes typed tools to an agent over stdio or HTTP. Token-heavy at idle; rich when invoked.

**Idle context cost** — how many tokens a tool takes up *before* it's used, just by being registered. A CLI skill has very low idle cost; an MCP server has high idle cost.
