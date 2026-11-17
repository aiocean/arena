# Rules

How a fight works, for a bot author. The rules are `crates/pixfx-duel`; every number lives in
`crates/pixfx-duel/src/data.rs` and is pinned by `tests/fixtures/js_numbers.json`, so this doc names numbers only
where they explain a mechanic. When a number here and `data.rs` disagree, `data.rs` is right.

## A fight

- Two fighters on a flat arena with walls: Thunder in the left corner (0), Frost in the right (1). They walk, jump,
  duck, guard, fly and meditate, and cast skills.
- A round ends when one fighter's hp reaches 0. A round that passes the time limit is **cut**. In training a cut
  round counts as lost for the learner, so stalling never pays. In the ladder a cut ends that fight: it shows in the
  `cut` column, and its unplayed rounds count neither way.
- The simulation runs at 60 ticks a second (`duel::TICK`). A seed decides every roll, so the same seed and the same
  two bots give the same fight on every machine.

## The two schools

| | Thunder | Frost |
|---|---|---|
| Stats | less hp and attack (2500 hp, 345 atk) | more hp and attack (3300 hp, 520 atk) |
| Traits | `fury`: each blow it lands raises its attack 4%, up to 5 stacks for the round; `unyielding`: once a round, a killing blow leaves it on its last sliver of life | `attuned`: every second paid spell it casts earns a rage point |
| Basics (always in reach) | slash, dive, blink | slash, dive, blink |
| Pool (the bar draws from it) | 40 skills: bolt, palm, step, seal, orb, bell, heal, spark, burst, quake, hex, and the Renegade Immortal skills below | 46 skills: lotus, shatter, icicle, frostfire, collapse, shard, jet, flurry, void, mirror, ward, and the Renegade Immortal skills below |
| Ultimate (full rage) | tribulation: three thunderclaps fall one after another where the foe will be | prison: a great collapse drags the foe in from far off and freezes it |

Frost's bigger stats are balanced by Thunder's faster, more varied kit. With both sides played by the same trained
AI, Thunder wins 49.1% of the mirror over 150 seeds (the target is 45-55%).

## Skills

Each skill has a kind (melee, dive, dash, blast, bolt, seeker, area, field, heal, ward, barrier, blink), a qi cost, a
cooldown, a reach, a damage, and two timings:

- **windup**: from the cast until the blow comes out;
- **recover**: after the blow, before the fighter can act again.

Areas and blinks can be aimed (`docs/bot.md` gives where each aim lands). Everything else aims at the foe.

## The bar

A fighter does not hold its whole pool. Each fight draws 8 of its school's pool (`POOL_SIZE`; always one skill of the school's signature kind, one that pays off a status, and a skill that lays that status), and three
bar slots show one skill each. A cast empties its slot, which refills from the pool after 1.5 to 2.5 s. A skill off
cooldown and uncast for 6 s leaves for another draw (`STALE`), whether or not there is qi to pay for it.

- **Tiers and traits.** Each draw rolls a tier (common, rare, epic; legendary only through duplicates) and traits
  (swift, heavy, piercing, lingering, cheap) that change its power, cost and timings (`data.rs`, `bar.rs`).
- **Evolving pairs.** Casting one skill of a pair while its partner is on the bar refills that slot at once with the
  partner evolved (x1.5 power). Thunder: bolt + seal, palm + step, quake + burst, fireball + vermilion, jithunder + finger, gravity + earthsplit, gather + heartbeat, slashluo + onestep, mountain + ancientgod, ancientthunder + tigerflag, demonfinger + kunjiwhip, fireescape + flameorigin, warprint + heavenlychop, fingerdeath + celestialslaughter. Frost: shard + icicle, shatter + flurry, lotus + jet, summonrain + summonwind, yinmoon + remnantnight, bodyfix + lifedeath, soulsearch + dreamdao, thirdeye + sevencolor, restriction + plumblossom, starfall + heavenflip, yellowspring + devour, timerestrict + lifedeathrestrict, soulbanner + servitude, voidnail + seize, ghostsummon + soulvortex, soulrefine + bodyformation, starrotation + meritspirit.
