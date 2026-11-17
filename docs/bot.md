# The bot

What a bot sees, what it can do, when it decides, and the file a player submits. The code is the source of truth:
`crates/arena-mind/src/policy/` (`observe.rs`, `action.rs`, `mind.rs`), `crates/pixfx-duel/src/seat.rs` and
`crates/arena-bot`.

## What it sees: 354 numbers

Each decision the bot gets `OBS` = 354 floats, in a frame where the foe is always ahead (positive x), so the same bot
plays from either corner. Every value is clamped to [-4, 4], anything not finite reads 0, and times are in seconds
unless a scale is given. Lengths are in the world's pixels (the page's), `UNIT` (4) of the rules' each (`docs/rules.md`): a bot
sees the same numbers whatever the arena's pixel size, and the pixels below are the world's too.

**Seeing late.** A blow the foe winds up reaches the bot 0.16 s after it starts (`TEMPERAMENT.reaction`), and a
threat in flight appears once it is 0.16 s old: that is the time to react to a danger. Everything else about the foe
(where it is, a walk, a cast it gathers that is not a blow, a recovery) is current, so an opening can be taken the
moment it shows (`arena-mind/src/view.rs`).

| Range | Count | What |
|---|---|---|
| 0..18 | 18 | **Me.** Distance to the wall behind and the wall ahead (over the arena's width), height (over the ceiling), facing the foe, hp / 100 (hp is a percentage of full life, so this reads 0..1), qi / 100, rage (over a full ultimate), flying, hover (over the ceiling), speed toward the foe / 200, then flags: can act, silenced, sealed, barrier up, ward up (indexes 0..15). Winding up, recovering, seconds left in that phase (15..18). |
| 18..56 | 38 | **Foe.** Distance (over the width), height, flying, hp / 100, qi / 100 (18..23). What it does: idle, moving toward me, moving away, guarding, meditating, flying (23..29). Winding up, recovering (29..31). The kind of skill it uses, one-hot over 12 kinds (31..43). How far into its phase as a share (43), seconds left (44), the phase's span (45), the skill's reach / 100 (46), damage / 50 (47), whether it is an ultimate (48); 29..49 are zero when it uses no skill. Out for, open in, frozen, shocked, barrier, the share of its skills ready, my seeker out (49..56). |
| 56..104 | 4 x 12 | **Threats** in flight, nearest first, zero when absent: present, shape (projectile, area, seeker, field, barrage), distance ahead / 100, height / 50, speed toward me / 200, lands in, size / 50, damage / 50. |
| 104..349 | 7 x 35 | **Slots**, zero when empty: the three basics, the three bar slots, the ultimate. Each: present, ready, kind (12), cost / 50, damage x power / 50, reach / 100, windup, recover, radius / 50, ultimate (+0..+21); then what it inflicts (+21..+35): freeze s, stun s, silence s / 2, defence-down s / 5, defence lost (share), burn s / 3, burn's share of attack a second x 5, chill stacks a hit adds now over the most it holds, slow per chill stack x 4, shock stacks a hit adds now over the most it holds, hold per shock twitch x 4, share of attack per shock twitch x 10, pull / 100, rage stolen over a full ultimate. |
| 349..354 | 5 | **Tag.** Its seat (0 left, 1 right: the slots that held the old schools) and its style (aggressive, balanced, defensive), one-hot. |

The 12 skill kinds, in order: melee, dive, dash, blast, bolt, seeker, area, field, heal, ward, barrier, blink.

**The damage system in these slots.** The rules play by the damage system (`docs/rules.md`), but the 354 numbers keep
their shape, so a bot of the old rules still runs: what the new state means is mapped onto the slots that fit it and
the rest reads 0 (design `docs/specs/2026-11-17-damage-system-design.md`, 16, lists what needs slots of its own).

- hp and qi are shares of the fighter's full life and qi; *silenced* is a lost soul (thất hồn); *sealed* is a blink
  lock or a root; the foe's *frozen* is its root's seconds left (nothing freezes), *shocked* its hold.
- A slot's *damage* is what the skill is expected to deal, in the old power's units (`data::POWER`); *power* is 1.
- What a slot inflicts, by the share of each kind of damage in the skill: freeze s is a hold (Định thân), stun s a
  stun, knockdown or jolt, silence s a lost soul; defence-down s and defence lost are sunder (metal); burn s and burn's
  rate are the burn (fire); the chill slots are soak (water) and the slow is bind (wood); the shock slots are jolt
  (thunder); rage stolen is 0.

A skill is known by its kind and its numbers, never its name, so a bot reads a skill the bar has just drawn the same
way as one it has seen a thousand times.

What it does not see: a skill's cooldown left and its stored uses (only whether it is ready), its supports (a slot
shows the skill's base cost and expected damage before them), the loadouts' keystones and roots, the energy shield,
the sense, the resources besides qi and rage, the ailments stacked on either fighter, and the foe's minions, totems,
traps and mines. The two corners
see the same kinds of numbers, so neither side knows more.

## What it can do: 30 acts

`ACTIONS` = 30, each at a fixed index:

| Index | Act |
|---|---|
| 0 | wait |
| 1 | walk toward the foe |
| 2 | walk away |
| 3 | jump |
| 4 | duck (held 0.4 s) |
| 5 | guard (held 0.4 s) |
| 6 | fly high (to 0.6 of the ceiling) |
| 7 | land |
| 8 | meditate |
| 9 + 3 x slot + aim | cast the skill in `slot` (0..7) with `aim` 0, 1 or 2 |

Where an aim lands:

| Skill kind | aim 0 | aim 1 | aim 2 |
|---|---|---|---|
| area | on the foe | 30 px past the foe | 30 px short of the foe |
| blink | 20 px on this side of the foe | 20 px past the foe | a full-reach blink backwards, away from the foe |
| any other | at the foe | (illegal) | (illegal) |

**Legal acts.** Each decision also comes with a mask of the acts that can happen now (`legal`):

- wait is always legal;
- while the fighter cannot act (in its own cast, or held by a status), nothing else is;
- jump and fly high whenever not flying (a jump in mid-air does nothing), land only when flying;
- a cast only from a filled slot whose skill is ready: cooled down, qi enough to pay, full rage for the ultimate;
  a silenced fighter casts only its blade (slash and dive), a sealed one cannot blink;
- aiming ahead or short only for areas and blinks.

An illegal act is played as a wait. An act that was legal when chosen but is no longer legal on the next tick (when
it is played) is dropped, and the fighter keeps doing what it was doing.

## When it decides

- The bot thinks every 0.12 s, jittered between 0.8x and 1.2x on its own seeded dice (`TEMPERAMENT.think`): at most
  about eight decisions a second.
- A decision comes only while the fighter can act: never during its own cast, a stagger, a freeze or shock, the
  crouch before a jump, or the 0.4 s of a duck. After a round starts, the first decision comes 0.4-0.7 s in.
- What it decides on one tick is played on the next.
- An act lasts until the next one: walking and meditating carry on, and `wait` stops both.
- Hit-stop on the page pauses the whole duel, so it changes nothing for a bot.

These are the same for every bot and every style, and the same as the hand-written brain's, so no bot is faster than
another: the arena measures decisions, not reflexes.

## What a player submits

One **bot module** (`.wasm`): any model of any size within the budgets below, memory across a match, and its own way
of picking an act. Most bots start as an ONNX model, the policy exported from PyTorch (or any library): input float32
`[1, 354]`, output the 30 acts' logits, float32 `[1, 30]`. One command builds it into a module with the `bots/onnx`
template ("Bring your own model" in `docs/training.md`):

```
cargo run -p arena-bot --example onnx -- mine.onnx mine.wasm
```

The template samples the act as the house bot does: illegal acts masked out, a softmax over the rest, read with the
seat's draw `u`. A bot that wants to always play its best act trains its logits sharp, or picks otherwise in its own
`arena_decide`.

## The bot module

A WebAssembly module of the ABI `arena-bot-1` (`crates/arena-bot`; the design and its reasons:
`docs/specs/2026-11-16-arena-wasm-bots-design.md`). It **imports nothing** and exports:

| Export | Signature | What |
|---|---|---|
| `memory` | memory | its linear memory |
| `arena_abi` | `() -> i32` | returns `1` |
| `arena_input` | `() -> i32` | the address of a buffer the arena writes before each decision: the 354 inputs as `f32` LE, then the 30 acts' legal mask as bytes (0 or 1). Read once, when the instance is made |
| `arena_start` | `(seed: i32)` | a new fight of the match; `seed` is the bot's own dice for it |
| `arena_round` | `()` | a new round, the first included |
| `arena_decide` | `(u: f32) -> i32` | the act, 0..30, for the decision the buffer holds; `u` is the seat's draw in [0, 1) |

- **One instance a match.** Its memory lives across every fight and round of a match (every seed, both ways), so a
  bot may learn its foe; it is thrown away after the match.
- **It picks its own act**: sampling with `u`, argmax, a search, rules. An act outside 0..30 is a wait, and an act
  that is not legal is dropped.
- **Budgets**: the module at most 32 MiB; memory at most 256 MiB (the arena caps the declared maximum);
  20M fuel a decision, 200M for each `arena_start`, `arena_round` and the module's own start function. Fuel is about
  one unit a WebAssembly instruction, counted by code the arena writes into the module, so it runs out on the same
  decision on every machine and in the browser. The house AI as a module uses about 3M a decision; a network some 6
  times its size still fits.
- **A trap, or running out of fuel, loses the round** it happened in; the foe wins it at once. A fault in
  `arena_start` loses that fight's first round. The instance plays on from the next round with its memory as the
  fault left it.
- **Features**: WebAssembly 2.0 with 128-bit SIMD, tail calls, exception handling (`exnref`), typed function
  references and extended constants. Refused: relaxed SIMD, GC, memory64, multiple memories, threads, and every
  proposal not shipped in all browsers. What gives different bits on different machines is refused.

A bot can also be written by hand in any language that compiles to WebAssembly.

## The house AI

The AI the site plays is an ONNX model like any player's: `crates/pixfx-duel/weights/policy.onnx` (`train/export.py`
writes it; 128 wide, two residual blocks of 256, a simplicial embedding, about 198K parameters). Natively it runs with
tract (`arena_bot::onnx`: the ladder, the tests, a trainer's sparring partner, `pixfx_duel.HOUSE` in Python); on the
site and in the tournament it is the module `bots/onnx` builds from it, `bots/policy.wasm`:

```
cargo run -p arena-bot --example onnx -- crates/pixfx-duel/weights/policy.onnx policy.wasm
```

The pages admit a module the way a submission is admitted, then run it with the browser's own WebAssembly, on the same
fuel, and play the same fights bit for bit as the tournament's native host (`site/tests/policy.test.js` against
`crates/arena-bot/tests/goldens/house_fight.json`). The home page takes a `.wasm` in each corner: drop one in and it fights at once.
