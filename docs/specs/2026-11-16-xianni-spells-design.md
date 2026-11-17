# Renegade Immortal skills

## Intent

The arena's duel is a cultivation duel. This adds the techniques of the novel *Renegade Immortal* (Xian Ni, Er Gen) as skills in the two schools' pools: 72 skills, 32 for Thunder and 40 for Frost, on top of the 27 spells the duel had.

## How a technique becomes a skill

- A skill is a spell of the duel's existing kinds (melee, dash, blast, bolt, orb, area, field, heal, ward, barrier, blink) and flags (shock, chill, burn, freeze, silence, defence down, pull, seal, rage steal, reflect, counter, strikes). No new rule was needed, so a bot still reads a skill by its kind and numbers (`docs/bot.md`): the 354 inputs and the 30 acts are unchanged.
- The school follows the technique's nature: lightning, force, body, movement and swords are Thunder's; cold, restrictions, seals, souls, time and dreams are Frost's.
- Numbers sit in the range of the skills of the same kind (`data.rs`): cost 10 to 50 qi, a heavy blow 28 to 34 power against the ultimates' 31, long gathers for big blows, and the heavy dashes and blasts are unsafe on block (`data::FRAMES`).
- Evolving pairs (`data::evolves`) pair skills of one family. A skill is in at most one pair; a skill with no family partner (`earthescape`, `inchearth`, `truefalse` and a few more) has none.
- The effect of each skill is an effect of the pixfx registry that matches the technique (a sword ring for the Magic Arsenal, rune rings for restrictions, the night-sky effect for Remnant Night). A skill played once with `cast` uses a one-shot effect: the duel never stops a looping one.
- Each effect takes its school's colours through `data::tint`, except effects whose own colours are the point (the jade bagua mirror, the golden bell, the lotus, the night sky).

## The pool

A fight still draws 8 skills of its school (`POOL_SIZE`). With 43 Thunder and 51 Frost skills in the pools, `bar::fight_pool` now also guarantees that a payoff skill (palm, burst, gather; shatter, lotus, yellowspring) comes with a skill that lays the status it spends, because the draw no longer leaves out only 3 of 11.

## Technique to skill

