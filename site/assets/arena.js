// The arena's home page: the duel is the Bevy game (crates/arena-game) on the hero's canvas; this page only starts
// it, hands it the bot modules a player drops into a corner and the seed to fight on, and shows its status line.
// It decides nothing about the fight.
import init, { boxes, corners, house, live, restart, run, setCorner, status } from "../game/arena_game.js";

const PAGES = { home: mountHome };
try {
  await PAGES[document.body.dataset.page]?.();
} catch (err) {
  loading(`The arena did not load: ${err?.message ?? err}`, 1, true);
  throw err;
}

/** The hero's loading overlay: what is loading and how far along (0 to 1), or that it failed. */
function loading(text, progress, failed = false) {
  const loader = document.querySelector(".loader");
  if (!loader) return;
  loader.querySelector(".loader-text").textContent = text;
  loader.style.setProperty("--progress", String(progress));
  loader.classList.toggle("failed", failed);
}

/** Take the overlay away once the fight shows. */
function loaded() {
  const loader = document.querySelector(".loader");
  if (!loader || loader.classList.contains("done")) return;
  loader.style.setProperty("--progress", "1");
  loader.classList.add("done");
  loader.addEventListener("transitionend", () => loader.remove(), { once: true });
  setTimeout(() => loader.remove(), 1000);
}

/** The seed the page's address asks for (`?seed=`), if any. */
function askedSeed() {
  const seed = Number.parseInt(new URLSearchParams(location.search).get("seed") ?? "", 10);
  return Number.isSafeInteger(seed) && seed > 0 ? seed : undefined;
}

/** Where the server's live matches are published (`live.json`, written by site/build.sh), or nothing. */
async function liveRelay() {
  try {
    const res = await fetch("live.json", { cache: "no-store" });
    if (!res.ok) return undefined;
    const { url } = await res.json();
    return typeof url === "string" && url && "WebTransport" in globalThis ? url : undefined;
  } catch {
    return undefined;
  }
}

/** The hitbox overlay (the game's `boxes`): what the rules test hits against, drawn over the fight. Off unless asked
 *  for: the "Hitbox" toggle or H, remembered in localStorage; `?boxes=1` (or `0`) asks for this visit only. */
function hitboxes() {
  const toggle = document.querySelector("button[data-boxes]");
  const legend = document.querySelector(".boxes-legend");
  if (!toggle) return;
  const remembered = () => {
    try {
      return localStorage.getItem("arena.boxes") === "1";
    } catch {
      return false;
    }
  };
  const show = (on, keep) => {
    boxes(on);
    toggle.setAttribute("aria-pressed", String(on));
    if (legend) legend.hidden = !on;
    document.body.dataset.boxesShown = on ? "on" : "off";
    if (keep) {
      try {
        localStorage.setItem("arena.boxes", on ? "1" : "0");
      } catch {}
    }
  };
  const asked = new URLSearchParams(location.search).get("boxes");
  show(asked === null ? remembered() : asked === "1", false);
  const flip = () => show(toggle.getAttribute("aria-pressed") !== "true", true);
  toggle.addEventListener("click", flip);
  document.addEventListener("keydown", (e) => {
    const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement;
    if (e.key.toLowerCase() === "h" && !e.ctrlKey && !e.metaKey && !e.altKey && !typing) flip();
  });
}

/** The page: the duel full-screen, and under it a slot for each corner. A `.wasm` dropped into a slot takes that corner
 *  at once, the fight starting over on the seed in the box; an empty slot plays the house bot. The page's address
 *  carries the seed (`?seed=`), so a fight can be shared: the same two bots and seed play it the same anywhere. */
async function mountHome() {
  const form = document.getElementById("match");
  const line = document.getElementById("status");
  const seedInput = form.elements.seed;
  const inputs = [form.elements.left, form.elements.right];

  loading("Loading the arena", 0.2);
  await init();
  hitboxes();
  // the server's matches, watched live, unless the address asks for a seed (a shared fight, played here)
  const relay = askedSeed() === undefined ? await liveRelay() : undefined;
  loading(relay ? "Connecting to the arena" : "Seating the bots", 0.6);
  // the game runs on its own from here, frame by frame, on the canvas #scene
  try {
    if (relay) live(relay);
    else run(askedSeed());
  } catch (err) {
    // winit's event loop hands control back to the browser by throwing; the game keeps running
    if (!String(err?.message ?? err).includes("Using exceptions for control flow")) throw err;
  }

  const seedNow = () => {
    const seed = Number.parseInt(seedInput.value, 10);
    return Number.isSafeInteger(seed) && seed > 0 ? seed : undefined;
  };
  const shown = (i, name) => {
    const seat = inputs[i].closest(".seat");
    seat.querySelector(".seat-file").textContent = name ?? "house bot";
    seat.classList.toggle("chosen", Boolean(name));
  };

  inputs.forEach((input, i) => {
    input.addEventListener("change", async () => {
      const file = input.files[0];
      if (!file) return;
      setCorner(i, file.name, new Uint8Array(await file.arrayBuffer()));
      restart(seedNow());
      shown(i, file.name);
    });
  });
  form.addEventListener("submit", (e) => {
    e.preventDefault();
    const seed = seedNow();
    if (seed === undefined) {
      line.textContent = "The seed is a whole number from 1 up.";
      line.classList.add("error");
      return;
    }
    restart(seed);
  });
  form.querySelector("[data-random]").addEventListener("click", () => restart(undefined));
  form.querySelector("[data-house]").addEventListener("click", () => {
    house();
    restart(seedNow());
    inputs.forEach((input, i) => {
      input.value = "";
      shown(i, null);
    });
  });

  // the game's status line, and the seed it fights on in the box
  setInterval(() => {
    const text = status();
    if (text.startsWith("Seed")) {
      loaded();
      const seed = text.match(/^Seed (\d+)/)?.[1];
      if (seed && document.activeElement !== seedInput) seedInput.value = seed;
    }
    // watching a live match, a slot no bot was dropped into names who plays its look there
    const live = corners();
    inputs.forEach((input, i) => {
      const seat = input.closest(".seat");
      if (seat.classList.contains("chosen")) return;
      const name = live[i] ?? "house bot";
      const file = seat.querySelector(".seat-file");
      if (file.textContent !== name) file.textContent = name;
    });
    line.classList.toggle("error", text.startsWith("Not a bot"));
    if (line.textContent !== text) line.textContent = text;
  }, 250);
}
