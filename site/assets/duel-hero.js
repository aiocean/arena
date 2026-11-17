// The live duel both sites play (the pixfx landing page's hero and the arena's pages): the rules through the engine's
// wasm (`duel.js`), drawn by pixfx-canvas2d (workers paint the effects), with the fighters, the scene's light, the HUD
// and its tooltips. The page owns the animation-frame loop and calls `step(dt)` each frame.
import { FightFeel, Lighting, Sprites, createPixfx, lookColors, putOutLight, stepLights } from "../pkg/pixfx-canvas2d.js";
import { DECOR, RAGE, UNIT, createDuel, createFighters, createVfx, freeDuel, step } from "./duel.js";
import { BARS_TALL, actorOf, drawBars, drawFine, drawNumbers, drawSkills, drawStreaks, hudAt, paintFighters } from "./duel-art.js";
import { buffInfo, skillInfo } from "./duel-info.js";
import { createScene, drawScene } from "./duel-scene.js";
import { kindle, lampGlows } from "./duel-light.js";

/** Behind the hero's scene while the screen shakes. */
const BACKDROP = "#0b0a1c";
/** How tall the film bars an ultimate brings in are, in game pixels, at the sky's darkest. */
const LETTERBOX = 12;
/** The effects that wrap a fighter's own sprite: a surge up its body when its rage fills, and a ring in its school's
 *  glow round every pose while its ultimate is ready or it meditates. */
const WRAP = { ring: "feedback/outline", surge: "feedback/power-up" };
/** Where a fighter's ring plays, above its feet: about the middle of its body, so the ring's box holds it in any pose. */
const RING_MID = 24;
const reduceMotion = matchMedia("(prefers-reduced-motion: reduce)").matches;

/** The fighters' sprite sheets beside this module (`site/embed.sh` copies `site/assets/fighters/`): for each school,
 *  the fighter drawn in its seat, its layout's JSON and its pages as RGBA bytes one after another
 *  (`<fighter>-<school>.png`, then `-1.png` and on, as many as the layout has pages). Fetched once, for every hero.
 *  Thunder is Vương Lâm (the cultivator), frost the Seven-Colored Daoist. */
const FIGHTERS = { thunder: "cultivator", frost: "seven-colored-daoist" };

/** A sheet's pages in `dir/` beside this module (`<base>.png`, then `<base>-1.png` and on, as many as its layout's
 *  JSON has pages) as RGBA bytes one after another. */
async function readPages(dir, layout, base) {
  const { pages = [], size } = JSON.parse(layout);
  const count = Math.max(1, pages.length || (size ? 1 : 0));
  const page = async (file) => {
    const image = new Image();
    image.src = new URL(`${dir}/${file}`, import.meta.url).href;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    ctx.drawImage(image, 0, 0);
    return ctx.getImageData(0, 0, canvas.width, canvas.height).data;
  };
  const parts = await Promise.all(Array.from({ length: count }, (_, i) => page(i === 0 ? `${base}.png` : `${base}-${i}.png`)));
  const rgba = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    rgba.set(p, at);
    at += p.length;
  }
  return rgba;
}

/** A text file in `dir/` beside this module, or a clear error. */
const text = (dir, file) =>
  fetch(new URL(`${dir}/${file}`, import.meta.url)).then((r) => (r.ok ? r.text() : Promise.reject(new Error(`${file}: HTTP ${r.status}`))));

let fighterSheets = null;
function loadFighterSheets() {
  fighterSheets ??= (async () => {
    const sheets = await Promise.all(Object.entries(FIGHTERS).map(async ([school, fighter]) => {
      const layout = await text("fighters", `${fighter}.json`);
      return [school, { layout, rgba: await readPages("fighters", layout, `${fighter}-${school}`) }];
    }));
    return { schools: Object.fromEntries(sheets) };
  })();
  return fighterSheets;
}

/** The spell effects drawn from sprite sheets (`site/assets/vfx/`, `art/vfx/tools/vfx.rs`): `index.json` names each
 *  effect the rules play (`"projectiles/laser-bolt"`) that has a sheet, `<file>.json` and its pages beside it. An
 *  effect without one plays as the engine's. None when there is no index. Fetched once, for every hero. */
let vfxSheets = null;
function loadVfxSheets() {
  vfxSheets ??= (async () => {
    const index = await fetch(new URL("vfx/index.json", import.meta.url)).then((r) => (r.ok ? r.json() : []));
    return Promise.all(index.map(async ({ fx, file }) => {
      const layout = await text("vfx", `${file}.json`);
      return { fx, layout, rgba: await readPages("vfx", layout, file) };
    }));
  })();
  return vfxSheets;
}