| Technique | Skill | School | Kind |
|---|---|---|---|
| Fireball Technique | `fireball` | Thunder | bolt |
| Gravity Technique / Attractive Force | `gravity` | Thunder | area |
| Earth Split / Lands Collapse | `earthsplit` | Thunder | area |
| Finger of Annihilation | `finger` | Thunder | bolt |
| Ji Thunder | `jithunder` | Thunder | bolt |
| Thunder Gathering Seal | `gather` | Thunder | area |
| Ancient God Blood | `ancientgod` | Thunder | blast |
| Slash Luo Art (Zhanluo) | `slashluo` | Thunder | dash |
| Magic Arsenal | `arsenal` | Thunder | field |
| Shrinking the Earth into an Inch / Great Teleportation Restriction Art | `inchearth` | Thunder | blink |
| One Step Tread the Heaven | `onestep` | Thunder | dash |
| Mountain Collapse / Mountain Crumbles | `mountain` | Thunder | area |
| Heartbeat Thunder Sound | `heartbeat` | Thunder | blast |
| Light and Shadow Shield | `lightshield` | Thunder | barrier |
| Nine Revolutions Immortal Refining Art | `nineturn` | Thunder | blast |
| Ancient Clan Indestructible Body | `indestructible` | Thunder | heal |
| Ancient Protection / Ancestral Soul Protection | `ancientprotect` | Thunder | ward |
| Vermilion Bird Nine Mysteries Transformation | `vermilion` | Thunder | blast |
| Summon Rain | `summonrain` | Frost | field |
| Summon Wind | `summonwind` | Frost | blast |
| Beans to Soldiers | `beans` | Frost | field |
| Yin Moon Has Clear Sky / Dark Moon | `yinmoon` | Frost | area |
| Soul Searching Technique | `soulsearch` | Frost | blast |
| Spirit Transformation Technique | `spiritchange` | Frost | bolt |
| Body Fixation Art / Immobilization Technique / Stop Spell | `bodyfix` | Frost | area |
| Destruction Restriction | `restriction` | Frost | area |
| Eighteen Plum Blossom Restrictions | `plumblossom` | Frost | area |
| Life and Death Seal | `lifedeath` | Frost | area |
| Cause and Effect Seal | `karma` | Frost | barrier |
| Dream Return to Ancient Times | `dreamback` | Frost | ward |
| Dream Dao | `dreamdao` | Frost | blast |
| Flowing Moon | `flowingmoon` | Frost | heal |
| Remnant Night / Sundered Night | `remnantnight` | Frost | area |
| Heaven Flipping Seal | `heavenflip` | Frost | area |
| Third Eye | `thirdeye` | Frost | bolt |
| Seven-Colored Dao Spear | `sevencolor` | Frost | dash |
| World-Burning Ancient Umbrella | `umbrella` | Frost | area |
| Spirit-Devouring Body-Consuming Heaven-Severing Great Art / Devouring | `devour` | Frost | blast |
| Falling Star | `starfall` | Frost | area |
| Foundation Seizing Great Art | `seize` | Frost | bolt |
| Yellow Spring Finger / Underworld Finger | `yellowspring` | Frost | blast |
| Ancient Thunder Realm / Thunder Origin Technique | `ancientthunder` | Thunder | area |
| Demon Transformation Finger | `demonfinger` | Thunder | bolt |
| Kunji Whip / Cause and Effect Whip | `kunjiwhip` | Thunder | blast |
| Heavenly Tiger Flag | `tigerflag` | Thunder | bolt |
| Time Restriction | `timerestrict` | Frost | area |
| Life and Death Restriction | `lifedeathrestrict` | Frost | field |
| Billion Soul Banner / Soul Banner method | `soulbanner` | Frost | field |
| Spirit Servitude Seal | `servitude` | Frost | area |
| Seven-Colored Divine Void Nail | `voidnail` | Frost | bolt |
| Finger of Death / 100 Fingers of Death | `fingerdeath` | Thunder | area |
| Heavenly Chop (sword divine ability) | `heavenlychop` | Thunder | blast |
| Nine Mysterious Spell, Fire Escape | `fireescape` | Thunder | dash |
| Rise Three Realm Flame Origin | `flameorigin` | Thunder | area |
| War Spirit Print (Cloud Sea divine ability) | `warprint` | Thunder | blast |
| Earth Escape Technique | `earthescape` | Thunder | blink |
| Celestial Slaughter Art (All Seer spell) | `celestialslaughter` | Thunder | bolt |
| Ghost Summoning Technique | `ghostsummon` | Frost | field |
| Soul Vortex | `soulvortex` | Frost | area |
| Soul Refining Technique | `soulrefine` | Frost | heal |
| Star Rotation | `starrotation` | Frost | area |
| Merit Spirit (Ancient God spell) | `meritspirit` | Frost | ward |
| Body Formation (Celestial Emperor Qing Lin spell) | `bodyformation` | Frost | heal |
| True and False Domain / True and False Seal | `truefalse` | Frost | area |
| Ancient God Cauldron | `ancientcauldron` | Thunder | blink |
| Eight Extremes Dao | `eightextremes` | Thunder | area |
| Dao Art Fusion | `daofusion` | Thunder | blast |
| Dao Transformation Yellow Spring | `daoyellow` | Frost | field |
| Condensed Spring Soul | `springsoul` | Frost | bolt |
| Star Soul Extraction | `starsoul` | Frost | blast |
| Life Destruction Spirit Summoning | `spiritsummon` | Frost | area |
| Star Compass | `starcompass` | Frost | blink |

## Balance

Measured with `cargo run --release -p pixfx-duel --example ladder -- --seeds 150 --telemetry`, the shipped policy (trained before these skills existed):

| Match | Before | First draft of the skills | 50 skills | 64 skills | All 72 skills |
|---|---|---|---|---|---|
| balanced vs brain (bar 75%, 65% each school) | 95.9% | 93.1% | 92.0% (Thunder 93.3%, Frost 90.7%) | 94.1% (Thunder 95.6%, Frost 92.7%) | 91.4% (Thunder 89.1%, Frost 93.8%) |
| balanced vs first mind (bar 80%) | 95.4% | 93.0% | 93.4% | 93.0% | 90.3% |
| balanced vs aggressive script | 91.1% | 85.1% | 85.6% | 85.7% | 83.8% |
| balanced vs turtle script | 100.0% | 99.9% | 99.9% | 100.0% | 99.0% |
| mirror, Thunder's share (target 45-55%) | 49.1% | 40.4% | 49.1% | 45.1% (Frost 54.9%) | 46.0% (Frost 54.0%) |

The last column is the build in this repository, measured on 150 seeds. The first run of the 72 skills left Thunder at 44.9%; raising the damage of `eightextremes` (11) and `daofusion` (36) and then turning `ancientcauldron` and `starcompass` into real swaps (the policy was never shown a swap, so it rarely lands anything after one) leaves it at 46.0%. Thunder stays at the low end of the band: the next retrain is the lever.

