// The arena duel, whose rules are crates/pixfx-duel, run through the arena's wasm (crates/arena-wasm, built into
// site/wasm). This file only hands the page what the rules decided: a duel whose `fighters`, `time`, `round`, `wins`,
// `winner`, `left`, `right` and `ground` are refreshed from the rules' view after each `step`, and the rules' tables,
// read from the arena's wasm once it has loaded (`useRules`), and the fighters' painter that draws them from the
// arena's sprite sheets (`createFighters`). It decides nothing. The pixfx engine's wasm (effects, bodies, light) is a
// separate module the page loads itself.
//
// Two sizes of pixel: the rules' are `UNIT` times finer than the page's world, which is the engine's (its effects,
// light and feel, the scene and the HUD). This file is the one place they meet: a duel is made from the page's
// numbers and hands back its view and commands in them; only the fighters are drawn in the rules' own pixels
// (`createFighters`).

/** The arena wasm's bindings, set by `useRules`. */
let engine = null;
/** What each corner seats, `[left, right]` `Corner`s (a bot module's instance each), set by `useBots` once the files
 *  have arrived; until then duels seat the brain. */
let corners = null;

/** The arena's wasm, or a clear error when the page forgot `useRules`. */
function loaded() {
  if (!engine) throw new Error("duel.js: call useRules(arena) once the arena wasm has loaded");
  return engine;
}

/** Every spell by name, each school's fighter stats, what each trait does: the rules' own tables. */
export let SPELLS = {};
export let STATS = {};
export let TRAITS = {};
/** The rage an ultimate needs, the seconds a bar skill waits unused before it leaves, the decor effects and the
 *  tiers, lowest first. */
export let RAGE = 0;
export let STALE = 0;
export let DECOR = {};
export let TIERS = [];
/** How many of the rules' pixels make one of the page's. */
export let UNIT = 1;

/**
 * Read the rules' tables from the arena's wasm once it has loaded (`site/wasm/arena_wasm.js`, its default export
 * awaited): `pkg` holds `Duel`, `Corner`, `admitBot`, `duelSpells`, `duelStats`, `duelTraits`, `duelConstants`, `damageOf` and
 * `orbOffset`.
 */
export function useRules(pkg) {
  engine = pkg;
  SPELLS = pkg.duelSpells();
  STATS = pkg.duelStats();
  TRAITS = pkg.duelTraits();
  ({ RAGE, STALE, DECOR, TIERS, UNIT } = pkg.duelConstants());
}

/** The first bytes of every WebAssembly module. */
const WASM = [0x00, 0x61, 0x73, 0x6d];

/** Whether `bytes` are a WebAssembly module, as every bot is. */
export const isBotModule = (bytes) => WASM.every((b, i) => bytes[i] === b);

/**
 * Seat a bot in each corner of every duel created from now on: `[left, right]` are the bytes of a bot module (a
 * `.wasm` of the arena-bot-1 ABI, `docs/bot.md`; an ONNX model becomes one with `bots/onnx`). A module is admitted as a
 * submission is (its hash is the bot's identity), and each corner runs its own instance of it, the corner's whole
 * match: it remembers from one duel to the next. Resolves to each corner's `{kind: "module", hash}`; rejects with why a
 * file is not a bot for this season.
 */
export async function useBots(files) {
  const pkg = loaded();
  const budgets = pkg.botBudgets();
  // each module admitted and compiled once, however many corners play it
  const compiled = new Map();
  const compile = (bytes) => {
    if (!compiled.has(bytes)) {
      compiled.set(
        bytes,
        (async () => {
          const admitted = pkg.admitBot(bytes);
          const hash = admitted.hash;
          const module = await WebAssembly.compile(admitted.bytes);
          admitted.free();
          return { module, hash };
        })(),
      );
    }
    return compiled.get(bytes);
  };
  const made = [];
  try {
    for (const bytes of files) {
      if (!isBotModule(bytes)) throw new Error("not a bot module (.wasm); build one from an ONNX model with bots/onnx");
      const { module, hash } = await compile(bytes);
      const instance = await WebAssembly.instantiate(module, {});
      made.push({ corner: pkg.Corner.bot(botInstance(instance, budgets)), kind: "module", hash });
    }
  } catch (e) {
    for (const m of made) m.corner.free();
    throw e;
  }
  dropCorners();
  corners = made.map((m) => m.corner);
  return made.map(({ kind, hash }) => ({ kind, hash }));
}

function dropCorners() {
  if (corners) for (const c of corners) c.free();
  corners = null;
}

/**
 * An instance of an admitted bot module as the duel asks it (the native host's `Bot`, `crates/arena-bot`): each call
 * is given its fuel through the module's own counter, the input is written where `arena_input` says. A trap or an
 * empty counter throws, and the corner gives up that round.
 */
