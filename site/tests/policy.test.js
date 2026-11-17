// The house bot on the page: the browser's WebAssembly plays the same fights as the tournament's native host
// (crates/arena-bot's goldens/house_fight.json, in the rules' pixels: the page's arena times UNIT), each corner plays
// its own module, and a decision costs well under a frame.
import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { createDuel, freeDuel, step, useBots } from "../assets/duel.js";
import { effects, house, rules } from "./engine.js";

const all = new Set(effects());
const golden = JSON.parse(readFileSync(new URL("../../crates/arena-bot/tests/goldens/house_fight.json", import.meta.url), "utf8"));
const make = (seed, seats) => createDuel({ seed, left: 0, right: 220, ground: 150, sky: 96, effects: [...all], seats });

/** A module of the ABI whose every decision is `act`: memory, arena_abi = 1, arena_input = 0, empty start and round. */
function always(act) {
  const str = (s) => [s.length, ...new TextEncoder().encode(s)];
  const section = (id, body) => [id, body.length, ...body];
  const exp = (name, kind, i) => [...str(name), kind, i];
  const typeSec = [4, 0x60, 0, 1, 0x7f, 0x60, 1, 0x7f, 0, 0x60, 0, 0, 0x60, 1, 0x7d, 1, 0x7f];
  const exports = [6, ...exp("memory", 2, 0), ...exp("arena_abi", 0, 0), ...exp("arena_input", 0, 1), ...exp("arena_start", 0, 2), ...exp("arena_round", 0, 3), ...exp("arena_decide", 0, 4)];
  const body = (code) => [code.length + 1, 0, ...code];
  const code = [5, ...body([0x41, 1, 0x0b]), ...body([0x41, 0, 0x0b]), ...body([0x0b]), ...body([0x0b]), ...body([0x41, act, 0x0b])];
  return new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0, ...section(1, typeSec), ...section(3, [5, 0, 0, 1, 2, 3]), ...section(5, [1, 0, 1]), ...section(7, exports), ...section(10, code)]);
}

describe("the house bot", () => {
  test("the page's fight equals the native host's", async () => {
    await useBots([house, house]);
    for (const seed of [1, 2, 3]) {
      const duel = make(seed);
      for (let i = 0; i < 60 * 60; i++) step(duel, 1 / 60);
      const got = {
        time: duel.time,
        round: duel.round,
        wins: duel.wins,
        // the golden's places are in the rules' pixels, as the rules' own view has them
        fighters: duel.rs.view().fighters.map((f) => [f.hp, f.qi, f.rage, f.x, f.air]),
      };
      expect(got).toEqual(golden[String(seed)]);
      freeDuel(duel);
    }
  }, 60_000);

  test("a decision costs under 2 ms in the browser's WebAssembly", async () => {
    // the same fight's ticks with the house bot and with the first mind in both corners: what the bot adds, over the
    // decisions it makes (each fighter about every 0.12 s), is what a decision costs; best of three runs each
    await useBots([house, house]);
    const ticks = 60 * 60;
    const time = (seats) => {
      let best = Infinity;
      for (let rep = 0; rep < 3; rep++) {
        const duel = make(7, seats);
        const start = performance.now();
        for (let i = 0; i < ticks; i++) step(duel, 1 / 60);
        best = Math.min(best, performance.now() - start);
        freeDuel(duel);
      }
      return best;
    };
    const extra = time(["policy", "policy"]) - time(["first", "first"]);
    const decisions = (2 * ticks) / 60 / 0.12;
    console.log(`house bot decision in wasm: ${(extra / decisions).toFixed(3)} ms over ${decisions} decisions`);
    expect(extra / decisions).toBeLessThan(2);
  }, 60_000);

  test("a duel without bots seats the brain, and a policy seat asks for a bot", () => {
    const config = { seed: 1, left: 0, right: 220, ground: 150, sky: 96, effects: [...all] };
    const brain = new rules.Duel(config, undefined);
    const brains = new rules.Duel(config, ["brain", "brain"]);
    for (let i = 0; i < 20 * 60; i++) {
      brain.step(1 / 60);
      brains.step(1 / 60);
    }
    expect(brain.view()).toEqual(brains.view());
    brain.free();
    brains.free();
    expect(() => new rules.Duel(config, ["policy", "brain"])).toThrow(/needs a bot module for its corner/);
    expect(rules.Policy).toBeUndefined();
  });

  test("each corner plays its own bot, and a corner the brain sits in ignores its bot", async () => {
    const fight = async (left, right, seats) => {
      await useBots([left, right]);
      const duel = make(5, seats);
      for (let i = 0; i < 20 * 60; i++) step(duel, 1 / 60);
      const view = duel.fighters.map((f) => [f.hp, f.qi, f.x]);
      freeDuel(duel);
      return view;
    };
    const waits = always(0);
    const hh = await fight(house, house);
    expect(await fight(house, house)).toEqual(hh);
    expect(await fight(house, waits)).not.toEqual(hh); // the right corner plays its own
    expect(await fight(waits, waits)).not.toEqual(await fight(house, waits)); // and so does the left
    expect(await fight(house, waits, ["policy", "brain"])).toEqual(await fight(house, house, ["policy", "brain"]));
    await useBots([house, house]);
  }, 60_000);

  test("each fighter shows the style it fights in", () => {
    const duel = make(3);
    for (const f of duel.fighters) expect(["aggressive", "balanced", "defensive"]).toContain(f.style);
    freeDuel(duel);
  });
});
