#!/usr/bin/env bash
# Optimize all PNGs in images/ and brand/ using oxipng.
# Install: cargo install oxipng  OR  brew install oxipng
set -euo pipefail

if ! command -v oxipng &>/dev/null; then
  echo "oxipng not found. Install with: cargo install oxipng  OR  brew install oxipng"
  exit 1
fi

find images brand -name "*.png" -print0 | xargs -0 oxipng -o 6 --strip safe
echo "Done."
