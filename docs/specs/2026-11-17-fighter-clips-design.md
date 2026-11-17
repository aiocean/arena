# Fighter clips: design (draft)

Status: draft, not built. Direction from the owner (2026-11-17): **more clips rather than fewer**, so the fight reads
alive. This spec lists every animation a painted fighter may have, what picks each one, and in what order to make
them. It changes no rule; the clips that would need one are marked and wait for the owner and a new season.

## What exists today

- A fighter's sheet must hold the 20 clips of `crates/arena-fighter/src/pick.rs` `CLIPS` (`sheet.rs` refuses a sheet
  missing one). The cultivator (`art/cultivator`) has them, plus `thrust`, which nothing picks yet.
- `pick.rs` maps the engine's 24 poses (`pixfx_fighter::pose::Pose`) onto those 20 clips, several poses sharing one
  clip by playing a part of it (`Fly`, `Land`, `Crouch` are parts of `jump`; `Fall` and `Down` of `knockdown`; `Sit`
  and `Kneel` of `getting-up`).
- What an `Appearance` already tells the page, beyond the pose: how the fighter gathers a cast (`Gather::Summon`,
  `Palm`, `Lunge`, `Prayer`, `Cross`), whether the blow comes from the air (`dive`), the wind-up's progress, how hard
  it was last struck (`hit_power`) and how long ago (`hit_t`), its speed (`vx`, `vy`), its facing, and `out`,
  `frozen`, `shocked`. Today only `Lunge` and `dive` change the clip; every other gather plays `cast`, every blow
  plays the same `hit`, and walking back plays `walk`.
- The spells (`crates/pixfx-duel/src/data.rs`) are mostly cast, not swung: 30 zones, 18 blasts, 14 bolts, 10 voids,
  5 dashes, 5 heals, 5 wards, 5 blinks, 4 barriers, 3 melee, 1 orb. A gesture set for casting matters more than a
  weapon set.

## Principles

1. **Optional clips fall back.** The 20 clips of `CLIPS` stay required. Every clip below is optional and names the
   clip it falls back to, so a character ships with the core and grows clip by clip, and the page never shows a hole.
   `pick` asks the sheet for the most specific clip and walks the fallback chain to the first one the sheet holds.
2. **A clip is picked from what the page already sees, wherever it can.** Tier 1 below needs only `pick.rs`. Tier 2
   needs the engine to tell the page more about a blow (visual only: no rule, no golden, no retrain). Tier 3 needs a
   rule.
3. **One main weapon per character, drawn into its sprite.** Fighting games bind the weapon to the character; the
   moveset is the character. A swappable weapon would need every body clip drawn empty-handed and the weapon attached
   per frame (the `attach-point` skill): possible later, not the default.
4. **What code does better is not drawn.** Cutting in two, crushing flat, burning to ash, a flying sword, an Ancient
   God phantom: one effect in code serves every character in every pose. A drawn clip would be per character and
   per pose.
5. **East only.** The duel is side-on; west is east mirrored. One direction per clip.

## The clips

Names are kebab-case like the existing ones. "Falls back to" is the clip shown when the sheet lacks this one.

### Moving

| Clip | Picked when | Falls back to | Tier |
|---|---|---|---|
| `walk-back` | `Walk` with `vx` against the facing | `walk` | 1 |
| `fly` | `Fly` (a loop, robes streaming) | `jump` (its apex) | 1 |
| `land` | `Land` | `jump` (its last part) | 1 |
| `crouch` | `Crouch` | `jump` (its crouch frame) | 1 |
| `dash-back` | a backward dash or blink away | `roll` | 2 |
| `blink` | a `blink` spell's cast (4 frames fading; the rest is effect) | `cast` | 2 |
| `sit`, `kneel` | `Sit`, `Kneel` | `getting-up` (its parts) | 1 |

### Guarding

