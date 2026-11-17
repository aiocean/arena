// The house bot module on the page: the house AI's ONNX model built into a bot module (site/assets/bots, built by
// site/embed.sh), admitted and run the way a player's module is. A seed replays the same fight, only bot modules are
// seated, and a module that traps gives up its rounds instead of stopping the duel.
import { afterAll, describe, expect, test } from "bun:test";
import { createDuel, freeDuel, isBotModule, step, useBots } from "../assets/duel.js";
import { effects, house, rules } from "./engine.js";

const all = new Set(effects());
const make = (seed, seats) => createDuel({ seed, left: 0, right: 220, ground: 150, sky: 96, effects: [...all], seats });
const fight = (duel, seconds) => {
  for (let i = 0; i < seconds * 60; i++) step(duel, 1 / 60);
  return { time: duel.time, round: duel.round, wins: duel.wins, fighters: duel.fighters.map((f) => [f.hp, f.qi, f.rage, f.x, f.air]) };
};

/** A module of the ABI that traps in every decision: memory, arena_abi = 1, arena_input = 0, empty start and round,
 *  arena_decide = unreachable. */
function trapping() {
  const str = (s) => [s.length, ...new TextEncoder().encode(s)];
  const section = (id, body) => [id, body.length, ...body];
  const exp = (name, kind, i) => [...str(name), kind, i];
  // types: 0 () -> i32, 1 (i32) -> (), 2 () -> (), 3 (f32) -> i32
  const typeSec = [4, 0x60, 0, 1, 0x7f, 0x60, 1, 0x7f, 0, 0x60, 0, 0, 0x60, 1, 0x7d, 1, 0x7f];
  const funcs = [5, 0, 0, 1, 2, 3];
  const memory = [1, 0, 1];
  const exports = [6, ...exp("memory", 2, 0), ...exp("arena_abi", 0, 0), ...exp("arena_input", 0, 1), ...exp("arena_start", 0, 2), ...exp("arena_round", 0, 3), ...exp("arena_decide", 0, 4)];
  const body = (code) => [code.length + 1, 0, ...code];
  const code = [5, ...body([0x41, 1, 0x0b]), ...body([0x41, 0, 0x0b]), ...body([0x0b]), ...body([0x0b]), ...body([0x00, 0x0b])];
  return new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0, ...section(1, typeSec), ...section(3, funcs), ...section(5, memory), ...section(7, exports), ...section(10, code)]);
}

afterAll(() => useBots([house, house]));

describe("a bot module on the page", () => {
  test("the house bot is a module the arena admits", () => {
    expect(isBotModule(house)).toBe(true);
    expect(isBotModule(new Uint8Array([0x08, 0x01, 0x12, 0x00]))).toBe(false);
    const admitted = rules.admitBot(house);
    expect(admitted.hash).toMatch(/^[0-9a-f]{64}$/);
    admitted.free();
  });

  test("a file that is not a bot module of the season is refused", async () => {
    expect(() => rules.admitBot(new Uint8Array([0, 0x61, 0x73, 0x6d, 1, 0, 0, 0, 0xff]))).toThrow();
    await expect(useBots([new Uint8Array([1, 2, 3]), house])).rejects.toThrow();
  });

  test("the house bot replays the same fight from a seed", async () => {
    const seated = await useBots([house, house]);
    expect(seated.map((s) => s.kind)).toEqual(["module", "module"]);
    const play = (seed) => {
      const duel = make(seed);
      const got = fight(duel, 60);
      freeDuel(duel);
      return got;
    };
    for (const seed of [1, 2]) expect(play(seed)).toEqual(play(seed));
  }, 60_000);

  test("an ONNX model is not a bot until it is built into a module", async () => {
    // the first bytes of an ONNX protobuf
    await expect(useBots([new Uint8Array([0x08, 0x08, 0x12, 0x07]), house])).rejects.toThrow(/not a bot module/);
  });

  test("a module that traps gives up every round, and the duel goes on", async () => {
    await useBots([trapping(), house]);
    const duel = make(5);
    const before = duel.round;
    fight(duel, 20);
    expect(duel.round).toBeGreaterThan(before);
    expect(duel.wins[0]).toBe(0);
    expect(duel.wins[1]).toBeGreaterThan(0);
    freeDuel(duel);
  });
});
