#!/usr/bin/env bash
# Write the arena duel's page kit into <dir>: the duel's page modules (duel*.js, skill-icons.js), the skill icon
# atlases, the fighters' and spell effects' sprite sheets (fighters/, vfx/), the arena's wasm (wasm/, built here from crates/arena-wasm) and the
# house bot module (bots/policy.wasm, built here from the house AI's ONNX model, crates/pixfx-duel/weights/policy.onnx,
# with the bots/onnx template, as a player builds theirs). A page that shows the duel loads the pixfx engine itself (its pixfx-canvas2d
# bundle at ../pkg/ from <dir>), then the kit.
# The arena site (site/build.sh) and the pixfx site (pixfx-rs site/build.sh) both build with this.
# Usage: projects/arena/site/embed.sh <dir>
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
out="${1:?usage: embed.sh <dir>}"

(cd "$root" && CARGO_TARGET_DIR="$root/target/wasm" wasm-pack build crates/arena-wasm --target web --out-dir ../../site/assets/wasm --no-pack)
mkdir -p "$here/assets/bots"
(cd "$root" && cargo run -q -p arena-bot --example onnx -- crates/pixfx-duel/weights/policy.onnx site/assets/bots/policy.wasm)

mkdir -p "$out/wasm" "$out/bots" "$out/fighters"
cp "$here"/assets/duel*.js "$here"/assets/skill-icons.js "$here"/assets/skills-*.png "$out/"
cp "$here"/assets/wasm/arena_wasm.js "$here"/assets/wasm/arena_wasm_bg.wasm "$out/wasm/"
cp "$here"/assets/bots/policy.wasm "$out/bots/"
cp "$here"/assets/fighters/*.png "$here"/assets/fighters/*.json "$out/fighters/"
# the spell effects drawn from sprite sheets (vfx/index.json and each effect's sheet), when there are any
if [ -f "$here/assets/vfx/index.json" ]; then
  mkdir -p "$out/vfx"
  cp "$here"/assets/vfx/*.png "$here"/assets/vfx/*.json "$out/vfx/"
fi
echo "arena kit: $out"