- **Lucky casts.** A cast sometimes rolls lucky (8%): doubled, free, or cast again (`data.rs`).

So a bot has seven castable slots: the three basics, the three bar slots and the ultimate.

## Qi and rage

- **Qi** pays for casts and for flying. It refills slowly on its own and fast while meditating (`QI_REGEN`,
  `MEDITATE_REGEN`); meditating goes on until the fighter does something else.
- **Rage** is a small whole-number pool: a point for each blow landed (ultimates earn none, and Frost's spells earn
  none on hit: its blade does, and `attuned` pays a point every second paid spell). When it is full, the ultimate is
  castable, and casting it spends it. Rage starts empty each round.

## Statuses

What a hit can leave behind (`data.rs` and `status.rs`): **chill** (stacks that slow walking; the last one freezes),
**freeze**, **shock** (stacks; a shocked fighter twitches, held and hurt), **burn** (a share of the caster's attack
every tick, ignoring defence), **stun**, **silence**, **seal**, **defence down**, a **pull**, and rage stolen. A bot
sees, for each of its slots, what that skill inflicts (`docs/bot.md`).

**Grit:** hard controls shorten as a fighter's grit builds, and from grit 2 a freeze is only a chill stack
(`data.rs`).

## Defence

- **Guard** stops only melee, blasts, dashes, bolts and orbs, and only facing the blow; zones and fields fall past
  it. Each held blow costs 8 qi (below 8 qi the guard breaks) and lets 20% through as chip damage, and a heavier blow
  is likelier to get through (`act.rs`, `data.rs`). Casting drops the guard.
- **Barriers** (bell, mirror) turn every blow aside for a moment; **wards** (ward, hex) hold 1-3 charges for 6 s,
  each erasing one blow; **dodge** is a stat roll.
- Two blades meeting **clash**.

## Frame data

The design is `docs/specs/2026-11-16-duel-frames-design.md`; the numbers are `data::FRAMES`.

- **A blow during a windup breaks the cast**: the cast is lost.
- **Counter:** a blow on a fighter in its windup does 1.2x and staggers 0.10 s longer.
- **Punish:** a blow on a fighter in its recover does 1.2x and staggers 0.20 s longer.
- **Unsafe on block:** dive, step, palm, burst, lotus and the heavy dashes and blasts (ancientgod, slashluo, onestep, nineturn, sevencolor, heavenlychop, fireescape, warprint) recover 0.15 s longer when their blow is guarded.
- **Ultimate opening:** an ultimate cannot be hit in the first 0.3 s of its windup; a blow passes through as a dodge.
- A field's strike (void) gives neither a counter nor a punish.

A slow recover is an opening, and a bot sees every phase and its seconds left (`docs/bot.md`), so it can learn to
punish.

## Styles

Each fighter is drawn one of three styles, aggressive, balanced or defensive (`style.rs`), on dice of their own, so
the style never moves a fight's rolls. The style is only a tag a bot reads (`docs/bot.md`): it changes no rule, no
reward and not the hand-written brain. A bot may play it or ignore it; the shipped AI was trained in all three.

## The Renegade Immortal skills

64 skills of the Thunder and Frost pools are techniques of the novel *Renegade Immortal* (Xian Ni). Each is a spell of the kinds above, read by a bot by its kind and numbers, never its name (`docs/bot.md`); `docs/specs/2026-11-16-xianni-spells-design.md` says which technique each one is and which techniques have no skill.

**Thunder**

