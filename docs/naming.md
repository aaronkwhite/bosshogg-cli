# Naming

Short doc capturing the naming decision and the options we considered. Reference for anyone wondering "why isn't this just called `hog`?"

## Decision

- **Crate name:** `bosshogg`
- **Binary name:** `bosshogg`
- **Brand (marketing capitalization):** *BossHogg*

Users typing it will probably alias it: `alias bh=bosshogg`. The install docs recommend that.

## Why not `hog`?

PostHog itself ships a CLI called `hog` — it's the interpreter for their [**Hog programming language**](https://posthog.com/docs/hog) (the language behind HogQL, realtime destinations, custom transforms, and CDP functions). A PostHog user running both PostHog's `bin/hog` and a community `hog` CLI would hit PATH collisions and conceptual confusion: is `hog foo.hog` running a script or querying a project?

Shipping a PostHog-adjacent tool that shadows PostHog's first-party namespace is a footgun for users and an unforced error for the project. Hard no.

## Why not `phog`?

- Binary name `phog` is free on PATH.
- Crate name `phog` is taken on crates.io (a Slint-based photo gallery, inactive but published).
- Brand is punchy but loses the whimsy of the "Boss Hogg" nod.
- Two strikes → skipped.

## Why not `posthog-cli` / bare `posthog`?

- `posthog-cli` (crate + `@posthog/cli` npm) is the **official** Rust CLI from PostHog Inc., actively maintained. Shipping a community crate with the same name is not an option.
- The bare `posthog` crate name is technically free, but squatting it risks trademark friction with PostHog Inc. and signals bad intent. We don't do that — we complement, we don't shadow.

## Why `bosshogg`?

- **Whimsy-on-brand.** PostHog has a hedgehog mascot. *BossHogg* — the double-g spelling of *Dukes of Hazzard*'s Boss Hogg — is the PostHog-shaped pun equivalent of "the agent-first tool that handles the hard parts." Distinctive enough to Google.
- **Free everywhere we looked:** crates.io, npm, Homebrew formulae/casks, GitHub. No active project of that name.
- **Unambiguous.** No collision with PostHog's own `hog` language interpreter, no collision with unrelated CLI tools, no collision with widely-known binaries.
- **SEO-compatible.** Rare enough that `site:bosshogg.dev` and `crates.io/crates/bosshogg` will be trivially findable without fighting for "posthog cli" keyword turf (which PostHog Inc. owns anyway — see [`vision-and-positioning.md`](vision-and-positioning.md)).
- **One name for two things.** Crate + binary = same token. Users don't juggle "cargo install X, then run Y." Similar to how `ripgrep` the crate ships `rg` as its binary; we flip it and keep one token, accepting the slightly-longer binary name.

## Alternatives for a post-v1 short alias

If `bosshogg` the binary name wears thin, these were the next-best candidates:

- `bh` — two-letter, unclaimed on Homebrew and crates.io (double-check before shipping). Risk: too generic (`bh` is a shell-completion/backup alias in various dotfiles setups).
- `bossh` — rare and doesn't collide with `ssh` + `bash` associations obviously enough.
- `hog-cli` — free crate name but drops the whimsy. Fallback if trademark friction ever arises.

We don't need to decide on any of these in v1. Ship `bosshogg`, alias as you like, revisit if the name becomes a real adoption blocker (it won't).

## References

- PostHog Hog language docs: `https://posthog.com/docs/hog`
- Full naming research: [`../research/competitors-and-naming.md`](../research/competitors-and-naming.md)
