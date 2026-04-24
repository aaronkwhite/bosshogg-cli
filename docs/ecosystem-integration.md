# BossHogg Ecosystem Integration

How BossHogg relates to and integrates with PostHog's official tooling, adjacent SDKs,
and the broader agent/MCP ecosystem.

---

## `@posthog/cli` (official PostHog CLI)

**What it does:** Source-map uploads, dSYM/ProGuard symbolication for crash analytics,
release tracking, and (as of recent versions) HogQL query execution.

**Env-var compatibility:** BossHogg is compatible with `@posthog/cli`'s environment variables
so both tools can coexist in the same CI environment without separate configuration:

| Variable | `@posthog/cli` | BossHogg |
|---|---|---|
| `POSTHOG_CLI_TOKEN` | Personal API key | Primary auth (checked first) |
| `POSTHOG_CLI_API_KEY` | Alt form | Fallback |
| `POSTHOG_API_KEY` | Alt form | Second fallback |
| `POSTHOG_CLI_HOST` | API host | Primary host override |
| `POSTHOG_HOST` | Alt form | Fallback |
| `POSTHOG_CLI_PROJECT_ID` | Project ID | Project override |

**When to reach for `@posthog/cli` instead of BossHogg:**
- Uploading source maps after a JS/TS build (`posthog-cli sourcemaps upload`)
- Uploading dSYM files after an iOS build
- Uploading ProGuard mappings after an Android build
- Sending release events in your CD pipeline

BossHogg does not implement source-map or symbolication upload. Use `@posthog/cli` for those
and BossHogg for everything else (flag management, HogQL, persons, insights).

**Reference:** https://github.com/PostHog/posthog/tree/master/cli

---

## `posthog-rs` SDK (official Rust SDK)

**What it does:** Event capture and remote/local feature flag evaluation, embedded in your
Rust application.

**Relationship to BossHogg:** Complementary — different directions of data flow:

| | `posthog-rs` | BossHogg |
|---|---|---|
| Direction | **In** — capture from your app to PostHog | **Out** — read/write PostHog admin surface |
| Use case | App instrumentation, flag evaluation in-process | Terminal ops, CI scripts, agent loops |
| Auth | Project token (`phc_...`) | Personal API key (`phx_...`) |
| Primary surface | Event ingest, flag evaluation | Admin API (insights, persons, cohorts, etc.) |

A Rust app typically uses `posthog-rs` for instrumentation and BossHogg (in a separate
terminal or CI step) for operational tasks like checking flag rollout status or querying
the resulting events.

**Reference:** https://crates.io/crates/posthog-rs

---

## PostHog MCP server

**What it does:** Exposes 100+ PostHog capabilities as MCP tools, with full web-UI parity
including chart rendering and the Max AI assistant.

**When to reach for the PostHog MCP server instead of BossHogg:**
- You need rendered chart images (PNG/SVG) directly in the agent response.
- You want to use PostHog's Max AI for natural-language data exploration.
- You are in a wizard-driven flow (framework setup, funnel configuration) that benefits
  from MCP's richer interactive surface.
- Your agent has context budget to spare and you want the broadest possible surface in one load.

**When to reach for BossHogg:**
- You are in a CI script or a long-running agent loop where idle token cost matters.
- You need multi-context switching (separate prod/staging/EU environments).
- You need structured output with deterministic exit codes for scripting.
- Your agent model is already trained on BossHogg commands via the skill.

The two tools are not mutually exclusive. A common pattern: load the BossHogg skill for
routine flag/query operations, and fall back to the PostHog MCP server for chart rendering
or Max AI sessions. The BossHogg skill's `mcp-gaps.md` reference documents exactly which
operations require the MCP server.

**Reference:** https://posthog.com/docs/model-context-protocol

---

## PostHog Wizard

**What it does:** Framework-specific setup automation — detects the framework (Next.js,
React, Django, etc.) and walks the user through installing the PostHog SDK, adding the
snippet, and configuring the first events.

