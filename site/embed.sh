#!/usr/bin/env bash
# Write the arena duel's page kit into <dir>: the duel's page modules (duel*.js, skill-icons.js), the skill icon
# atlases, the arena's wasm (wasm/, built here from crates/arena-wasm) and the trained policies (weights/*.bin). A page
# that shows the duel loads the pixfx engine itself (its pixfx-canvas2d bundle at ../pkg/ from <dir>), then the kit.
# The arena site (site/build.sh) and the pixfx site (tools/pixfx-rs/site/build.sh) both build with this.
# Usage: projects/arena/site/embed.sh <dir>
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
out="${1:?usage: embed.sh <dir>}"

(cd "$root" && CARGO_TARGET_DIR="$root/target/wasm" wasm-pack build crates/arena-wasm --target web --out-dir ../../site/assets/wasm --no-pack)

mkdir -p "$out/wasm" "$out/weights"
cp "$here"/assets/duel*.js "$here"/assets/skill-icons.js "$here"/assets/skills-*.png "$out/"
cp "$here"/assets/wasm/arena_wasm.js "$here"/assets/wasm/arena_wasm_bg.wasm "$out/wasm/"
cp "$root"/crates/pixfx-duel/weights/*.bin "$out/weights/"
echo "arena kit: $out"
