// The two wasm modules the duel's page loads, for the tests: the pixfx engine's (pixfx-web's, at the pinned rev in
// .pixfx/) and the arena's rules (site/assets/wasm). `site/build.sh` builds both.
import { readFileSync } from "node:fs";
import { useBots, useRules } from "../assets/duel.js";

const engineDir = new URL("../../.pixfx/packages/pixfx-web/wasm/", import.meta.url);
const pkg = await import(new URL("pixfx_wasm.js", engineDir).href);
pkg.initSync({ module: readFileSync(new URL("pixfx_wasm_bg.wasm", engineDir)) });

const rulesDir = new URL("../assets/wasm/", import.meta.url);
const rules = await import(new URL("arena_wasm.js", rulesDir).href);
rules.initSync({ module: readFileSync(new URL("arena_wasm_bg.wasm", rulesDir)) });

useRules(rules);
// the house bot module in both corners, as the pages seat it once its file arrives (site/embed.sh builds it from
// crates/pixfx-duel/weights/policy.onnx)
export const house = new Uint8Array(readFileSync(new URL("../assets/bots/policy.wasm", import.meta.url)));
await useBots([house, house]);
export { pkg, rules };
export const { Body, Fx, Mind, effects } = pkg;
