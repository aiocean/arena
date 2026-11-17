// The arena's duel pages load this script (docs pages load only `menu.js`). It mounts the page's own part, picked by
// the body's data-page: the duel full-screen on the home page, two bots chosen from files on the watch page. The duel itself
// is `duel-hero.js`, which the pixfx site embeds too; this file decides nothing about it.
import { loadPixfx } from "../pkg/pixfx-canvas2d.js";
import loadRules, * as rules from "./wasm/arena_wasm.js";
import { useBots, useRules } from "./duel.js";
import { createHero, loadBots, loadIcons } from "./duel-hero.js";

/** The bot both corners play when the page is not told otherwise: the trained policy shipped with the rules, as the
 *  bot module built from its ONNX model (`bots/onnx`), run the way a player's module is. */
const HOUSE = "policy.wasm";

const PAGES = { home: mountHome, watch: mountWatch };
try {
  await PAGES[document.body.dataset.page]?.();
} catch (err) {
  loading(`The arena did not load: ${err?.message ?? err}`, 1, true);
  throw err;
}

/** The engine, the rules and the skill icons, then the house bot. */
async function start() {
  loading("Loading the engine", 0.2);
  const [engine, icons] = await Promise.all([loadPixfx(), loadIcons([32, 24]), loadRules()]);
  useRules(rules);
  loading("Seating the bots", 0.55);
  const [house] = await loadBots([HOUSE]);
  loading("Drawing the fighters", 0.8);
  return { engine, icons, house };
}

/** The hero's loading overlay: what is loading and how far along (0 to 1), or that it failed. */
function loading(text, progress, failed = false) {
  const loader = document.querySelector(".loader");
  if (!loader) return;
  loader.querySelector(".loader-text").textContent = text;
  loader.style.setProperty("--progress", String(progress));
  loader.classList.toggle("failed", failed);
}

/** Take the overlay away once the duel shows its fighters. */
function loaded() {
  const loader = document.querySelector(".loader");
  if (!loader || loader.classList.contains("done")) return;
  loader.style.setProperty("--progress", "1");
  loader.classList.add("done");
  loader.addEventListener("transitionend", () => loader.remove(), { once: true });
  setTimeout(() => loader.remove(), 1000);
}

/** Run `hero` each animation frame while its canvas is on screen and the tab is visible. */
function run(hero, onFrame) {
  const began = performance.now();
  let last = null;
  let visible = true;
  new IntersectionObserver(([e]) => {
    visible = e.isIntersecting;
  }).observe(hero.canvas);
  function frame(now) {
    const dt = last === null ? 0 : Math.max(0, now - last) / 1000;
    last = now;
    if (visible && !document.hidden) {
      hero.step(dt);
      // the fighters' sheets arrive last; a page whose sheets never come shows the stage after a while all the same
      if (hero.fighters || now - began > 12000) loaded();
      onFrame?.();
    }
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

async function mountHome() {
  const { engine, icons, house } = await start();
  await useBots([house, house]);
  const hero = createHero(document.getElementById("scene"), new AbortController().signal, icons, engine);
  run(hero);
}

/** The watch page: a bot module (`.wasm`) for each corner (the house bot where none is chosen), and a seed. The same two files and the same seed play the same fight on every machine. */
async function mountWatch() {
  const form = document.getElementById("match");
  const status = document.getElementById("status");
  const seedInput = form.elements.seed;
  const params = new URLSearchParams(location.search);
  const asked = Number.parseInt(params.get("seed") ?? "", 10);
  const { engine, icons, house } = await start();
  await useBots([house, house]);
  const hero = createHero(document.getElementById("scene"), new AbortController().signal, icons, engine, {
    seed: Number.isSafeInteger(asked) && asked > 0 ? asked : undefined,
    replay: true,
  });
  seedInput.value = String(hero.seed());
  const names = ["house bot", "house bot"];
  const say = (text, error = false) => {
    status.textContent = text;
    status.classList.toggle("error", error);
  };

  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const seed = Number.parseInt(seedInput.value, 10);
    if (!Number.isSafeInteger(seed) || seed < 1) return say("The seed is a whole number from 1 up.", true);
    try {
      const files = [form.elements.left.files[0], form.elements.right.files[0]];
      const bytes = await Promise.all(files.map(async (f) => (f ? new Uint8Array(await f.arrayBuffer()) : house)));
      await useBots(bytes);
      files.forEach((f, i) => (names[i] = f ? f.name : "house bot"));
    } catch (err) {
      return say(`That file is not a bot for this season: ${err?.message ?? err}`, true);
    }
    hero.restart(seed);
    history.replaceState(null, "", `?seed=${seed}`);
    // the new fight plays up in the hero: bring it into view
    hero.canvas.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth", block: "start" });
    say("");
  });
  for (const seat of form.querySelectorAll(".seat")) {
    const input = seat.querySelector("input[type=file]");
    const shown = seat.querySelector(".seat-file");
    input.addEventListener("change", () => {
      const file = input.files[0];
      shown.textContent = file ? file.name : shown.dataset.empty;
      seat.classList.toggle("chosen", Boolean(file));
    });
  }
  form.querySelector("[data-random]").addEventListener("click", () => {
    seedInput.value = String(1 + Math.floor(Math.random() * 1e6));
  });

  run(hero, () => {
    const d = hero.duel;
    if (!d || status.classList.contains("error")) return;
    const text = `Seed ${hero.seed()} · round ${d.round} · Thunder (${names[0]}) ${d.wins[0]} – ${d.wins[1]} Frost (${names[1]})`;
    if (status.textContent !== text) status.textContent = text;
  });
}

