#!/usr/bin/env bash
# Build the arena site into site/dist: its pages, the player docs rendered from docs/*.md, the game (the Bevy app in
# crates/arena-game built for the browser, at dist/game/<hash>, its pictures at dist/game/assets-<hash>) and the house
# bot module (dist/bots/policy.wasm, built from the house AI's ONNX model with the bots/onnx template, as a player builds
# theirs). Everything under dist/game is named by its content and kept by browsers and the CDN for a year
# (dist/game/_headers); the pages, arena.js (which names the game's folder) and live.json are checked on every visit.
# Usage: site/build.sh            (from anywhere)
#        site/build.sh --serve    (build, then serve dist on http://127.0.0.1:8771/)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
out="$here/dist"
target="${CARGO_TARGET_DIR:-$root/target}"

# SHA-256 of the files named, or of what is piped in; `hash`: its first 12 hex digits
sum() { if command -v sha256sum >/dev/null; then sha256sum "$@"; else shasum -a 256 "$@"; fi; }
hash() { sum | cut -c1-12; }

# the game's pictures (the fighters' and the effects' sheets, baked and drawn), in a folder named by their content,
# which the game's wasm is built to load from
pictures=$(cd "$root/crates/arena-game/assets" && find fighters vfx drawn -type f \( -name '*.png' -o -name '*.webp' \) | LC_ALL=C sort)
assets="assets-$(cd "$root/crates/arena-game/assets" && echo "$pictures" | xargs sum | hash)"
export ARENA_ASSETS="game/$assets"

# the browser's WebTransport bindings (watching live matches, crates/arena-game/src/live.rs) are behind this cfg
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="--cfg=web_sys_unstable_apis"
(cd "$root" && cargo build --profile web -p arena-game --lib --target wasm32-unknown-unknown)

rm -rf "$out"
mkdir -p "$out/assets" "$out/game" "$out/bots"
(cd "$root" && cargo run -q -p arena-bot --example onnx -- crates/arena-duel/weights/policy.onnx "$out/bots/policy.wasm")
staged="$out/game/staged"
wasm-bindgen --target web --no-typescript --out-dir "$staged" "$target/wasm32-unknown-unknown/web/arena_game.wasm"
wasm-opt -Oz --strip-debug --enable-bulk-memory --enable-nontrapping-float-to-int --enable-sign-ext --enable-mutable-globals \
  "$staged/arena_game_bg.wasm" -o "$staged/arena_game_bg.wasm"
# the game's wasm and its JS in a folder named by their content (the JS finds the wasm beside itself)
game="$(cat "$staged/arena_game_bg.wasm" "$staged/arena_game.js" | hash)"
mv "$staged" "$out/game/$game"
(cd "$root/crates/arena-game/assets" && echo "$pictures" | while read -r f; do mkdir -p "$out/game/$assets/$(dirname "$f")"; cp "$f" "$out/game/$assets/$f"; done)
# the fighters' sheets as lossless WebP (the same pixels, 2.0 MB against 2.8 MB of PNG), which the game loads on the page
pairs=()
for png in "$out/game/$assets"/fighters/*.png; do pairs+=("$png" "${png%.png}.webp"); done
(cd "$root" && cargo run -q --release -p arena-vfx --example webp -- "${pairs[@]}" >/dev/null)
rm "$out/game/$assets"/fighters/*.png
printf '/*\n  Cache-Control: public, max-age=31536000, immutable\n' > "$out/game/_headers"
cp "$here"/*.html "$out/"
sed -i.bak "s#\"game/arena_game#\"game/$game/arena_game#g" "$out/index.html" && rm "$out/index.html.bak"
# where the page watches the server's live matches (crates/arena-server): the relay's URL with a subscribe-only token,
# from the environment, never from the repo. Without it the page fights on its own, as before.
if [[ -n "${ARENA_LIVE_URL:-}" ]]; then
  printf '{"url":"%s"}\n' "$ARENA_LIVE_URL" > "$out/live.json"
fi
cp "$here"/assets/arena.js "$here"/assets/arena.css "$here"/assets/menu.js "$here"/assets/site.css "$out/assets/"
sed -i.bak "s#\"../game/arena_game.js\"#\"../game/$game/arena_game.js\"#" "$out/assets/arena.js" && rm "$out/assets/arena.js.bak"
for f in "$out/assets/arena.js" "$out/index.html"; do
  grep -q "game/$game/arena_game.js" "$f" || { echo "site/build.sh: $f does not name the game's folder" >&2; exit 1; }
done
# the big files compressed beside themselves: the server (game-factory backend/web, games-server) sends a file's .br to
# a browser that takes brotli, else its .gz. Brotli at its best is a third smaller than gzip on the game's wasm (4.5 MB
# against 6.7 MB). Without a `brotli` on the machine the site ships gzip only.
big=("$out/game/$game/arena_game_bg.wasm" "$out/game/$game/arena_game.js" "$out/bots/policy.wasm")
gzip -9 -k "${big[@]}"
if command -v brotli >/dev/null; then
  brotli -q 11 -k "${big[@]}"
else
  echo "site/build.sh: no brotli here, gzip only" >&2
fi
bun "$here/docs.js" "$root/docs" "$out/docs"
echo "arena site: $out (game/$game, game/$assets)"

if [[ "${1:-}" == "--serve" ]]; then
  cd "$out" && python3 -m http.server 8771 --bind 127.0.0.1
fi