| Clip | Picked when | Falls back to | Tier |
|---|---|---|---|
| `guard-hit` | `Guard` within a moment of a held blow (`hit_t` small) | `guard` | 1 |
| `guard-break` | the guard breaks (qi below 8) | `hit-heavy` | 2 |
| `guard-low` | a crouching guard | `guard` | 3 |

### Casting: the heart of a cultivation game

Each is one clip split in two like `cast` today: the wind-up plays its first half by progress (`Channel`), the
release the rest (`Cast`). The wind-up must read at a glance, because a blow in it is a COUNTER (`docs/rules.md`).

| Clip | Gesture | Picked when | Falls back to | Tier |
|---|---|---|---|---|
| `cast-summon` | both arms raised in a V | `Gather::Summon` (zones, voids, orbs) | `cast` | 1 |
| `cast-palm` | a palm drawn back, then thrust out | `Gather::Palm` (bolts, blasts) | `cast` | 1 |
| `cast-prayer` | hands joined before the chest, a hand seal | `Gather::Prayer` (heals, wards) | `cast` | 1 |
| `cast-cross` | arms crossed, then thrown open | `Gather::Cross` (barriers, blinks) | `cast` | 1 |
| `cast-point` | one finger pointed at the foe | a finger spell (`fingerdeath`, `demonfinger`, `voidnail`) | `cast-palm` | 2 |
| `cast-sky` | a hand raised to the sky | a blow that falls from above (`mountain`, `starfall`, `tribulation`) | `cast-summon` | 2 |
| `cast-ground` | a fist or foot slammed down | a blow from the ground (`earthsplit`, `quake`) | `cast-summon` | 2 |
| `cast-throw` | a treasure flung out or up | an orb, a banner, a bell | `cast-palm` | 2 |
| `channel-long` | a held, braced stance | a long wind-up (`daofusion`, an ultimate) | `cast` wind-up | 2 |
| `ultimate` | the character's own great gesture | an ultimate | `cast-summon` | 2 |
| `signature-<skill>` | one skill's own animation | that skill | its gather's clip | 2 |

### Striking (by weapon family)

A character has the family of its weapon. Blows from the air are the `dive` variant.

| Family | Clips | Falls back to |
|---|---|---|
| Unarmed (fist and foot) | `jab`, `punch`, `uppercut`, `palm-strike`, `kick-high`, `kick-mid`, `kick-low` (sweep), `roundhouse`, `air-kick`, `dive-kick` (diagonal, down and forward) | `slash`, `overhead` |
| Sword | `slash`, `overhead`, `thrust`, `slash-up`, `air-slash` | (core) |
| Saber, great sword | as the sword, slower, with a wind-up; `spin-slash` | `slash` |
| Spear | `thrust`, `thrust-flurry` (three quick), `sweep-long`, `spin-guard`, `dive-stab` | `slash` |
| Bow | `draw-bow` (holds at full draw while it gathers), `loose`, `air-shot`, `volley-up` | `cast-palm` |
| Banner (the soul banner) | `banner-wave` (souls stream out), `banner-plant` (a zone), `banner-strike` | `cast-summon` |
| Flying sword | the body points (`cast-point`); the sword is a sprite moved by code; `ride-sword` for flight | `cast-point`, `fly` |
| Treasure (bell, seal, umbrella) | `cast-throw`, then the treasure grows and falls (effect) | `cast-throw` |

Today the duel has 3 melee skills and no high, mid or low: a high kick and a sweep would only look different. They
become real in tier 3 (below).

### Being struck

