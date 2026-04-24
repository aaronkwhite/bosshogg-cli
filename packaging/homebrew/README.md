# BossHogg Homebrew Formula

This directory contains the Homebrew formula template for the
[`aaronkwhite/homebrew-tap`](https://github.com/aaronkwhite/homebrew-tap) tap.

## Installing BossHogg via Homebrew

```bash
brew tap aaronkwhite/tap
brew install bosshogg
```

Or in a single command:

```bash
brew install aaronkwhite/tap/bosshogg
```

## How the formula works

`bosshogg.rb` is a multi-platform formula that downloads the prebuilt binary tarball
matching the user's OS and architecture from the GitHub Release for the tagged version.
It installs the binary to `bin/` and auto-generates shell completions via clap's
`bosshogg completion` subcommand.

## Tap maintainer: updating SHA256 after a release

After each tagged release the release workflow computes SHA256 checksums for the
four tarballs and pushes an updated `bosshogg.rb` to the tap repo. The placeholders
`<sha256-placeholder>` in this file are replaced by the workflow.

**Manual procedure** (fallback if the workflow didn't run):

1. Download the four tarballs from the GitHub Release page:
   - `bosshogg-aarch64-apple-darwin.tar.gz`
   - `bosshogg-x86_64-apple-darwin.tar.gz`
   - `bosshogg-aarch64-unknown-linux-gnu.tar.gz`
   - `bosshogg-x86_64-unknown-linux-gnu.tar.gz`

2. Compute SHA256 for each:
   ```bash
   sha256sum bosshogg-*.tar.gz
   # or on macOS:
   shasum -a 256 bosshogg-*.tar.gz
   ```

3. Replace each `<sha256-placeholder>` in `bosshogg.rb` with the corresponding hash.

4. Update the `version` field if it differs from the current release.

5. Commit and push to `aaronkwhite/homebrew-tap`:
   ```bash
   git add bosshogg.rb
   git commit -m "bosshogg v<version>"
   git push
   ```

6. Verify the formula installs cleanly on macOS arm64:
   ```bash
   brew install --build-from-source ./bosshogg.rb  # local test
   brew audit --strict bosshogg                     # lint check
   ```

## Release workflow automation

The `.github/workflows/release.yml` in the main repo contains a commented-out
`homebrew_tap` job that automates steps 2–5. See the TODO block in that file for
the one-time setup required to enable it (create the tap repo + deploy key).
