# Rules

How a fight works, for a bot author. The rules are `crates/arena-duel`; how much a blow deals, what it lays and what
a skill costs is the damage system, `crates/arena-calc`, whose design is
[`docs/specs/2026-11-17-damage-system-design.md`](specs/2026-11-17-damage-system-design.md) (Vietnamese; it is the
authority, and every number in it is a documented `pub const`). This page says how the fight uses it, and names
numbers only where they explain a mechanic. When a number here and the code disagree, the code is right.

## A fight

- Two fighters on a flat arena with walls, seat 0 on the left and seat 1 on the right. They walk, jump, duck, guard,
  fly and meditate, and cast skills. There are no classes: both start from the same base (3000 life, a 600 energy
  shield, 100 sense, 100 qi, 400 might, 600 armour, no resistances; design 1). Seat 0 is drawn as Vương Lâm in
  Thunder's colours and seat 1 as the Seven-Colored Daoist in Frost's, and that is all the two looks are.
- A round ends when one fighter's life reaches 0 (unless a keystone gives it a spare life). A round that passes the
  time limit is **cut**. In training a cut round counts as lost for the learner, so stalling never pays. In the
  ladder a cut ends that fight: it shows in the `cut` column, and its unplayed rounds count neither way.
- A bot module that traps or runs out of fuel **gives up the round** it happened in (`docs/bot.md`).
- The simulation runs at 60 ticks a second (`duel::TICK`). A seed decides every roll, so the same seed and the same
  two bots give the same fight on every machine.

## Loadouts

Each fighter fights with a **loadout** (design 2, `arena_calc::loadout`): exactly 8 actives from the shared pool of
168 skills, up to 3 supports on each (12 in all, matching their tags), up to 2 keystones and an optional root
element, within a budget of 50 points. `loadout::check` is the one judge of a legal loadout.

- A fight draws a legal loadout for each seat at random from its seed, on dice of their own (they never move the
  fight's rolls). A tool can give both loadouts instead (`Duel::with_loadouts`). A bot does not choose its loadout
  yet (design 13, 16).
- The three basics are always at hand: slash, dive and blink.
- The old schools' traits are keystones any loadout may take: Bất Khuất (unyielding: once a round a killing blow
  leaves 1 life) and Sát Ý (fury: +4% damage for each blow landed, 5 stacks a round).
- **There is no ultimate** (owner, 2026-11-18): no ultimate slot, no rage. The former ultimates (Thiên Kiếp
  `tribulation`, Tàn Dạ `remnantdusk`, Tô Kiếp `sukiep`, Đạo Pháp Dung Hợp `daofusion`) are **top-tier skills (đỉnh
  cấp)**: actives like any other, of tier S and rank Tín Niệm (Faith, the top of the ladder: a Faith blow passes
  every shield and wipes what stands on the field), held by "a blow plays out" like any blow. They are priced as the
  top tier: a heavy blow, a 20 s cooldown and 60 qi, and **a loadout holds at most one skill of rank Tín Niệm**
  (`loadout::FAITH_MAX`; `check` refuses a second, the draw never takes one).

## Skills

A skill is a gem of the pool (`arena_calc::gem`): its tags, its kinds of damage, its base, costs and timings. It
moves as the motion it borrows (`Gem::spell`, the duel's `data::motions`: melee, dive, dash, blast, bolt, seeker,
area, field, heal, ward, barrier, blink), with two timings built for the cast (supports and keystones change them):

- **windup**: from the cast until the blow comes out;
- **recover**: after the blow, before the fighter can act again.

Its role comes first: an intent (aura), a domain, a curse with no blow, a buff (stance, blessing), or a trap, mine,
totem or minion (its tag, or a support's), each does what the design says; anything else moves as its motion.

**A blow plays out before the next.** As in a fighting game, a skill that deals damage is over only when what it
sent is over: while a fighter's bolt still flies, its seeker still circles, its field still turns, its spray still
pours, its area's strikes still fall or its skill's repeats are still due, it casts no other such skill (its slots
show not ready). It can still move, guard, jump, fly and meditate, and a skill with a role that tends it or works on
its foe (curse, aura, blessing, movement, guard, recovery, stance: `arena_calc::gem::UNBOUND`, `Gem::binds`) it casts
whenever it is free, even one that deals damage. What it did not cast by hand never holds it: a trap's, mine's,
totem's or minion's blows, a triggered cast, a stored force; nor does a bolt a mirror sent back. It strikes again
the tick after its last blow ends.

Reaches and places are in the arena's pixels (`UNIT` = 4 of them to one of the world's); the damage system's unit,
T, is a fighter's height, 240 of the arena's pixels.

## The bar

A fighter's three bar slots show one of its loadout's actives each (only those cast by hand: a skill a trigger
casts never shows). A cast empties its slot, which refills from the other actives after 1.5 to 2.5 s; a skill ready
and uncast for 6 s leaves for another (`STALE`). The bar's old tiers, traits, lucky casts and evolving pairs are off
(design 2). So a bot has six castable slots: the three basics and the three bar slots (the policy's seventh slot,
the old ultimate's, is always empty until the next bot ABI drops it).

A skill's cooldown is its **stored uses** (`arena_calc::charge`): one, or more with a support that stores them; a
cast spends one and they come back one at a time.

## Resources

- **Qi** pays for most casts and for flying (3 a second, and it stops the regen). It refills on its own and fast
  while meditating. **Life**, **lifespan**, **killing intent**, **souls**, **sense** and **stars** pay for some
  skills (design 1, 10). Souls come from soul damage dealt (one each 100), from kills, and from each minion that dies
  within 4 T.

## A blow

Every blow is built for the moment (`build::build_skill`) and resolved in the damage system's twelve steps (design 5,
`hit::resolve_with`), with four rolls from the fight's dice in a fixed order: evasion, crit, double damage, star.
Ten kinds of damage, each with its ailment (design 4): bleed or daze (force), sunder (metal), bind (wood), soak
(water), burn (fire), heavy (earth), jolt (thunder), glare (light), wither (yin), dread (soul). Burns and bleeds are
damage over time; the rest stack and do something when full. The energy shield takes most kinds first, yin goes
straight to life, soul to the sense. Nothing chills or freezes.

