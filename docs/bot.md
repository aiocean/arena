# The bot

What a bot sees, what it can do, when it decides, and the file a player submits. The code is the source of truth:
`crates/arena-mind/src/policy/` (`observe.rs`, `action.rs`, `mind.rs`), `crates/arena-duel/src/seat.rs` and
`crates/arena-bot`.

## What it sees: 2055 numbers

Each decision the bot gets `OBS` = 2055 floats, in a frame where the foe is always ahead (positive x), so the same bot
plays from either corner. Every value is clamped to [-4, 4], anything not finite reads 0, and times are in seconds
unless a scale is given. Lengths are in the world's pixels (the page's), `UNIT` (4) of the rules' each (`docs/rules.md`): a bot
sees the same numbers whatever the arena's pixel size, and the pixels below are the world's too.

**Seeing late.** A blow the foe winds up reaches the bot 0.16 s after it starts (`TEMPERAMENT.reaction`), and a
threat in flight appears once it is 0.16 s old: that is the time to react to a danger. Everything else about the foe
(where it is, a walk, a cast it gathers that is not a blow, a recovery) is current, so an opening can be taken the
moment it shows (`arena-mind/src/view.rs`). The foe's cast block (316..327) follows its doing: it reads 0 while the
delay still hides a blow it winds up.

**ABI 5 (RULES 27) appended 166 numbers to ABI 4's 1889, which keep their indices and meaning:** what the
fighter remembers. Its own last 3 acts (1889..1979) and the last 4 things it saw the foe start (1979..2055), each with
how long ago, so a bot that ducked a moment ago knows it, and one that saw the foe guard, then walk in, then wind up a
bolt can read the pattern. The memory is the arena's, the same for every bot, and it forgets at each round. ABI 5 also
added two acts after the casts (27 and 28: a duck and a guard held 1 s), so a bot can commit to a long duck in one
decision instead of three short ones. A model of ABI 4 carries over with its first layer widened, zeros on the new
inputs, and two new logits that start far below the rest (`train/widen.py`): it then plays as it did.

**ABI 4 (RULES 25) appended 148 numbers to ABI 3's 1741: 0..1741 keep their indices and meaning**, except that a
slot's *ready* (and the foe's slots' *ready*, and the foe's share of its skills ready) now also says the foe stands
inside the skill's range band (`docs/rules.md`, "Range bands"): a skill out of its band cannot be cast, and is not
legal. The new numbers (1741..1889) say each slot's band and why it cannot be cast, the foe's bands, and how long its
buffs, the domains, the fields and the auras last. A model of ABI 3 carries over with its first layer widened, zeros
on the new inputs (`train/widen.py`): it then plays exactly as it did, and training teaches it the rest.

ABI 3 appended to ABI 2's 305 numbers: 0..305 keep their indices and meaning, except that a slot's cost and damage
are now what the skill builds to with its supports and keystones.

