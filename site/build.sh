#!/usr/bin/env bash
# Build the arena site into site/dist: its pages, the player docs rendered from docs/*.md, the game (the Bevy app in
# crates/arena-game built for the browser, at dist/game) and the house bot module (dist/bots/policy.wasm, built from
# the house AI's ONNX model with the bots/onnx template, as a player builds theirs).
# Usage: site/build.sh            (from anywhere)
#        site/build.sh --serve    (build, then serve dist on http://127.0.0.1:8771/)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
out="$here/dist"
target="${CARGO_TARGET_DIR:-$root/target}"

(cd "$root" && cargo build --profile web -p arena-game --lib --target wasm32-unknown-unknown)
(cd "$root" && cargo run -q -p arena-bot --example onnx -- crates/pixfx-duel/weights/policy.onnx site/assets/bots/policy.wasm)

rm -rf "$out"
mkdir -p "$out/assets" "$out/game" "$out/bots"
wasm-bindgen --target web --no-typescript --out-dir "$out/game" "$target/wasm32-unknown-unknown/web/arena_game.wasm"
wasm-opt -Oz --strip-debug --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
  "$out/game/arena_game_bg.wasm" -o "$out/game/arena_game_bg.wasm"
cp "$here"/*.html "$out/"
cp "$here"/assets/arena.js "$here"/assets/arena.css "$here"/assets/menu.js "$here"/assets/site.css "$out/assets/"
cp "$here"/assets/bots/policy.wasm "$out/bots/"
# the big files gzipped beside themselves: the server sends a file's .gz to a browser that takes gzip
gzip -9 -k "$out/game/arena_game_bg.wasm" "$out/game/arena_game.js" "$out/bots/policy.wasm"
bun "$here/docs.js" "$root/docs" "$out/docs"
echo "arena site: $out"

if [[ "${1:-}" == "--serve" ]]; then
  cd "$out" && python3 -m http.server 8771 --bind 127.0.0.1
fi