The first draft favoured Frost: its new skills (`yellowspring`, `soulbanner`, `summonrain`, `sevencolor`) land and hurt, while Thunder's (`gather`, `gravity`, `earthsplit`, `mountain`) rarely landed, and a larger pool dilutes Thunder's older, stronger core more. The shipped numbers raise Thunder's weak new skills and trim those four of Frost's; no number of the original 27 spells changed.

## Techniques with a rule of their own, and those the duel approximates

Two techniques name a mechanic that the duel now has, as a flag on an existing kind; a bot still reads each skill by kind and numbers, so the 354 inputs and the 30 acts are unchanged:

| Technique | Skill | Rule |
|---|---|---|
| Ancient God Cauldron | `ancientcauldron` | `swap`: a blink that trades places with the foe wherever it stands |
| Star Compass | `starcompass` | `swap`, quicker and cheaper |
| Life Destruction Spirit Summoning | `spiritsummon` | `blood`: pays 20 life at the cast's start (never the last point); each point adds `BLOOD_POWER` (3%) to the blow |

Six more name a mechanic the duel does not have; each is a skill of the existing kinds that keeps its role:

| Technique | Skill | The mechanic it stands in for | What the skill does instead |
|---|---|---|---|
| Dao Art Fusion | `daofusion` | fuses abilities | one heavy, slow, shocking blast; the duel's evolving pairs (`data::evolves`) already fuse two skills into one at x1.5 |
| Eight Extremes Dao | `eightextremes` | eight Dao paths at once | four burning strikes in sequence |
| Dao Transformation Yellow Spring | `daoyellow` | an independent reincarnation cycle | a chilling field |
| Condensed Spring Soul | `springsoul` | a hundred million souls fused into one | a chilling dart |
| Star Soul Extraction | `starsoul` | pulls a planet's soul out | rage steal at close range |
| Ghost Summoning, Billion Soul Banner and the fields above | `ghostsummon`, `soulbanner`, `daoyellow` | summoned allies | a field the foe stands in: the duel's summoned thing |

An ally that fights beside the caster would change what a bot senses (`docs/bot.md`): that needs a retrain and a new season, and it is the owner's call.

## Techniques with no skill

The source (the Fandom page "Wang Lin/Techniques", below) lists more than the duel can show. Entries without a skill, and why:

| Entry | Why it has no skill |
|---|---|
| Furnace Becomes the World, Thunder Origin spell, Underworld Ascension Method, Ji Realm Spiritual Energy Refining, Refining Technique, Yin Energy Detection, Divine Path | cultivation methods and aids: they train or sense between fights, there is no action to cast |
| Disguising Technique, Memory Erasing Technique | out-of-fight uses; the duel's silence skill (`spiritchange`) carries the memory wipe |
| Specializations (restrictions, body tempering, the soul, killing intent), the Essences, True Bodies, the Main Body (27 Stars), Dao Source: Reincarnation | what a cultivator is, not an action; the schools' traits stand for them |
| Ancient God "Tactics" | named without any description |
| Magic treasures with no combat verb (Immortal Pointing Brush, Immortal Sealing Seal, Collection Pavilion, Heaven Defying Bead) | they carry other techniques; the ones with a verb are skills (`tigerflag`, `soulbanner`, `kunjiwhip`, `voidnail`) |

## Sources and coverage

Read in full: the Fandom page "Wang Lin/Techniques" (renegade-immortal-xian-ni.fandom.com, through its `api.php?action=parse&prop=wikitext`; the `/wiki/` URLs answer 402 and 403), the Baidu Baike pages for Wang Lin (techniques and treasures) and for *Renegade Immortal*, and the Wikipedia article (it names no technique). The Fandom page names each technique but gives few effects, so the effect of a skill follows the technique's name and role; where Baidu Baike describes one (Finger of Annihilation, Ghost-like arts, the restrictions, the Bai Fan arts) the skill follows that text. Not read: VS Battles (402) and the pages of single techniques on Fandom. Every combat entry of the Fandom page has a skill or a row above; names differ between sources, so the table gives each skill's source names.

## Not done

- **Icons.** The icon atlas (`site/art/skill-icons.sh`) is a generated sheet of 25. The new skills share the cell of the existing skill of their school that looks most like them (`site/assets/skill-icons.js`); drawing their own needs a new sheet.
- **A retrain.** The shipped policy was trained on the 27 spells. It still clears the entry bar on the new pools (the table above), but it was never shown the new skills; a retrain on a desktop (`train/README.md`) is how its play improves with them.
- **A season.** No season tag exists yet, so no bot is made stale. The first season freezes this table (`docs/tournament.md`).