| Range | Count | What |
|---|---|---|
| 0..17 | 17 | **Me.** Distance to the wall behind and the wall ahead (over the arena's width), height (over the ceiling), facing the foe, hp / 100 (hp is a percentage of full life, so this reads 0..1), qi / 100, flying, hover (over the ceiling), speed toward the foe / 200, then flags: can act, silenced, sealed, barrier up, ward up (indexes 0..14). Winding up, recovering, seconds left in that phase (14..17). |
| 17..54 | 37 | **Foe.** Distance (over the width), height, flying, hp / 100, qi / 100 (17..22). What it does: idle, moving toward me, moving away, guarding, meditating, flying (22..28). Winding up, recovering (28..30). The kind of skill it uses, one-hot over 12 kinds (30..42). How far into its phase as a share (42), seconds left (43), the phase's span (44), the skill's reach / 100 (45), damage / 50 (46); 28..47 are zero when it uses no skill. Out for, open in, frozen, shocked, barrier, the share of its skills ready, my seeker out (47..54). |
| 54..102 | 4 x 12 | **Threats** in flight, nearest first, zero when absent: present, shape (projectile, area, seeker, field, barrage), distance ahead / 100, height / 50, speed toward me / 200, lands in, size / 50, damage / 50. |
| 102..300 | 6 x 33 | **Slots**, zero when empty: the basics (the blink, then two always empty since RULES 20), the three bar slots. Each: present, ready, kind (12), cost / 50, damage x power / 50, reach / 100, windup, recover, radius / 50 (+0..+20); then what it inflicts (+20..+33): freeze s, stun s, silence s / 2, defence-down s / 5, defence lost (share), burn s / 3, burn's share of attack a second x 5, chill stacks a hit adds now over the most it holds, slow per chill stack x 4, shock stacks a hit adds now over the most it holds, hold per shock twitch x 4, share of attack per shock twitch x 10, pull / 100. |
| 300..305 | 5 | **Tag.** Its look (Vương Lâm, Thất Thải Đạo Nhân, in that order: its art, so its body's boxes; the slots that held the old schools) and its style (aggressive, balanced, defensive), one-hot. Never its seat: everything is seen from its side, and a seat would only let a policy play one seat better than the other. Looks are drawn apart from seats, so a fighter meets both looks in both seats. |
| 305..316 | 11 | **My cast.** Zero when not casting: casting, in its active window, seconds to the window (0 in it or after), the window's length, seconds until punishable (until its recovery begins; 0 while it recovers), the recovery's span (longer for an unsafe blow that was guarded), a hitbox, the hitbox's near and far x ahead of me / 100, its bottom and top over the ground / 50. The active window opens as the wind-up ends and lasts while the clip's active frames show; a clip with none reads 0 for both. The hitbox is the blade's, now or on its active frame; zero for a clip with no blade out ahead. |
| 316..327 | 11 | **Foe's cast.** The same for the foe's cast, the hitbox still from my side; zero until its doing (17..54) shows it acting. |
| 327..1215 | 6 x 148 | **Slot builds**, in the slots' order, zero when empty. Each: cooldown left, cooldown / 10, stored uses / 4, most uses / 4, windup, recover, the recovery a guarded blow adds (0 for a safe skill), the active window's start (from the cast) and length, damage / 50, qi / 50, extra projectiles / 4, strikes / 4, area factor (1 as cast), pierce / 4, chain / 4, fork / 4, inert supports / 3 (+0..+18); then each ailment's stacks a landing blow lays besides its parts', over the stacks that make it full (+18..+29); then the supports it carries, one flag each in the support pool's order (119, +29..+148). |
| 1215..1441 | 2 x 113 | **Effects** on me, then on the foe. Each ailment (11): stacks over its full, full / 10, the longest time left / 10, and for burn and bleed the strongest one's damage a second over full life x 10 (+0..+44). Stunned, rooted, down, shocked, soul lost, blink sealed, no guard, choked, rotting, seconds (+44..+53). Each of the 12 curses' time left / 10 (+53..+65). The 15 auras held (+65..+80). Its skills' buffs: how many / 3, the longest's time left / 10, their sizes summed (a stance's damage more, a blessing's damage taken less) (+80..+83). Energy shield share, ward charges / 3, barrier seconds (+83..+86). Its loadout's keystones, 20 flags, the 15th always 0 (+86..+106). Its root, one-hot over 7 (+106..+113). |
| 1441..1537 | 8 x 12 | **Units** on the field, both sides', nearest first, zero when absent: present, mine, kind (trap, mine, totem, minion), distance ahead / 100, height over my feet / 50, life share, time left / 10 (40 s for one that lasts), armed, seconds to its next blow (0 for a trap or a mine). |
| 1537..1741 | 6 x 34 | **Foe's slots**, its basics (the blink, two empty) then its bar, zero when empty: present, ready, cooldown left, stored uses / 4, kind (12), windup, recover, the active window's start and length, damage / 50, qi / 50, reach / 100, then each ailment a blow lays over its full (11). |
| 1741..1795 | 6 x 9 | **Why** each of my slots can or cannot be cast, in the slots' order, zero when empty (ABI 4): has a band, its min / 100, max / 100 (all 0 for a skill castable at any distance), the foe inside it; then one flag for each reason it cannot be cast now, any number at once, none when it is ready: cooling down (no use stored), short of qi, the foe too near (closer than min), too far (farther than max), barred (silenced, sealed or rooted for a movement, no guard, a curse, its last binding blow still playing, its aura lit already, its cards spent, or I cannot act). |
| 1795..1819 | 6 x 4 | **Foe's bands**, in its slots' order, zero when empty: has a band, min / 100, max / 100, I stand inside it (it could cast it at me from where it is). |
| 1819..1839 | 4 x 5 | **Buffs**: my own timed buffs from my skills, soonest to end first, zero when absent: present, stance (more damage), blessing (less taken), other, time left / 10. |
| 1839..1889 | 10 x 5 | **In play**: the domains (2: mine first), the fields (4: the vortices standing, nearest first) and the auras (4: mine first), zero when absent. Each: present, mine, time left / 10 (4, forty seconds, for an aura: it lasts while its holder keeps it), seconds until it works (an aura's wind-up; 0 for the others), where it stands ahead of me / 100 (0 for an aura, which is on its holder). |
| 1889..1979 | 3 x 30 | **My acts**, the last 3 I chose, newest first, zero when none (ABI 5): the act one-hot over the 29 acts (the table below), seconds since I chose it / 2. |
| 1979..2055 | 4 x 19 | **Foe's acts**, the last 4 things I saw the foe start, newest first, zero when none (ABI 5): idle, moving toward me, moving away, guarding, meditating, flying (6), the kind of skill it uses, one-hot over 12 as in the foe's block (only for a skill), seconds since / 2. A start is a change in what it is seen doing, through the same reaction delay as the foe's block. |

The 12 skill kinds, in order: three reserved, then blast, bolt, seeker, area, field, heal, ward, barrier, blink. The
three reserved slots held melee, dive and dash until the arena went spells only (RULES 19): no skill is one of them
now, so they are always 0, and the inputs keep their shape. A cast's hitbox (in "My
cast" and "Foe's cast") was a blade's: a spell draws none, so it reads 0 too.

The damage system's lists, in their orders in `crates/arena-calc`: the 11 ailments (`ailment::Ailment::ALL`: bleed,
daze, sunder, bind, soak, burn, heavy, jolt, glare, wither, dread), the 119 supports (`support::SUPPORTS`), the 12
curses (`curse::Kind::index`), the 15 auras (`aura::AuraKind::ALL`), the 20 keystone flags (`keystone::Keystone::ALL`,
Cực Cảnh of any kind is one; flag 14, where Đạo Dung Thân was until RULES 20, is always 0) and the 7 roots (`loadout::ROOTS`: metal, wood, water, fire, earth, light, yin).

**The damage system in ABI 2's slots.** In 0..305 what the damage system's state means is mapped onto the slots
that fit it and the rest reads 0 (design `docs/specs/2026-11-17-damage-system-design.md`, 16); 305..1741 show it as
it is.

- hp and qi are shares of the fighter's full life and qi; *silenced* is a lost soul (thất hồn); *sealed* is a blink
  lock or a root; the foe's *frozen* is its root's seconds left (nothing freezes), *shocked* its hold.
- A slot's *damage* is what the skill is expected to deal, in the old power's units (`data::POWER`): its base
  expected damage times what its build's blow deals (steps 1 to 4, times its strikes) over what the same skill deals
  built bare (no supports, no keystones, a fresh fighter); its *cost* is the qi it builds to. *Power* is 1. Both are
  built at each round's start and at each cast, as the slot builds (327..1215) are.
- What a slot inflicts, by the share of each kind of damage in the skill: freeze s is a hold (Định thân), stun s a
  stun, knockdown or jolt, silence s a lost soul; defence-down s and defence lost are sunder (metal); burn s and burn's
  rate are the burn (fire); the chill slots are soak (water) and the slow is bind (wood); the shock slots are jolt
  (thunder).
- There is no ultimate and no rage (`docs/rules.md`, "Loadouts"). ABI 2 dropped their places: the old rage, the
  ultimate flags, rage stolen and the ultimate's slot. ABI 3 appended 305..1741, ABI 4 1741..1889, ABI 5 1889..2055. A bot
  of another ABI (1: 354 inputs, 2: 305, 3: 1741, 4: 1889) is refused.

A skill is known by its kind and its numbers, never its name, so a bot reads a skill the bar has just drawn the same
way as one it has seen a thousand times.

What it does not see: the sense, the resources besides qi and life, the skills in its pool that are not on its bar,
and the foe's supports. The two corners see the same kinds of numbers, so neither side knows more.

## What it can do: 29 acts

`ACTIONS` = 29, each at a fixed index:

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
| 9 + 3 x slot + aim | cast the skill in `slot` (0..6) with `aim` 0, 1 or 2 |
| 27 | duck held long (1 s; ABI 5) |
| 28 | guard held long (1 s; ABI 5) |

Where an aim lands:

| Skill kind | aim 0 | aim 1 | aim 2 |
|---|---|---|---|
| area | on the foe | 30 px past the foe | 30 px short of the foe |
| blink | 20 px on this side of the foe | 20 px past the foe | a full-reach blink backwards, away from the foe |
| any other | at the foe | (illegal) | (illegal) |

**Legal acts.** Each decision also comes with a mask of the acts that can happen now (`legal`):

- wait is always legal;
- while the fighter cannot act (in its own cast, or held by a status), nothing else is;
- jump whenever not flying with 8 qi or more (a jump costs 8 qi; a jump in mid-air does nothing), fly high whenever
  not flying with 8 qi or more (taking off costs a jump, from the ground or in the air: RULES 26), land only when
  flying;
- meditate only on the ground, not flying (RULES 26; it sits down only from there);
- a cast only from a filled slot whose skill is ready: cooled down, qi enough to pay,
  and, for a skill that deals damage, none of its last blows still playing out (`docs/rules.md`, "Skills");
  a silenced fighter casts only its attacks (the qi burst, if its loadout holds it: no spell), a sealed one cannot blink;
- aiming ahead or short only for areas and blinks.

An illegal act is played as a wait. An act that was legal when chosen but is no longer legal on the next tick (when
it is played) is dropped, and the fighter keeps doing what it was doing.

## When it decides

- The bot thinks every 0.12 s, jittered between 0.8x and 1.2x on its own seeded dice (`TEMPERAMENT.think`): at most
  about eight decisions a second.
- A decision comes only while the fighter can act: never during its own cast, while it sits down or gets up, a stagger, a freeze or shock, the
  crouch before a jump, or a duck's hold (0.4 s, or 1 s for a long one). After a round starts, the first decision comes 0.4-0.7 s in.
- What it decides on one tick is played on the next.
- An act lasts until the next one: walking carries on and `wait` stops it. Meditating is a skill with phases
  (`docs/rules.md`, "Meditation"): `meditate` sits down (0.4 s, no decision), sitting carries on through `meditate`
  and `wait`, and any other act gets up (0.4 s, no decision; that act is dropped). Its qi comes in 0.5 s blocks
  after a first whole second, and a blow on a meditating fighter does 1.5x.
- Hit-stop on the page pauses the whole duel, so it changes nothing for a bot.

These are the same for every bot and every style, and the same as the hand-written brain's, so no bot is faster than
another: the arena measures decisions, not reflexes.

## What a player submits

One **bot module** (`.wasm`): any model of any size within the budgets below, memory across a match, and its own way
of picking an act. Most bots start as an ONNX model, the policy exported from PyTorch (or any library): input float32
`[1, 2055]`, output the 29 acts' logits, float32 `[1, 29]`. One command builds it into a module with the `bots/onnx`
template ("Bring your own model" in `docs/training.md`):

```
cargo run -p arena-bot --example onnx -- mine.onnx mine.wasm
```

The template samples the act as the house bot does: illegal acts masked out, a softmax over the rest, read with the
seat's draw `u`. A bot that wants to always play its best act trains its logits sharp, or picks otherwise in its own
`arena_decide`.

## The bot module

A WebAssembly module of the ABI `arena-bot-5` (`crates/arena-bot`; the design and its reasons:
`docs/specs/2026-11-16-arena-wasm-bots-design.md`). It **imports nothing** and exports:

| Export | Signature | What |
|---|---|---|
| `memory` | memory | its linear memory |
| `arena_abi` | `() -> i32` | returns `5`; a module that returns anything else is refused (an ABI 4 module too: rebuild it on ABI 5, its model widened) |
| `arena_input` | `() -> i32` | the address of a buffer the arena writes before each decision: the 2055 inputs as `f32` LE, then the 29 acts' legal mask as bytes (0 or 1). Read once, when the instance is made |
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

The AI the site plays is an ONNX model like any player's: `crates/arena-duel/weights/policy.onnx` (`train/export.py`
writes it; 128 wide, two residual blocks of 256, a simplicial embedding, about 198K parameters). Natively it runs with
tract (`arena_bot::onnx`: the ladder, the tests, a trainer's sparring partner, `arena_duel.HOUSE` in Python); on the
site and in the tournament it is the module `bots/onnx` builds from it (`site/build.sh` writes it to
`site/dist/bots/policy.wasm`):

```
cargo run -p arena-bot --example onnx -- crates/arena-duel/weights/policy.onnx policy.wasm
```

The game on the page admits a dropped module the way a submission is admitted (`crates/arena-game/src/web.rs`) and
runs it with the browser's own WebAssembly, on the same fuel as the tournament's native host; the house module's native
fights are pinned by `crates/arena-bot/tests/house_fight.rs` (`goldens/house_fight.json`), and its bytes too
(`goldens/house_module.sha256`): the example builds the same module from any clone, so the module deployed is the one
pinned. Until a module is dropped in, the home page shows the arena server's live house matches; drop a `.wasm` into a
corner and it fights at once.