**How the BossHogg skill complements Wizard skills:**

The Wizard handles SDK installation and first-capture wiring. BossHogg takes over after
that: once the PostHog SDK is installed and events are flowing, BossHogg (via its Claude
Code skill) handles the operational layer — checking event volumes via HogQL, creating
insights, setting up feature flags, and debugging specific users.

A combined agent session might:
1. Use a framework-specific Wizard skill to install PostHog SDK in a Next.js app.
2. Use `bosshogg capture event` to fire a test event and verify ingestion.
3. Use `bosshogg query run` to confirm the event appears in ClickHouse.
4. Use `bosshogg flag create` to gate the new feature behind a flag.

**Reference:** https://github.com/PostHog/wizard

---

## Eventual PR to `PostHog/skills`

If BossHogg's Claude Code skill proves useful to the broader PostHog community, the skill
can be submitted to the PostHog skills marketplace.

**Submission procedure:**

1. Ensure the skill passes the eval gate (≥90% on Opus, idle tokens < 300).
2. Fork `PostHog/skills` and add `.claude/skills/bosshogg/` as a top-level skill directory.
3. Update the `marketplace.json` registry entry with the skill's name, description, and
   install instructions.
4. Open a PR with:
   - Title: `feat(skill): bosshogg — agent-first PostHog CLI skill`
   - Description covering: what the skill does, what commands it teaches, the eval pass rate,
     idle-token cost, and any known limitations.
   - A link to the BossHogg GitHub repo and the `evals/evals.json` file.
5. Address review feedback from the PostHog team.

**What goes in the PR description:**
- One-paragraph summary of what BossHogg teaches agents.
- Eval metrics (Opus pass rate, idle tokens).
- Scope: which of the 25 resources the skill covers.
- Known gaps (see `mcp-gaps.md`).
- Compatibility: Claude Code only for v1; MCP stdio transport planned for v1.1.

---

## Eventual PR to `PostHog/wizard` — framework-detector extension

The PostHog Wizard framework-detector could be extended to recognize BossHogg as an
installed operational tool and offer to wire it into CI scripts or agent config files.

**Concept:**

When the Wizard detects that `bosshogg` is on `PATH` (or that `bosshogg` appears in
`Cargo.toml` dev-dependencies), it could offer:
- Auto-generating a `bosshogg configure` step in the CI pipeline.
- Adding the BossHogg skill to `.claude/skills/` if a Claude Code project is detected.
- Generating a starter `bosshogg flag create` command for the first feature flag.

**Pseudocode for the detector extension:**

```python
def detect_bosshogg(project_root: Path) -> Optional[BossHoggDetection]:
    """Detect BossHogg installation and suggest integration steps."""

    # Check if binary is available
    if shutil.which("bosshogg"):
        return BossHoggDetection(
            installed=True,
            version=run(["bosshogg", "--version"]).stdout.strip(),
        )

    # Check if it's a Rust project that could install it
    cargo_toml = project_root / "Cargo.toml"
    if cargo_toml.exists() and "bosshogg" in cargo_toml.read_text():
        return BossHoggDetection(
            installed=False,
            installable=True,
            install_hint="cargo install bosshogg",
        )

    return None


def suggest_bosshogg_integration(detection: BossHoggDetection, ci_type: str) -> list[Step]:
    steps = []

    if not detection.installed:
        steps.append(InstallStep("cargo install bosshogg"))

    steps.append(ConfigureStep(
        description="Add POSTHOG_CLI_TOKEN to CI secrets",
        env_var="POSTHOG_CLI_TOKEN",
    ))

    if ci_type == "github-actions":
        steps.append(YAMLStep(
            name="BossHogg doctor",
            run="bosshogg doctor --json | jq '.ok'",
        ))

    return steps
```

**PR target:** `PostHog/wizard` — open after BossHogg v1.0 ships and has real user adoption.
The PR description should include adoption metrics (GitHub stars, crates.io downloads,
skill installs) to justify the maintenance burden on the Wizard team.
