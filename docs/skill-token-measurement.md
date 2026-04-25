# Skill idle-token measurement

How to validate the README's `~200 idle tokens` claim for the BossHogg Claude Code skill.

> **Why measure.** The README's headline is a numeric claim ("~220x reduction in idle context cost vs the PostHog MCP server"). It only holds if the skill actually loads in ~200 tokens of frontmatter and defers everything else to references that load on demand. Drift in `SKILL.md` — extra prose, additional bullets, longer description — adds idle tokens to every Claude Code session that has the skill installed. Re-measure before each release.

## Prerequisites

- Claude Code installed locally.
- A reasonably empty Claude Code project (no other heavy skills loaded).
- The bosshogg skill installed:
  ```bash
  /plugin marketplace add aaronkwhite/bosshogg-cli
  /plugin install bosshogg@bosshogg
  ```
  Confirm via `/plugin` that `bosshogg` shows enabled.

## Procedure

### 1. Capture the cold-session baseline

Open a fresh Claude Code session in a project directory that does **not** load other custom skills. Send a single trivial message (`hi`) and immediately note the input-token usage from the session's status line or the `/cost` view.

Record:

- **Baseline tokens** — Claude Code's own system prompt + tool definitions + the bosshogg skill frontmatter, minus the user message. Subtract the user message length to isolate the skill's contribution.

### 2. Disable the skill, repeat

`/plugin disable bosshogg`, restart the session, send `hi` again, record tokens.

The delta — `baseline − without_skill` — is the **idle token cost** of the skill.

Target: **≤ 250 tokens**. Above that, the README's `~200` claim drifts.

### 3. Trigger the skill, observe full body load

In the same session, send a message that exercises bosshogg (e.g. `using bosshogg, list my feature flags`). Verify:

- The skill activates (Claude announces it).
- Token count jumps as `SKILL.md` body and any referenced `references/*.md` files load.
- The jump is bounded — the on-demand body should be a few thousand tokens, not 44 k.

The point isn't to minimize the on-demand cost; it's to confirm the cost is **on-demand**, not idle.

## Acceptance gates

| Metric | Target | Action if exceeded |
|---|---|---|
| Idle token cost | ≤ 250 | Trim `SKILL.md` frontmatter (description + name + triggers); push references into `references/*.md`. |
| Description length | ≤ 1024 chars | Sharpen wording. The description IS the idle cost driver. |
| On-demand body load | ≤ 8 k tokens | Split `SKILL.md` body into smaller `references/` files loaded conditionally. |

## How the number gets to the README

Once measured, update the figure in two places:

1. `README.md` — the comparison table (`~200 tokens (frontmatter only)`) and the prose ("BossHogg's ~200-token target is based on the skill frontmatter size").
2. The `description` field in `.claude/skills/bosshogg/.claude-plugin/marketplace.json` — verify it hasn't grown.

Re-measure on every release that changes `SKILL.md` or the marketplace manifest. Don't ship a version bump that changes either without re-running this procedure.

## Reproducibility

- Capture the Claude Code version (`claude --version`) and the bosshogg version (`bosshogg --version`) alongside any measurement.
- The token figures depend on Claude Code's own system-prompt size and may shift across CC releases. Always re-baseline against a current Claude Code; don't compare across versions.

## When to skip

If `SKILL.md` and `marketplace.json` are byte-identical to the previous measured release, the idle cost is unchanged. Skip with a CHANGELOG note ("skill unchanged since vX.Y.Z; idle tokens not re-measured").