| Skill | Kind | Qi | Does |
|---|---|---|---|
| Fireball (`fireball`) | bolt | 14 | A ball of fire that flies at the foe and sets it burning. |
| Gravity Technique (`gravity`) | area | 22 | Pulls the foe in toward a point, then crushes it there. |
| Earth Split (`earthsplit`) | area | 28 | The ground under the foe splits and blows up, breaking its defence. Only hits on the ground. |
| Finger of Annihilation (`finger`) | bolt | 28 | A thin, fast beam from one finger that hits hard. |
| Ji Thunder (`jithunder`) | bolt | 24 | A heavy bolt of the Ji Realm that shocks. |
| Thunder Gathering Seal (`gather`) | area | 36 | Gathers thunder under the foe and releases it in one burst: shocks, and hits harder per shock stack on the foe. |
| Ancient God Blood (`ancientgod`) | blast | 36 | The body swells into an Ancient God phantom for one heavy blow at close range. |
| Slash Luo (`slashluo`) | dash | 30 | A sword form that cuts through space: dashes through the foe along a long line. |
| Magic Arsenal (`arsenal`) | field | 48 | A ring of flying swords turns over the foe and cuts whoever stands under it, then strikes. |
| Shrinking the Earth (`inchearth`) | blink | 26 | A long blink: the ground folds and the caster steps out far away. |
| One Step to Heaven (`onestep`) | dash | 30 | One stride that carries the caster through the foe, a long way, cutting it on the way. |
| Mountain Collapse (`mountain`) | area | 44 | A peak falls on the foe: a huge blow after a long mark. Only hits on the ground. |
| Heartbeat Thunder Sound (`heartbeat`) | blast | 20 | A thunderclap at the heart, at close range: shocks. |
| Light and Shadow Shield (`lightshield`) | barrier | 20 | A dome of gathered light that sends the next bolt or orb back at its caster. |
| Nine Revolutions Refining (`nineturn`) | blast | 40 | Spirals power inside the body, then releases it in one huge blast at close range. Long to gather. |
| Indestructible Body (`indestructible`) | heal | 40 | Recovery outpaces destruction: heals and washes off burn, silence, defence down, chill and shock. |
| Ancient Protection (`ancientprotect`) | ward | 42 | A golden bell of ancestral souls: 1 to 3 charges, each stops a whole blow. Cleanses. |
| Vermilion Bird Transformation (`vermilion`) | blast | 30 | A burning bird of fire sweeps out of the palm at close range and sets the foe burning. |
| Ancient Thunder Realm (`ancientthunder`) | area | 38 | An ancient thunder dragon falls twice on the foe, one strike after another, and shocks it. |
| Demon Transformation Finger (`demonfinger`) | bolt | 22 | A finger of demonic qi that silences what it hits. |
| Kunji Whip (`kunjiwhip`) | blast | 26 | A whip that strikes the soul directly at middle reach: silences, and shocks. |
| Heavenly Tiger Flag (`tigerflag`) | bolt | 26 | A heavenly tiger leaps from the flag in a straight line and mauls what it meets. |
| Finger of Death (`fingerdeath`) | area | 34 | Three fingers of death fall one after another where the foe will be. |
| Heavenly Chop (`heavenlychop`) | blast | 34 | A sword cut that splits heaven at close range: a heavy blow. |
| Fire Escape (`fireescape`) | dash | 28 | Turns to flame and dashes through the foe, setting it burning. |
| Rise Three Realm Flame Origin (`flameorigin`) | area | 34 | The origin of flame rises under the foe and burns it. |
| War Spirit Print (`warprint`) | blast | 32 | A seal of war stamped on the foe at close range: a heavy blow that breaks its defence. |
| Earth Escape (`earthescape`) | blink | 14 | Sinks into the ground and comes out a few steps away. |
| Celestial Slaughter Art (`celestialslaughter`) | bolt | 36 | A bolt of slaughter intent, slow to gather and heavy. |

**Frost**