function botInstance(instance, budgets) {
  const x = instance.exports;
  const fuel = x[budgets.fuel];
  const setup = BigInt(budgets.setupFuel);
  const decide = BigInt(budgets.decideFuel);
  fuel.value = setup;
  if (x.arena_abi() !== budgets.abi) throw new Error(`the module is not arena-bot-${budgets.abi}`);
  fuel.value = setup;
  const at = x.arena_input() >>> 0;
  const obsBytes = budgets.obs * 4;
  return {
    start(seed) {
      fuel.value = setup;
      x.arena_start(seed | 0);
    },
    round() {
      fuel.value = setup;
      x.arena_round();
    },
    decide(obs, legal, u) {
      // the module's memory may have grown since the last call: a fresh view each time
      const mem = new Uint8Array(x.memory.buffer, at, obsBytes + budgets.actions);
      mem.set(new Uint8Array(obs.buffer, obs.byteOffset, obsBytes));
      mem.set(legal, obsBytes);
      fuel.value = decide;
      return x.arena_decide(u);
    },
  };
}

/** The lengths of a command or event of a step (where it is, a streak's ends, an effect's size), and of the duel's
 *  view (the arena's edges and floor, each fighter's place and speed). */
const LENGTHS = ["x", "y", "x0", "y0", "x1", "y1"];
const OPT_LENGTHS = ["width", "height"];
const FIGHTER_LENGTHS = ["x", "air", "vx", "vy"];

/** Every length in `o` (named in `keys`) divided by `UNIT`: the rules' pixels in the page's. */
function inPage(o, keys) {
  for (const k of keys) if (typeof o[k] === "number") o[k] /= UNIT;
  return o;
}

/** The rules' view in the page's pixels. */
function viewOf(rs) {
  const v = inPage(rs.view(), ["left", "right", "ground"]);
  for (const f of v.fighters) inPage(f, FIGHTER_LENGTHS);
  return v;
}

/**
 * A new duel in the arena from `left` to `right` (the page's pixels), standing on `ground`, flying no higher than `sky`.
 * `effects` are the effect ids the page can play (the engine's `effects()`): some rules play only when their effect is
 * there. `seats`, for tests, seats another mind in a corner:
 * `["policy" | "brain" | "first", ...]` (each corner's bot when `useBots` has run, else the brain, when left out).
 */
export function createDuel({ seed = 1, left, right, ground, effects, sky, seats }) {
  const pkg = loaded();
  const config = { seed, left: left * UNIT, right: right * UNIT, ground: ground * UNIT, sky: sky * UNIT, effects };
  const rs = corners ? pkg.Duel.withCorners(config, seats, ...corners) : new pkg.Duel(config, seats);
  return { rs, seed, ...viewOf(rs) };
}

/** Advance the duel by `dt` seconds; returns the effect commands and events of this step, in the page's pixels. */
export function step(duel, dt) {
  const out = duel.rs.step(dt);
  for (const c of out) {
    inPage(c, LENGTHS);
    if (c.opts) inPage(c.opts, OPT_LENGTHS);
  }
  Object.assign(duel, viewOf(duel.rs));
  return out;
}

/** Let go of a duel the page no longer shows. */
export function freeDuel(duel) {
  duel.rs.free();
}

/**
 * The fighters' painter for a world `w` x `h` of the page's pixels, drawing the fighters in the rules' (`UNIT` times
 * finer), with a sprite sheet for each school (`sheets`: `{ schools: { thunder: { layout, rgba }, ... } }`, each
 * school its own fighter, from `loadFighterSheets` in duel-hero.js). Free it with `.free()`.
 */
export function createFighters(w, h, sheets) {
  const fighters = new (loaded().Fighters)(w, h, UNIT);
  try {
    for (const [school, s] of Object.entries(sheets.schools)) fighters.add(school, s.layout, s.rgba);
  } catch (e) {
    fighters.free();
    throw e;
  }
  return fighters;
}

/**
 * The spell effects' painter for a world `w` x `h` of the page's pixels, drawing them in the rules' (`UNIT` times
 * finer), with each effect's sprite sheet (`sheets`: `[{ fx, layout, rgba }, ...]`, from `loadVfxSheets` in
 * duel-hero.js). Free it with `.free()`.
 */
export function createVfx(w, h, sheets) {
  const vfx = new (loaded().Vfx)(w, h, UNIT);
  try {
    for (const s of sheets) vfx.add(s.fx, s.layout, s.rgba);
  } catch (e) {
    vfx.free();
    throw e;
  }
  return vfx;
}

/** Fighter f's skill bar: three bar slots, then the ultimate. */
export const skillBar = (f) => f.skillBar;

/** What the HUD shows under fighter f's bars, good first, then bad. */
export const buffsOf = (f) => f.buffs;

/** The damage `by` (`{stats, fury}`) deals `f` (`{stats, defDown}`) with a blow of `power` and `type`. */
export function damageOf(by, f, power, type, crit, roll) {
  return loaded().damageOf({ stats: by.stats, fury: by.fury }, { stats: f.stats, defDown: f.defDown }, power, type, crit, roll);
}

/** Where an orb bobs `t` seconds after it was cast, from its line: `[dx, dy]`. */
export const orbOffset = (t) => loaded().orbOffset(t);
