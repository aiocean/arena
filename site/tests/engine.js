// The two wasm modules the duel's page loads, for the tests: the pixfx engine's (pixfx-web's, built in the pixfx
// checkout: `cd ../../tools/pixfx-rs/packages/pixfx-web && bun run build:wasm`) and the arena's rules
// (`site/embed.sh` builds it into site/assets/wasm).
import { readFileSync } from "node:fs";
import { usePolicies, useRules } from "../assets/duel.js";

const engineDir = new URL("../../../../tools/pixfx-rs/packages/pixfx-web/wasm/", import.meta.url);
const pkg = await import(new URL("pixfx_wasm.js", engineDir).href);
pkg.initSync({ module: readFileSync(new URL("pixfx_wasm_bg.wasm", engineDir)) });

const rulesDir = new URL("../assets/wasm/", import.meta.url);
const rules = await import(new URL("arena_wasm.js", rulesDir).href);
rules.initSync({ module: readFileSync(new URL("arena_wasm_bg.wasm", rulesDir)) });

useRules(rules);
// the trained policy in both corners, as the pages seat it once its file arrives
export const policyBytes = readFileSync(new URL("../../crates/pixfx-duel/weights/policy.bin", import.meta.url));
usePolicies([policyBytes, policyBytes]);
export { pkg, rules };
export const { Body, Fx, Mind, effects } = pkg;
