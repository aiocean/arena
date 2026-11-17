#!/usr/bin/env bash
# Build the arena site into site/dist: its pages, the player docs rendered from docs/*.md, the game (the Bevy app in
# crates/arena-game built for the browser, at dist/game, its pictures at dist/game/assets) and the house bot module (dist/bots/policy.wasm, built from
# the house AI's ONNX model with the bots/onnx template, as a player builds theirs).
# Usage: site/build.sh            (from anywhere)
#        site/build.sh --serve    (build, then serve dist on http://127.0.0.1:8771/)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
out="$here/dist"
target="${CARGO_TARGET_DIR:-$root/target}"

# the browser's WebTransport bindings (watching live matches, crates/arena-game/src/live.rs) are behind this cfg
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="--cfg=web_sys_unstable_apis"
(cd "$root" && cargo build --profile web -p arena-game --lib --target wasm32-unknown-unknown)

rm -rf "$out"
mkdir -p "$out/assets" "$out/game" "$out/bots"
(cd "$root" && cargo run -q -p arena-bot --example onnx -- crates/arena-duel/weights/policy.onnx "$out/bots/policy.wasm")
wasm-bindgen --target web --no-typescript --out-dir "$out/game" "$target/wasm32-unknown-unknown/web/arena_game.wasm"
wasm-opt -Oz --strip-debug --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
  "$out/game/arena_game_bg.wasm" -o "$out/game/arena_game_bg.wasm"
# the game's pictures, which it loads beside its wasm: the fighters' and the effects' sheets (baked and drawn)
mkdir -p "$out/game/assets/fighters" "$out/game/assets/vfx" "$out/game/assets/drawn"
cp "$root"/crates/arena-game/assets/fighters/*.png "$out/game/assets/fighters/"
cp "$root"/crates/arena-game/assets/vfx/*.webp "$out/game/assets/vfx/"
cp "$root"/crates/arena-game/assets/drawn/*.webp "$out/game/assets/drawn/"
cp "$here"/*.html "$out/"
# where the page watches the server's live matches (crates/arena-server): the relay's URL with a subscribe-only token,
# from the environment, never from the repo. Without it the page fights on its own, as before.
if [[ -n "${ARENA_LIVE_URL:-}" ]]; then
  printf '{"url":"%s"}\n' "$ARENA_LIVE_URL" > "$out/live.json"
fi
cp "$here"/assets/arena.js "$here"/assets/arena.css "$here"/assets/menu.js "$here"/assets/site.css "$out/assets/"
# the big files compressed beside themselves: the server (game-factory backend/web, games-server) sends a file's .br to
# a browser that takes brotli, else its .gz. Brotli at its best is a third smaller than gzip on the game's wasm (4.5 MB
# against 6.7 MB). Without a `brotli` on the machine the site ships gzip only.
big=("$out/game/arena_game_bg.wasm" "$out/game/arena_game.js" "$out/bots/policy.wasm")
gzip -9 -k "${big[@]}"
if command -v brotli >/dev/null; then
  brotli -q 11 -k "${big[@]}"
else
  echo "site/build.sh: no brotli here, gzip only" >&2
fi
bun "$here/docs.js" "$root/docs" "$out/docs"
echo "arena site: $out"

if [[ "${1:-}" == "--serve" ]]; then
  cd "$out" && python3 -m http.server 8771 --bind 127.0.0.1
fi
