#!/usr/bin/env bash
# Build the arena site into site/dist: its pages, the player docs rendered from docs/*.md, the duel's page kit
# (site/embed.sh: page modules, the arena's wasm, the trained policies) and the pixfx engine in dist/pkg (the
# pixfx-canvas2d bundle, its painting worker and the pixfx-web wasm), built from the engine at the pinned rev
# (site/pixfx.sh puts it at .pixfx/), whose site.css and preload.js the site uses too.
# Usage: site/build.sh            (from anywhere)
#        site/build.sh --serve    (build, then serve dist on http://127.0.0.1:8771/)
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/.." && pwd)"
pixfx="$("$here/pixfx.sh")"
out="$here/dist"

(cd "$pixfx/packages/pixfx-web" && bun install && bun run build:wasm)
(cd "$pixfx/packages/pixfx-canvas2d" && bun install && bun run build)

rm -rf "$out"
mkdir -p "$out/assets"
cp "$here"/*.html "$out/"
cp "$here"/assets/arena.js "$here"/assets/arena.css "$here"/assets/menu.js "$pixfx/site/assets/site.css" "$out/assets/"
"$here/embed.sh" "$out/assets"
cp -R "$pixfx/packages/pixfx-canvas2d/dist" "$out/pkg"
bun "$here/docs.js" "$root/docs" "$out/docs"
bun "$pixfx/site/preload.js" "$out"
echo "arena site: $out"

if [[ "${1:-}" == "--serve" ]]; then
  cd "$out" && python3 -m http.server 8771 --bind 127.0.0.1
fi