/** Fetch each named bot module beside this module as bytes, `[left, right]`, from `bots/` (`site/embed.sh` puts the
 *  house bot there, `policy.wasm`). A file named twice is fetched once. */
export async function loadBots(files) {
  const names = [...new Set(files)];
  const buffers = await Promise.all(
    names.map((name) =>
      fetch(new URL(`bots/${name}`, import.meta.url)).then((r) =>
        r.ok ? r.arrayBuffer() : Promise.reject(new Error(`${name}: HTTP ${r.status}`)),
      ),
    ),
  );
  const bytes = new Map(names.map((name, i) => [name, new Uint8Array(buffers[i])]));
  return files.map((name) => bytes.get(name));
}

// ---------- hero ----------

/**
 * The live duel on `canvas`: the arena sized to the canvas, the rules from `duel.js`, the fighters, effects, light, HUD
 * and tooltips. `seed` is the first duel's (a random one when left out); each later rebuild, on a resize, takes the
 * next, or the same one again with `replay` (a page that shows one seed's fight). Returns the view: `step(dt)` draws a frame, `restart(seed)` begins a fresh duel on that seed (with the
 * bots `useBots` seated last), `seed()` is the seed of the duel now playing, `free()` lets it all go.
 */
export function createHero(canvas, signal, icons, engine, { seed: first, replay = false } = {}) {
  const view = { canvas, icons, stage: null, duel: null, feel: null, sprites: null, fighters: null, vfx: null, drawn: new Map(), lighting: null, lights: [], layout: null, handles: new Map(), wrapped: [] };
  const have = new Set(engine.effects());
  // a picture Rust painted (fighters, faces, light), drawn and let go at once: the fighters and faces in their own
  // finer pixels, the rest in the world's
  const show = (picture) => {
    if (picture?.unit > 1) drawFine(canvas.getContext("2d"), view.layout.scale, picture);
    else view.stage.drawPicture(picture);
    picture?.free();
  };
  let seed = first ?? 1 + Math.floor(Math.random() * 1e6);
  // how many times the view was built: sheets that arrive for an older build are not used
  let builds = 0;
  let playing = seed;

  function build() {
    view.free();
    view.handles.clear();
    view.wrapped = [];
    view.lights = [];
    const narrow = innerWidth < 760;
    // the fighters stay big enough to read their poses: bigger pixels on a wider screen. The canvas is in the
    // screen's own pixels, and a fighter's pixel (a `UNIT`th of the world's) is a whole number of them.
    const dpr = Math.min(2, Math.max(1, Math.round(devicePixelRatio || 1)));
    let scale = (narrow ? 2 : innerWidth < 1200 ? 3 : 4) * dpr;
    scale = Math.max(UNIT, scale - (scale % UNIT));
    const rect = canvas.getBoundingClientRect();
    const w = Math.ceil((rect.width * dpr) / scale);
    const h = Math.ceil((rect.height * dpr) / scale);
    canvas.width = w * scale;
    canvas.height = h * scale;
    const ground = h - (narrow ? 14 : 22);
    // the arena is the middle of the dais, at most 420 pixels wide, a brazier at each end
    const lampInset = 14;
    const span = Math.min(w - 16, 420);
    const arena = { left: Math.round((w - span) / 2), right: Math.round((w + span) / 2) };

    view.stage = createPixfx(canvas.getContext("2d"), { scale });
    // the fight's own FightFeel answers every blow; the stage's effects just play the seconds it gives them
    view.stage.feel.setEnabled(false);
    // the night over the whole scene: one fireflies canvas its size, the meadow on the dais (the effect's ground
    // is mid-width, 10 pixels above its canvas's foot)
    if (have.has(DECOR.night)) view.stage.play(DECOR.night, Math.floor(w / 2), ground, { width: w, height: ground + 10 });
    // a brazier at each end of the arena lights it
    if (have.has(DECOR.lamp)) for (const x of [arena.left, arena.right]) view.stage.play(DECOR.lamp, x, ground, { seed: x });
    view.feel = new FightFeel();
    view.sprites = new Sprites(w, h);
    // the fighters are drawn from the arena's sprite sheets, once they have arrived (the first frames show the stage)
    const built = ++builds;
    loadFighterSheets().then((sheets) => {
      if (built === builds && !view.fighters) view.fighters = createFighters(w, h, sheets);
    }).catch((e) => console.error("duel: the fighters' sheets did not load:", e));
    // and the spell effects that have sprite sheets, once theirs have; until then (and for the rest) the engine's
    view.drawn.clear();
    loadVfxSheets().then((sheets) => {
      if (built === builds && !view.vfx && sheets.length > 0) view.vfx = createVfx(w, h, sheets);
    }).catch((e) => console.error("duel: the effects' sheets did not load:", e));
    view.lighting = new Lighting(w, h);
    // the bars go just under the page's top bar when it lies over the hero (positioned absolute or fixed, as on the
    // pixfx site); a top bar in the page's flow (the arena's own pages, where it sits under the hero) leaves the bars
    // at the top, wherever the page is scrolled (its links can wrap past its own box on a narrow screen, so measure
    // what is in it)
    const overlays = [...document.querySelectorAll(".topbar")].filter((bar) => ["absolute", "fixed"].includes(getComputedStyle(bar).position));
    // (both measured from the viewport, so the bar's bottom less the canvas's top holds however far the page is scrolled)
    const bottoms = overlays.flatMap((bar) => [bar, ...bar.querySelectorAll("*")]).map((el) => el.getBoundingClientRect().bottom);
    const barTop = bottoms.length ? Math.max(6, Math.ceil(((Math.max(...bottoms) - rect.top) * dpr) / scale) + 6) : 6;
    // fighters fly no higher than keeps their heads (about 62 pixels tall) under the bars
    const sky = ground - (barTop + BARS_TALL + 13) - 64;
    if (view.duel) freeDuel(view.duel);
    playing = seed;
    view.duel = createDuel({ seed: replay ? seed : seed++, left: arena.left + lampInset, right: arena.right - lampInset, ground, effects: [...have], sky });
    // each fighter's sprite as it was last painted, which the effects wrapped on it read
    view.wrapped = view.duel.fighters.map((f) => {
      return { body: null, rage: f.rage, surge: null, ring: null };
    });
    // each fighter's skills, on the floor under its own side: bigger icons where the floor has the room
    const icon = h - ground >= 20 ? 16 : 12;
    view.layout = { scale, dpr, w, h, ground, barTop, icon, lamps: [arena.left, arena.right], scene: createScene(w, h, ground, barTop + BARS_TALL) };
    // a still frame shows the fight under way
    if (reduceMotion) {
      for (let i = 0; i < 6 * 30; i++) play(step(view.duel, 1 / 30));
      view.step(0);
    }
  }

  /** The duel's effect commands, on the stage. */
  function play(commands) {
    for (const c of commands) {
      // an effect with a sprite sheet for its caster's school is drawn from it, the rest by the engine
      const school = c.by == null ? "" : view.duel.fighters[c.by].school;
      if (c.type === "play" && view.vfx?.has(c.fx, school)) {
        view.drawn.set(c.key, { fx: c.fx, school, x: c.x, y: c.y, t: 0, flip: !!c.opts?.flip });
      } else if (c.type === "play") view.handles.set(c.key, view.stage.play(c.fx, c.x, c.y, c.opts));
      else if (c.type === "move" && view.drawn.has(c.key)) Object.assign(view.drawn.get(c.key), { x: c.x, y: c.y });
      else if (c.type === "stop" && view.drawn.has(c.key)) view.drawn.delete(c.key);
      else if (c.type === "move" && view.handles.has(c.key)) view.handles.get(c.key).setPosition(c.x, c.y);
      else if (c.type === "stop" && view.handles.has(c.key)) {
        view.handles.get(c.key).stop();
        view.handles.delete(c.key);
      }
    }
  }

  /** Hand each fighter's new pose to the effects wrapped on it and keep them on it. When its rage fills: a surge. A ring
   *  while its rage is full or it meditates, until neither holds or the fighter is down. */
  function wrapFighters(pictures) {
    const { ground } = view.layout;
    for (const f of view.duel.fighters) {
      const one = view.wrapped[f.index];
      if (!one) continue;
      one.body = actorOf(pictures[f.index]?.body);
      const feet = ground - f.air;
      const ready = f.rage >= RAGE && f.hp > 0;
      if (ready && one.rage < RAGE && have.has(WRAP.surge)) {
        one.surge?.stop();
        const surge = view.stage.play(WRAP.surge, f.x, feet);
        surge.wrap(() => one.body).once("complete", () => {
          if (one.surge === surge) one.surge = null;
        });
        one.surge = surge;
      }
      const ringed = (ready || f.pose === "meditate") && f.hp > 0;
      if (ringed && !one.ring && have.has(WRAP.ring)) {
        one.ring = view.stage.play(WRAP.ring, f.x, feet - RING_MID, { palette: [lookColors(f.school).glow] });
        one.ring.wrap(() => one.body);
      } else if (!ringed && one.ring) {
        one.ring.stop();
        one.ring = null;
      }
      one.ring?.setPosition(f.x, feet - RING_MID);
      one.surge?.setPosition(f.x, feet);
      one.rage = f.rage;
    }
  }

  view.step = (dt) => {
    const { scale, w, h, ground, scene } = view.layout;
    // a blow stops the fight and its effects together for a few frames; a knockout plays out slowed
    const run = view.feel.advance(dt);
    const commands = step(view.duel, run);
    play(commands);
    view.feel.react(commands);
    view.lights = stepLights(kindle(view.lights, commands, ground, (fx) => engine.meta(fx).loop, putOutLight), run);
    // the fighters are painted before the effects play, so an effect wrapped on one grips this frame's pose
    const painted = paintFighters(view.sprites, view.fighters, view.duel, view.feel, ground, view.feel.jolts());
    wrapFighters(painted.fighters);
    view.stage.update(reduceMotion ? 0 : run);
    const ctx = canvas.getContext("2d");
    ctx.fillStyle = BACKDROP;
    ctx.fillRect(0, 0, w * scale, h * scale);
    ctx.save();
    const [sx, sy] = view.feel.shake(1);
    ctx.translate(sx * scale, sy * scale);
    drawScene(ctx, scene, view.duel.time, scale);
    view.stage.drawLayer("back");
    // an ultimate darkens the world behind the fighters and the magic
    const dark = view.feel.dusk();
    if (dark > 0) {
      ctx.fillStyle = `rgba(4, 2, 10, ${dark})`;
      ctx.fillRect(-scale * 4, -scale * 4, (w + 8) * scale, (h + 8) * scale);
    }
    // the one a blow struck shudders through the hit-stop
    for (const p of painted.fighters) {
      if (!p) continue;
      show(p.sprite);
      show(p.marks);
      p.body?.free();
    }
    show(painted.dust);
    // the spell effects drawn from sprite sheets, over the fighters; a one-shot one that has played out goes
    if (view.vfx && view.drawn.size > 0) {
      view.vfx.begin();
      for (const [key, e] of view.drawn) {
        e.t += reduceMotion ? 0 : run;
        if (!view.vfx.draw(e.fx, e.school, e.t, e.x, e.y, e.flip)) view.drawn.delete(key);
      }
      show(view.vfx.picture());
    }
    // the magic lights the dais, the peaks and the fighters in its own colour
    show(view.lighting.paint(view.lights, lampGlows(view.layout.lamps, ground, view.duel.time)));
    view.stage.drawLayer("front");
    drawStreaks(ctx, view.feel.streaks(), view.duel, scale);
    drawNumbers(ctx, view.feel.numbers(), view.duel, scale);
    ctx.restore();
    // an ultimate frames the screen like a film for as long as the sky is dark
    if (dark > 0) {
      const band = Math.round((dark / FightFeel.duskDepth()) * LETTERBOX);
      ctx.fillStyle = "#000";
      ctx.fillRect(0, 0, w * scale, band * scale);
      ctx.fillRect(0, (h - band) * scale, w * scale, band * scale);
    }
    drawBars(ctx, view.fighters, show, view.duel, scale, view.layout.barTop);
    // each fighter's skills, from the atlas whose pixels land on whole screen pixels
    const { icon } = view.layout;
    drawSkills(ctx, view.duel, scale, h, icon, (icon * scale) % 32 === 0 ? view.icons[32] : view.icons[24]);
    // what the pointer holds changes with the fight: its tooltip follows
    view.tip.update();
  };

  let pending = null;
  addEventListener("resize", () => {
    clearTimeout(pending);
    pending = setTimeout(build, 150);
  }, { signal });
  signal.addEventListener("abort", () => clearTimeout(pending));
  view.free = () => {
    view.stage?.destroy();
    view.sprites?.free();
    view.fighters?.free();
    view.fighters = null;
    view.vfx?.free();
    view.vfx = null;
    view.drawn.clear();
    view.feel?.free();
    view.lighting?.free();
    if (view.duel) freeDuel(view.duel);
    view.duel = null;
  };
  view.playing = () => view.stage?.stats().engines ?? 0;
  view.seed = () => playing;
  view.restart = (s) => {
    seed = s;
    build();
  };

  build();
  view.tip = createTip(view, signal);
  return view;
}

