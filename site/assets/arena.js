// The arena's home page loads this script (docs pages load only `menu.js`): the duel full-screen, and a slot for each
// corner where a player drops in their bot. The duel itself
// is `duel-hero.js`, which the pixfx site embeds too; this file decides nothing about it.
import { loadPixfx } from "../pkg/pixfx-canvas2d.js";
import loadRules, * as rules from "./wasm/arena_wasm.js";
import { useBots, useRules } from "./duel.js";
import { createHero, loadBots, loadIcons } from "./duel-hero.js";

/** The bot both corners play when the page is not told otherwise: the trained policy shipped with the rules, as the
 *  bot module built from its ONNX model (`bots/onnx`), run the way a player's module is. */
const HOUSE = "policy.wasm";

const PAGES = { home: mountHome };
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

/** The page: the duel full-screen, and under it a slot for each corner. A `.wasm` dropped into a slot takes that corner
 *  at once, the fight starting over on the seed in the box; an empty slot plays the house bot. The page's address
 *  carries the seed (`?seed=`), so a fight can be shared: the same two bots and seed play it the same anywhere. */
async function mountHome() {
  const form = document.getElementById("match");
  const status = document.getElementById("status");
  const seedInput = form.elements.seed;
  const asked = Number.parseInt(new URLSearchParams(location.search).get("seed") ?? "", 10);
  const { engine, icons, house } = await start();
  await useBots([house, house]);
  const hero = createHero(document.getElementById("scene"), new AbortController().signal, icons, engine, {
    seed: Number.isSafeInteger(asked) && asked > 0 ? asked : undefined,
    replay: Number.isSafeInteger(asked) && asked > 0,
  });
  seedInput.value = String(hero.seed());
  // what each corner seats now: its bytes and the name shown for it
  const corners = [
    { bytes: house, name: "house bot" },
    { bytes: house, name: "house bot" },
  ];
  const inputs = [form.elements.left, form.elements.right];
  const say = (text, error = false) => {
    status.textContent = text;
    status.classList.toggle("error", error);
  };
  const shown = (i) => {
    const seat = inputs[i].closest(".seat");
    seat.querySelector(".seat-file").textContent = corners[i].name;
    seat.classList.toggle("chosen", corners[i].bytes !== house);
  };
  /** Seat `next` (what each corner should hold; `corners` until then) and start the fight over on `seed`. A bot the
   *  arena refuses throws before anything changes. */
  const fight = async (seed, next = corners) => {
    await useBots(next.map((c) => c.bytes));
    next.forEach((c, i) => (corners[i] = c));
    hero.restart(seed);
    seedInput.value = String(seed);
    history.replaceState(null, "", `?seed=${seed}`);
    say("");
  };
  const seedNow = () => {
    const seed = Number.parseInt(seedInput.value, 10);
    return Number.isSafeInteger(seed) && seed > 0 ? seed : hero.seed();
  };

  inputs.forEach((input, i) => {
    input.addEventListener("change", async () => {
      const file = input.files[0];
      if (!file) return;
      const next = [...corners];
      next[i] = { bytes: new Uint8Array(await file.arrayBuffer()), name: file.name };
      try {
        await fight(seedNow(), next);
        shown(i);
      } catch (err) {
        input.value = "";
        say(`${file.name} is not a bot for this season: ${err?.message ?? err}`, true);
      }
    });
  });
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    const seed = Number.parseInt(seedInput.value, 10);
    if (!Number.isSafeInteger(seed) || seed < 1) return say("The seed is a whole number from 1 up.", true);
    await fight(seed);
  });
  form.querySelector("[data-random]").addEventListener("click", async () => {
    await fight(1 + Math.floor(Math.random() * 1e6));
  });
  form.querySelector("[data-house]").addEventListener("click", async () => {
    await fight(seedNow(), corners.map(() => ({ bytes: house, name: "house bot" })));
    inputs.forEach((input, i) => {
      input.value = "";
      shown(i);
    });
  });

  run(hero, () => {
    const d = hero.duel;
    if (!d || status.classList.contains("error")) return;
    const text = `Seed ${hero.seed()} · round ${d.round} · Thunder (${corners[0].name}) ${d.wins[0]} – ${d.wins[1]} Frost (${corners[1].name})`;
    if (status.textContent !== text) status.textContent = text;
  });
}