| Clip | Picked when | Falls back to | Tier |
|---|---|---|---|
| `hit-light` | `Hurt` with a low `hit_power` (a flinch, 4 frames) | `hit` | 1 |
| `hit-heavy` | `Hurt` with `hit_power` ≥ 9 (the engine's own reel line) | `hit` | 1 |
| `knockdown-back` | `Fall` after a blow from the front: thrown onto the back | `knockdown` | 2 |
| `knockdown-front` | `Fall` after a blow from behind or a sweep: pitched onto the face | `knockdown` | 2 |
| `crumple` | a blow to the soul (a void, `kunjiwhip`) or a knockout: sags to the knees | `knockdown` | 2 |
| `down-front`, `getting-up-front` | lying on the face, and rising from it | `knockdown`, `getting-up` | 2 |
| `tech-roll` | rising with a roll | `roll` | 2 |
| `stunned` | stun (dazed sway) | `hit` | 2 |
| `pulled` | a pull (dragged toward its source) | `hit` | 2 |
| `held` | body fixation (rigid) | `frozen` | 2 |
| `crushed` | a blow from above (`mountain`, `gravity`): pressed flat; the flattening is a squash in code | `hit-heavy` | 2 |
| `hit-head`, `hit-gut`, `hit-legs` | where the blow lands | `hit` | 3 |

### The round's end

| Clip | Picked when | Falls back to | Tier |
|---|---|---|---|
| `win` | (core) each character's own, the one to make best | | core |
| `defeat` | (core) | | core |
| `intro` | the round's start: the character's entrance | `stance-idle` | 2 |
| `taunt` | after a knockout, before `win` | `win` | 2 |

### Done in code, not drawn

| Effect | How |
|---|---|
| Cut in two | the sprite cut along the blade's line; the halves fly apart and turn; blood or light at the cut |
| Crushed flat | the `crushed` pose squashed in height, then sprung back |
| Ash, mist, soul fading | the sprite dissolves pixel by pixel |
| Flying sword | a separate sprite moved by the cast |
| Dharma phantom (`ancientgod`) | the sprite scaled up behind the fighter, tinted |
| Frozen, shocked, burning | the effect layer over `frozen`, `shocked`, `hit` |

A knockout's finish (cut in two, crushed, ash) chosen from the killing spell is visual only (tier 2). A finishing
move a fighter must earn is a rule (tier 3).

## Tiers

| Tier | Needs | Clips | Rule change |
|---|---|---|---|
| Core | nothing (exists) | the 20 of `CLIPS` | no |
| 1 | `pick.rs` only, from what `Appearance` carries | about 12 | no |
| 2 | the engine to tell the page more about a blow (its kind, its skill, front or back, from above or below, the finish) | about 25 | no: visual only, no golden, no retrain |
| Weapon | the character's family | 5-10 per character | no |
| 3 | rules: high, mid and low blows, a crouching guard, sweeps and dive kicks that matter, earned finishers | about 10 | yes: `docs/rules.md`, the ladder, a retrain, a new season (owner) |

A full character is then **about 60 clips**: the core 20, tier 1, tier 2 and its weapon set. Tier 3 adds about 10.

## Making them

- PixelLab, per character: `animate_character` mode `v3` on the character's approved HD state (256x256, `art/README.md`),
  east only, a custom action text per clip, 4 to 8 frames (PixelLab's limit at 256) (`art/cultivator/src/metadata.json` holds the cultivator's texts: reuse their wording so the
  cast moves alike). Check each clip against the master (same face, crown, colours) before downloading.
- Hit reactions are short (4 frames, played fast) and read at once; casts are longer and their wind-up half must show the
  gesture building.
- Order: core first (the character can fight), then tier 1 (cheap, no engine change), then the weapon set, then
  tier 2 once the engine passes the blow's facts.
- Cost: at 256, v3 costs 1 generation a frame. 60 clips of about 6 frames is about 360 generations a character, so a
  month's 2000 makes the full set for about five characters. Make the core and tier 1 for the cast first, the rest as
  the budget allows.
- The sheet grows past one texture: 60 clips of 8 frames at a 256 px cell would be 2048 px wide and 15360 px tall,
  and even trimmed to the figure (about 240 px tall) the rows pass the 8192 px texture limit of many phones. A
  character's clips go on several sheets (pages), each under 4096 px a side, with the layout naming each clip's page;
  frames are trimmed and short clips share a row.

## Open decisions (owner)

1. Tier 3: whether high, mid and low become rules (a new season).
2. Whether a knockout's finish (cut in two, crushed, ash) shows blood or only light.
3. Whether a character may change weapons (principle 3).