// ---------- the hero's tooltips ----------

/** A tooltip's nodes, laid out as in Path of Exile: a header with the name and the type line, then the properties,
 *  what it does and the mods, each part set off by a rule. */
function tipNodes(info) {
  const make = (tag, className, text) => Object.assign(document.createElement(tag), { className, textContent: text ?? "" });
  const head = make("header", "tip-head");
  head.append(make("h3", "", info.title));
  if (info.type) head.append(make("p", "tip-type", info.type));
  const parts = [];
  if (info.props.length > 0) {
    parts.push(info.props.map(([label, value]) => {
      const line = make("p", "tip-prop", `${label}: `);
      line.append(make("span", "", value));
      return line;
    }));
  }
  parts.push([make("p", "tip-text", info.text)]);
  if (info.mods.length > 0) parts.push(info.mods.map((mod) => make("p", "tip-mod", mod)));
  const body = make("div", "tip-body");
  parts.forEach((nodes, i) => body.append(...(i > 0 ? [make("hr", "")] : []), ...nodes));
  return [head, body];
}

/**
 * Hover a skill slot or a buff tile (or tap it) to see what it is: a panel by the pointer, kept up to date while the
 * pointer stays. A tap pins it; a tap anywhere else, or a scroll, lets it go.
 */
function createTip(view, signal) {
  const { canvas } = view;
  const el = document.createElement("div");
  el.className = "duel-tip";
  el.hidden = true;
  el.setAttribute("role", "tooltip");
  document.body.append(el);
  signal.addEventListener("abort", () => el.remove());
  // where the pointer is, in the page, and whether a tap pinned it there
  let at = null;
  let shown = "";
  const target = () => {
    if (!at || !view.duel || !view.layout) return null;
    const r = canvas.getBoundingClientRect();
    const { scale, barTop, h, icon } = view.layout;
    const gx = (((at.cx - r.left) / r.width) * canvas.width) / scale;
    const gy = (((at.cy - r.top) / r.height) * canvas.height) / scale;
    return hudAt(view.duel, gx, gy, barTop, h, icon);
  };
  const update = () => {
    const hit = target();
    canvas.style.cursor = hit ? "pointer" : "";
    const f = hit && view.duel.fighters[hit.fighter];
    const info = !hit ? null : hit.kind === "skill" ? skillInfo(f, hit.slot) : buffInfo(hit.buff);
    if (!info) {
      el.hidden = true;
      shown = "";
      return;
    }
    const key = JSON.stringify(info);
    if (key !== shown) {
      shown = key;
      el.replaceChildren(...tipNodes(info));
      el.dataset.tone = info.tone ?? "plain";
    }
    // drawn in the duel's own pixels: one of its pixels, in page pixels
    const r = canvas.getBoundingClientRect();
    el.style.setProperty("--px", `${Math.max(2, Math.round((view.layout.scale * r.width) / canvas.width))}px`);
    el.hidden = false;
    // beside the pointer, kept on the screen
    const pad = 14;
    const x = Math.max(4, Math.min(at.cx + pad, innerWidth - el.offsetWidth - 4));
    const y = at.cy + pad + el.offsetHeight > innerHeight - 4 ? at.cy - el.offsetHeight - pad : at.cy + pad;
    el.style.left = `${x}px`;
    el.style.top = `${Math.max(4, y)}px`;
  };
  canvas.addEventListener("pointermove", (e) => {
    if (e.pointerType !== "mouse" || at?.pinned) return;
    at = { cx: e.clientX, cy: e.clientY, pinned: false };
    update();
  }, { signal });
  canvas.addEventListener("pointerleave", () => {
    if (at?.pinned) return;
    at = null;
    update();
  }, { signal });
  canvas.addEventListener("click", (e) => {
    at = { cx: e.clientX, cy: e.clientY, pinned: true };
    if (!target()) at = null;
    update();
  }, { signal });
  document.addEventListener("click", (e) => {
    if (e.target === canvas || !at?.pinned) return;
    at = null;
    update();
  }, { signal });
  addEventListener("scroll", () => {
    if (!at) return;
    at = null;
    update();
  }, { signal, passive: true });
  return { update };
}

/** Each skill icon atlas, by the size of its icons: `{ [size]: { image, size } }`. */
export async function loadIcons(sizes) {
  const atlases = await Promise.all(sizes.map(async (size) => {
    const image = new Image();
    image.src = new URL(`skills-${size}.png`, import.meta.url).href;
    await image.decode();
    return [size, { image, size }];
  }));
  return Object.fromEntries(atlases);
}

