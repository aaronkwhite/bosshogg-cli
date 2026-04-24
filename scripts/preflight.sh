#!/usr/bin/env bash
# scripts/preflight.sh — release gate for BossHogg tags.
#
# Run before cutting a `v*` tag. Fails loudly on the first broken check so
# CI and humans get the same answer.
#
# Usage:
#   scripts/preflight.sh                  # checks working tree + lints + tests
#   scripts/preflight.sh 2026.4.0         # ALSO verifies CHANGELOG entry exists

set -euo pipefail

red()    { printf '\033[31m%s\033[0m\n' "$*" >&2; }
green()  { printf '\033[32m%s\033[0m\n' "$*"; }
yellow() { printf '\033[33m%s\033[0m\n' "$*"; }
blue()   { printf '\033[34m%s\033[0m\n' "$*"; }

step() { blue "==> $*"; }
fail() { red "FAIL: $*"; exit 1; }
ok()   { green "OK:   $*"; }

version="${1:-}"

cd "$(git rev-parse --show-toplevel)"

# ---------- 1. clean working tree ----------
step "clean working tree"
if [[ -n "$(git status --porcelain)" ]]; then
    git status --short >&2
    fail "working tree is dirty — commit or stash before tagging"
fi
ok "working tree clean"

# ---------- 2. toolchain pin present ----------
step "rust-toolchain.toml present"
if [[ ! -f rust-toolchain.toml ]]; then
    fail "rust-toolchain.toml missing — needed to pin the release toolchain"
fi
ok "rust-toolchain.toml present"

# ---------- 3. CHANGELOG entry (if a version was passed) ----------
if [[ -n "$version" ]]; then
    step "CHANGELOG.md entry for $version"
    if ! grep -qE "^## \\[$version\\]" CHANGELOG.md; then
        fail "CHANGELOG.md has no '## [$version]' section — add one before tagging"
    fi
    ok "CHANGELOG has entry for $version"

    step "Cargo.toml version matches $version"
    cargo_version=$(sed -n 's/^version = "\\(.*\\)"/\\1/p' Cargo.toml | head -1)
    if [[ "$cargo_version" != "$version" ]]; then
        fail "Cargo.toml version is '$cargo_version', expected '$version'"
    fi
    ok "Cargo.toml version matches"
else
    yellow "SKIP: CHANGELOG + Cargo.toml version check (no version passed)"
fi

# ---------- 4. cargo fmt ----------
step "cargo fmt --check"
PATH=~/.rustup/toolchains/1.95-aarch64-apple-darwin/bin:$PATH cargo fmt --all --check || fail "cargo fmt --check failed"
ok "cargo fmt clean"

# ---------- 5. cargo clippy ----------
step "cargo clippy --all-targets -- -D warnings"
PATH=~/.rustup/toolchains/1.95-aarch64-apple-darwin/bin:$PATH cargo clippy --all-targets --all-features -- -D warnings \
    || fail "clippy reported warnings"
ok "clippy clean"

# ---------- 6. cargo test (non-ignored) ----------
step "cargo test --all-features"
PATH=~/.rustup/toolchains/1.95-aarch64-apple-darwin/bin:$PATH cargo test --all-features || fail "cargo test failed"
ok "cargo test green"

# ---------- 7. cargo build --release ----------
step "cargo build --release"
PATH=~/.rustup/toolchains/1.95-aarch64-apple-darwin/bin:$PATH cargo build --release || fail "release build failed"
ok "release build succeeded"

green "========================================="
green "PREFLIGHT GREEN"
if [[ -n "$version" ]]; then
    green "Ready to tag v$version."
else
    green "(pass a version like \`scripts/preflight.sh 2026.4.0\` to gate CHANGELOG too)"
fi
green "========================================="
