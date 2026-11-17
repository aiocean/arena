// The native = wasm check (a host for the WebAssembly the page runs; node is the closest to a browser that CI has):
// `examples/wasm_hashes.rs`, built for wasm32-unknown-unknown and bound by wasm-bindgen into <dir>, works out seeds
// 1-20's fingerprints of 120 s, which must equal the native goldens (`../goldens/event_hashes.json`).
// Usage: node crates/arena-duel/tests/wasm/native_wasm.mjs <dir with wasm_hashes.js>
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node native_wasm.mjs <dir with wasm_hashes.js>");
  process.exit(2);
}
const { event_hashes } = createRequire(import.meta.url)(resolve(dir, "wasm_hashes.js"));
const want = readFileSync(new URL("../goldens/event_hashes.json", import.meta.url), "utf8");
const got = event_hashes(20, 120);
if (got !== want) {
  const w = want.split("\n"), g = got.split("\n");
  for (let i = 0; i < Math.max(w.length, g.length); i++) {
    if (w[i] !== g[i]) console.error(`native ${w[i] ?? ""}\nwasm   ${g[i] ?? ""}`);
  }
  console.error("FAIL: the WebAssembly build fights differently from the native goldens");
  process.exit(1);
}
console.log("PASS: seeds 1-20, 120 s each, hash for hash the same in WebAssembly as natively");
