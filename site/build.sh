#!/usr/bin/env bash
# Build the arena site into site/dist: its pages, the player docs rendered from docs/*.md, the game (the Bevy app in
# crates/arena-game built for the browser, at dist/game/<hash>, its pictures at dist/game/assets-<hash>) and the house
# bot module (dist/bots/policy.wasm, built from the house AI's ONNX model with the bots/onnx template, as a player builds
# theirs). Everything under dist/game is named by its content and kept by browsers and the CDN for a year
# (dist/game/_headers); the pages and live.json are checked on every visit. arena.js lives in the game's folder too: the
# CDN keeps a .js for hours whatever the server says, and an arena.js kept past a deploy named a folder gone (a 404).
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

# the game's pictures as the page gets them (the fighters' and the effects' sheets, baked and drawn; the fighters'
# as lossless WebP, the same pixels as their PNGs, 2.0 MB against 2.8 MB), in a folder named by the files shipped,
# which the game's wasm is built to load from
pictures="$target/site-pictures"
rm -rf "$pictures" && mkdir -p "$pictures/fighters" "$pictures/vfx" "$pictures/drawn"
cp "$root"/crates/arena-game/assets/vfx/*.webp "$pictures/vfx/"
cp "$root"/crates/arena-game/assets/drawn/*.webp "$pictures/drawn/"
pairs=()
for png in "$root"/crates/arena-game/assets/fighters/*.png; do pairs+=("$png" "$pictures/fighters/$(basename "${png%.png}").webp"); done
(cd "$root" && cargo run -q --release -p arena-vfx --example webp -- "${pairs[@]}" >/dev/null)
assets="assets-$(cd "$pictures" && find . -type f | LC_ALL=C sort | xargs sum | hash)"
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
# arena.js beside the game it starts, so the page's one unhashed name is index.html, which no cache keeps
sed "s#\"../game/arena_game.js\"#\"./arena_game.js\"#" "$here/assets/arena.js" > "$staged/arena.js"
grep -q '"./arena_game.js"' "$staged/arena.js" || { echo "site/build.sh: arena.js does not import the game" >&2; exit 1; }
# the game's wasm, its JS and arena.js in a folder named by their content (the JS finds the wasm beside itself)
game="$(cat "$staged/arena_game_bg.wasm" "$staged/arena_game.js" "$staged/arena.js" | hash)"
mv "$staged" "$out/game/$game"
cp -R "$pictures" "$out/game/$assets"
printf '/*\n  Cache-Control: public, max-age=31536000, immutable\n' > "$out/game/_headers"
# the build stamp: the commit this site is built from (12 hex digits, as arena-server's ARENA_GIT_HASH) and whether
# tracked files differed from it; the page gets it as window.__arena.build and a footer line, live.json as "build"
commit="$(git -C "$root" rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
dirty=false
if [[ -n "$(git -C "$root" status --porcelain --untracked-files=no 2>/dev/null)" ]]; then dirty=true; fi
stamp="$commit"; [[ "$dirty" == true ]] && stamp="$commit+dirty"
# a page's `"@ARENA_DIRTY@" === "true"` (a script that runs unbuilt too) becomes the boolean itself
for page in "$here"/*.html; do
  sed -e "s/\"@ARENA_DIRTY@\" === \"true\"/$dirty/g" \
    -e "s/@ARENA_COMMIT@/$commit/g" -e "s/@ARENA_DIRTY@/$dirty/g" -e "s/@ARENA_STAMP@/$stamp/g" \
    "$page" > "$out/$(basename "$page")"
done
sed -i.bak -e "s#\"game/arena_game#\"game/$game/arena_game#g" -e "s#\"assets/arena.js\"#\"game/$game/arena.js\"#" \
  "$out/index.html" && rm "$out/index.html.bak"
# where the page watches the server's live matches (crates/arena-server): the relay's URL with a subscribe-only token,
# from the environment, never from the repo. Without it the page fights on its own, as before.
if [[ -n "${ARENA_LIVE_URL:-}" ]]; then
  printf '{"url":"%s","build":{"commit":"%s","dirty":%s}}\n' "$ARENA_LIVE_URL" "$commit" "$dirty" > "$out/live.json"
fi
cp "$here"/assets/arena.css "$here"/assets/menu.js "$here"/assets/site.css "$out/assets/"
# the site's icon at its root: browsers ask for /favicon.ico on every page that names none
cp "$here"/assets/favicon.ico "$here"/assets/favicon.svg "$out/"
grep -q "\"game/$game/arena.js\"" "$out/index.html" || { echo "site/build.sh: index.html does not name the game's folder" >&2; exit 1; }
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
# the wiki, generated from the code (crates/arena-wiki): a page per skill, support, ailment, keystone, aura, domain,
# curse, unit and look, its pictures drawn from the sheets
(cd "$root" && cargo run -q --release -p arena-wiki -- "$out/wiki" >/dev/null)
echo "arena site: $out (game/$game, game/$assets)"

if [[ "${1:-}" == "--serve" ]]; then
  cd "$out" && python3 -m http.server 8771 --bind 127.0.0.1
fi