**Controls** land on the body: a stun, a hold (Định thân), a jolt or a knockdown holds the fighter where it stands and
breaks a cast it gathers; a root stops walking and blinking; a lost soul (thất hồn) silences, so only attacks
answer. Grit shortens each hard control after the last (design 5.2).

Supports' **triggers** (cast on crit, when hurt, when the shield breaks...) cast at once at the foe, through the
damage system's event queue.

## Bodies and where blows land

All lengths are the arena's pixels (the page's world pixels times 4). Blows land as in a fighting game: on frame data
tied to the fighters' clips (`arena_duel::frames`; `art/README.md`, "Frame data and the sprite").

- **Hurtboxes follow the frame a fighter shows.** Every frame of every clip of each look carries one to three
  hurtboxes measured on its sprite: the head and torso, the legs, and on a clip that strikes or casts the arm held out
  ahead (`frames_data`, generated by `cargo run -p arena-game --example frame_boxes` after a sheet changes, never read
  from a picture at run time). The rules pick the frame a fighter shows as the game draws it (`pick`, from its pose,
  how long it has held it and the cast it gathers), so a fighter crouched is hit low, one in the air where its body is,
  one striking on its arm. A blow, a bolt or a shot touches a fighter when it touches one of those boxes
  (`shot::overlaps`; a bolt's flight sees each box as a column of round bodies, one target however many).
- **A strike's active frame is where the picture strikes.** A blade or casting clip's active frame is the one whose
  blade or hand reaches farthest ahead (the generator finds it); the wind-up plays the clip up to it, the recovery the
  rest. The rules strike, and a bolt leaves, on the tick the wind-up ends, so on that frame.
- **A melee strike** is its clip's hitbox on the active frame: as high as the drawn hand and blade, as long as the
  drawing or the skill's reach, whichever is farther, ahead of the striker (`unit::strike_box`). A clip drawn with
  nothing held out ahead (the Frost slash, drawn striking behind) strikes its own box: from the striker's middle to its
  reach ahead, from its feet to its shoulders and a little more. **A blast** strikes a box from the caster's middle to
  its reach ahead, as tall as the foe. Each is tested once, on that tick, against the foe's hurtboxes
  (`unit::strike_hits`). A foe behind the striker is not struck.
- **A bolt leaves the casting hand** as drawn on the cast clip's active frame (`frames::cast_hand`); a clip with no
  hand out ahead, or a unit, casts from its shoulders, 152 up and 32 ahead.
- The body's own box (48 wide, 228 tall, 160 ducking: `data::HD_BODY`, `HD_DUCK`) stays for walking, walls and two
  bodies kept apart, and stands in for a frame with no data.
- Zones and fields strike every body in their area.
- An effect the rules play once and keep no hold of (a buff's or a curse's show, a heal, a blink, a plain blast) is
  stopped as it starts when it loops, so it shows a moment and fades (`act::cast_once`).

## Defence

- **Guard** stops only melee, blasts, dashes, bolts and orbs, and only facing the blow; zones and fields fall past
  it. Each held blow costs 8 qi (below 8 qi the guard breaks) and lets 20% through, through the damage system; a
  heavier blow is likelier to break through whole. Casting drops the guard.
- **Barriers** (bell, mirror) turn every blow aside for a moment; the bell answers a close blow, the
  mirror sends a bolt back. **Wards** hold 1-3 charges for 6 s, each erasing one whole blow; a ward and a heal wash
  off the ailments.
- **Evasion** is the damage system's roll; it never evades an instant skill.
- Two blades meeting **clash**.

## Projectiles, areas and the field

- A bolt flies as the damage system's projectile (`arena_calc::projectile::Flight`): it pierces, forks and chains
  among the foe and the foe's minions, totems, traps and mines, and extra projectiles fan out from the cast.
- Knockback, run speed and dash and blink reach come from `arena_calc::motion` (bind slows, heavy shortens dashes).
- **Traps, mines, totems and minions** are bodies on the field the foe can strike and destroy (design 17.8): a trap
  springs on the foe that steps on it, a mine goes off when the foe stands in its blast (its owner sets it off: a
  seat has no act for it yet), a totem casts its skill on its cadence, a minion walks at the foe and strikes it.

## Frame data

The design is `docs/specs/2026-11-16-duel-frames-design.md`; the numbers are `data::FRAMES`.

- **A blow during a windup breaks the cast**: the cast is lost.
- **Counter:** a blow on a fighter in its windup does 1.2x (the damage system's step 10) and staggers 0.10 s longer.
- **Punish:** a blow on a fighter in its recover does 1.2x and staggers 0.20 s longer.
- **Unsafe on block:** dive, step, palm, burst, lotus and the heavy dashes and blasts recover 0.15 s longer when their
  blow is guarded.
- A field's strike (void) gives neither a counter nor a punish.

## Styles

Each fighter is drawn one of three styles, aggressive, balanced or defensive (`style.rs`), on dice of their own. The
style is only a tag a bot reads (`docs/bot.md`): it changes no rule and no reward.
