#!/usr/bin/env bash
# Put the pixfx engine the arena is pinned to at .pixfx/ (gitignored) and print its path. The site build and the site
# tests read the engine only from there, so the pages run the same engine rev as the rules in Cargo.toml.
# With $PIXFX set to a pixfx checkout, .pixfx/ links to it instead (working on the engine beside the arena:
# CONTRIBUTING.md); otherwise .pixfx/ is a clone of the engine, checked out at the rev in Cargo.toml.
# Usage: site/pixfx.sh
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/.pixfx"
url="https://github.com/aiocean/pixfx-rs"

if [[ -n "${PIXFX:-}" ]]; then
  engine="$(cd "$PIXFX" && pwd)"
  if [[ -e "$dir" && ! -L "$dir" ]]; then
    echo "pixfx.sh: $dir is a clone; remove it to link \$PIXFX there" >&2
    exit 1
  fi
  ln -sfn "$engine" "$dir"
  echo "$dir"
  exit 0
fi

rev="$(sed -n 's/^pixfx-draw = { git = "[^"]*", rev = "\([0-9a-f]*\)" }$/\1/p' "$root/Cargo.toml")"
if [[ -z "$rev" ]]; then
  echo "pixfx.sh: no pixfx-draw rev in $root/Cargo.toml" >&2
  exit 1
fi
if [[ -L "$dir" ]]; then
  rm "$dir"
fi
if [[ ! -d "$dir/.git" ]]; then
  git clone --quiet --filter=blob:none --no-checkout "$url" "$dir"
fi
if ! git -C "$dir" cat-file -e "$rev^{commit}" 2>/dev/null; then
  git -C "$dir" fetch --quiet origin
fi
git -C "$dir" checkout --quiet --detach "$rev"
echo "$dir"