| Skill | Kind | Qi | Does |
|---|---|---|---|
| Summon Rain (`summonrain`) | field | 30 | Rain falls on the foe for two seconds, chilling whoever stands in it. |
| Summon Wind (`summonwind`) | blast | 26 | A black wind-dragon exhales yin wind at the foe at middle reach: chills. |
| Beans to Soldiers (`beans`) | field | 36 | Souls of the slain rise as soldiers round the foe, dragging it in and cutting it while it stands there. |
| Yin Moon Clear Sky (`yinmoon`) | area | 32 | Marks the foe with a moon sign, then cracks break open: breaks its defence. |
| Soul Searching (`soulsearch`) | blast | 28 | Pulls at the foe's soul at close range: silences it and steals a point of rage. |
| Spirit Transformation (`spiritchange`) | bolt | 20 | A dart that wipes the foe's memory of its spells: silences. |
| Body Fixation (`bodyfix`) | area | 36 | A seal under the foe that holds its body in place: a long freeze, little damage. |
| Destruction Restriction (`restriction`) | area | 40 | Rings of runes write themselves round the foe, locking blinking until they close, then crush. Only hits on the ground. |
| Eighteen Plum Blossom Restrictions (`plumblossom`) | area | 34 | A self-running restriction of rings round the foe that breaks its defence when it closes. |
| Life and Death Seal (`lifedeath`) | area | 36 | Left hand life, right hand death: a seal that closes on the foe and freezes it briefly. |
| Cause and Effect Seal (`karma`) | barrier | 22 | A dome that answers the next close blow with a stunning counter. |
| Dream Return to Ancient Times (`dreamback`) | ward | 44 | Runes close round the caster: 1 to 3 charges, each stops a whole blow. Cleanses. |
| Dream Dao (`dreamdao`) | blast | 26 | Drags the foe into a dream at middle reach: silences it. |
| Flowing Moon (`flowingmoon`) | heal | 38 | Turns time back on the caster: heals and washes off burn, silence, defence down, chill and shock. |
| Remnant Night (`remnantnight`) | area | 46 | The night sky cracks over the foe and falls: a heavy blow that silences. Slow to land; only hits on the ground. |
| Heaven Flipping Seal (`heavenflip`) | area | 50 | A giant seal overturns heaven onto the foe: a huge blow that breaks its defence. |
| Third Eye (`thirdeye`) | bolt | 20 | A beam from the third eye that breaks the defence of whatever it hits. |
| Seven-Colored Dao Spear (`sevencolor`) | dash | 30 | A spear of seven-coloured light that carries the caster through the foe. |
| World-Burning Umbrella (`umbrella`) | area | 34 | An ancient umbrella sweeps fire over the foe: burns. |
| Spirit-Devouring Art (`devour`) | blast | 26 | Devours the foe at close range: steals a point of rage. |
| Falling Star (`starfall`) | area | 40 | Two stars fall one after another where the foe will be. |
| Foundation Seizing (`seize`) | bolt | 18 | A dart that takes a point of the foe's rage. |
| Yellow Spring Finger (`yellowspring`) | blast | 24 | A finger of the underworld's cold at middle reach: chills, and hits harder per chill stack on the foe. |
| Time Restriction (`timerestrict`) | area | 38 | A restriction of time round the foe: locks blinking while it runs, then closes and holds the foe. Only hits on the ground. |
| Life and Death Restriction (`lifedeathrestrict`) | field | 40 | Turns the vitality of whoever stands in it into death energy: a cloud that burns for three seconds. |
| Soul Banner (`soulbanner`) | field | 44 | Spirits swirl in a vortex over the foe, dragging it in and cutting it while it stands there. |
| Spirit Servitude Seal (`servitude`) | area | 34 | A seal under the foe that drags it in, then closes on it: silences. |
| Seven-Colored Divine Void Nail (`voidnail`) | bolt | 34 | A nail of void driven at the foe at great speed: breaks its defence. |
| Ghost Summoning (`ghostsummon`) | field | 34 | Ghosts rise round the foe and drag it in while they cut it. |
| Soul Vortex (`soulvortex`) | area | 30 | A vortex of souls pulls the foe in, then bursts. |
| Soul Refining (`soulrefine`) | heal | 28 | Refines a soul into life: heals and washes off burn, silence, defence down, chill and shock. |
| Star Rotation (`starrotation`) | area | 38 | Three stars turn down on the foe one after another where it will be. |
| Merit Spirit (`meritspirit`) | ward | 42 | A spirit of merit stands over the caster: 1 to 3 charges, each stops a whole blow. Cleanses. |
| Body Formation (`bodyformation`) | heal | 34 | Forms the body anew: heals and washes off burn, silence, defence down, chill and shock. |
| True and False Seal (`truefalse`) | area | 36 | Open eyes are truth, closed eyes falsehood: a seal on the foe that silences it and breaks its defence. |
