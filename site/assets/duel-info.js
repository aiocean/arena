// What the duel's tooltips say: each skill's name and what it does, each trait and buff, and the tooltip for a slot
// of a fighter's skill bar or a buff under its bars (lines of plain text; the page lays them out).
import { RAGE, SPELLS, TRAITS, skillBar } from "./duel.js";

/** Each skill's name and what it does, in a line. */
export const SPELL_INFO = {
  slash: { name: "Sword Slash", does: "A cut at close range. Free, and always at hand." },
  dive: { name: "Falling Sword", does: "Drops onto the foe sword first; cracks the ground if it lands." },
  blink: { name: "Blink", does: "Vanishes and reappears a few steps away." },
  bolt: { name: "Thunder Bolt", does: "A bolt of lightning that flies at the foe and shocks it." },
  palm: { name: "Thunder Palm", does: "A thunderclap from the open palm at arm's length: shocks, steals rage, and hits harder per shock stack." },
  step: { name: "Lightning Step", does: "Dashes through the foe, cutting it on the way." },
  seal: { name: "Thunder Seal", does: "Marks the ground under the foe, then strikes it with lightning that shocks." },
  orb: { name: "Ball Lightning", does: "A slow orb that seeks the foe, burning and shocking it." },
  bell: { name: "Golden Bell", does: "A dome that answers the next close blow with a stunning counter." },
  heal: { name: "Healing Light", does: "Heals and washes off burn, silence, defence down, chill and shock." },
  spark: { name: "Spark", does: "A quick, cheap bolt that shocks." },
  burst: { name: "Thunder Burst", does: "A blast at close range that hits harder per shock stack on the foe." },
  quake: { name: "Quake", does: "The ground under the foe cracks and blows up. Only hits on the ground." },
  hex: { name: "Hex Shield", does: "A dome of hex cells: 1 to 3 charges, each stops a whole blow. Cleanses." },
  tribulation: { name: "Heavenly Tribulation", does: "Ultimate: three thunderclaps fall one after another where the foe will be." },
  lotus: { name: "Ice Lotus", does: "A lotus of ice shards at close range that breaks the foe's defence, harder per chill stack." },
  shatter: { name: "Shatter", does: "A seal under the foe that bursts into cold and freezes it, harder per chill stack." },
  icicle: { name: "Icicle Fall", does: "An icicle drops on the foe, freezing it and breaking its defence. Only hits on the ground." },
  collapse: { name: "Collapse", does: "Space folds in: drags the foe in, then pops, silencing it." },
  frostfire: { name: "Frostfire", does: "A pale fire under the foe that burns and chills whoever stands in it." },
  void: { name: "Void Orb", does: "A dark orb that drags the foe in and burns it while it stands under it." },
  mirror: { name: "Ice Mirror", does: "A dome that sends the next bolt or orb back at its caster." },
  ward: { name: "Feather Ward", does: "1 to 3 charges, each stops a whole blow. Cleanses." },
  shard: { name: "Ice Shard", does: "A quick, cheap shard that chills." },
  jet: { name: "Water Jet", does: "A jet of freezing water at the foe that chills." },
  flurry: { name: "Flurry", does: "A short snowstorm on the foe that chills it while it stays." },
  prison: { name: "Ice Prison", does: "Ultimate: drags the foe in from far off, freezes and silences it, and locks blinking." },
  // Renegade Immortal (docs/specs/2026-11-16-xianni-spells-design.md)
  fireball: { name: "Fireball", does: "A ball of fire that flies at the foe and sets it burning." },
  gravity: { name: "Gravity Technique", does: "Pulls the foe in toward a point, then crushes it there." },
  earthsplit: { name: "Earth Split", does: "The ground under the foe splits and blows up, breaking its defence. Only hits on the ground." },
  finger: { name: "Finger of Annihilation", does: "A thin, fast beam from one finger that hits hard." },
  jithunder: { name: "Ji Thunder", does: "A heavy bolt of the Ji Realm that shocks." },
  gather: { name: "Thunder Gathering Seal", does: "Gathers thunder under the foe and releases it in one burst: shocks, and hits harder per shock stack on the foe." },
  ancientgod: { name: "Ancient God Blood", does: "The body swells into an Ancient God phantom for one heavy blow at close range." },
  slashluo: { name: "Slash Luo", does: "A sword form that cuts through space: dashes through the foe along a long line." },
  arsenal: { name: "Magic Arsenal", does: "A ring of flying swords turns over the foe and cuts whoever stands under it, then strikes." },
  inchearth: { name: "Shrinking the Earth", does: "A long blink: the ground folds and the caster steps out far away." },
  onestep: { name: "One Step to Heaven", does: "One stride that carries the caster through the foe, a long way, cutting it on the way." },
  mountain: { name: "Mountain Collapse", does: "A peak falls on the foe: a huge blow after a long mark. Only hits on the ground." },
  heartbeat: { name: "Heartbeat Thunder Sound", does: "A thunderclap at the heart, at close range: shocks." },
  lightshield: { name: "Light and Shadow Shield", does: "A dome of gathered light that sends the next bolt or orb back at its caster." },
  nineturn: { name: "Nine Revolutions Refining", does: "Spirals power inside the body, then releases it in one huge blast at close range. Long to gather." },
  indestructible: { name: "Indestructible Body", does: "Recovery outpaces destruction: heals and washes off burn, silence, defence down, chill and shock." },
  ancientprotect: { name: "Ancient Protection", does: "A golden bell of ancestral souls: 1 to 3 charges, each stops a whole blow. Cleanses." },
  vermilion: { name: "Vermilion Bird Transformation", does: "A burning bird of fire sweeps out of the palm at close range and sets the foe burning." },
  summonrain: { name: "Summon Rain", does: "Rain falls on the foe for two seconds, chilling whoever stands in it." },
  summonwind: { name: "Summon Wind", does: "A black wind-dragon exhales yin wind at the foe at middle reach: chills." },
  beans: { name: "Beans to Soldiers", does: "Souls of the slain rise as soldiers round the foe, dragging it in and cutting it while it stands there." },
  yinmoon: { name: "Yin Moon Clear Sky", does: "Marks the foe with a moon sign, then cracks break open: breaks its defence." },
  soulsearch: { name: "Soul Searching", does: "Pulls at the foe's soul at close range: silences it and steals a point of rage." },
  spiritchange: { name: "Spirit Transformation", does: "A dart that wipes the foe's memory of its spells: silences." },
  bodyfix: { name: "Body Fixation", does: "A seal under the foe that holds its body in place: a long freeze, little damage." },
  restriction: { name: "Destruction Restriction", does: "Rings of runes write themselves round the foe, locking blinking until they close, then crush. Only hits on the ground." },
  plumblossom: { name: "Eighteen Plum Blossom Restrictions", does: "A self-running restriction of rings round the foe that breaks its defence when it closes." },
  lifedeath: { name: "Life and Death Seal", does: "Left hand life, right hand death: a seal that closes on the foe and freezes it briefly." },
  karma: { name: "Cause and Effect Seal", does: "A dome that answers the next close blow with a stunning counter." },
  dreamback: { name: "Dream Return to Ancient Times", does: "Runes close round the caster: 1 to 3 charges, each stops a whole blow. Cleanses." },
  dreamdao: { name: "Dream Dao", does: "Drags the foe into a dream at middle reach: silences it." },
  flowingmoon: { name: "Flowing Moon", does: "Turns time back on the caster: heals and washes off burn, silence, defence down, chill and shock." },
  remnantnight: { name: "Remnant Night", does: "The night sky cracks over the foe and falls: a heavy blow that silences. Slow to land; only hits on the ground." },
  heavenflip: { name: "Heaven Flipping Seal", does: "A giant seal overturns heaven onto the foe: a huge blow that breaks its defence." },
  thirdeye: { name: "Third Eye", does: "A beam from the third eye that breaks the defence of whatever it hits." },
  sevencolor: { name: "Seven-Colored Dao Spear", does: "A spear of seven-coloured light that carries the caster through the foe." },
  umbrella: { name: "World-Burning Umbrella", does: "An ancient umbrella sweeps fire over the foe: burns." },
  devour: { name: "Spirit-Devouring Art", does: "Devours the foe at close range: steals a point of rage." },
  starfall: { name: "Falling Star", does: "Two stars fall one after another where the foe will be." },
  seize: { name: "Foundation Seizing", does: "A dart that takes a point of the foe's rage." },
  yellowspring: { name: "Yellow Spring Finger", does: "A finger of the underworld's cold at middle reach: chills, and hits harder per chill stack on the foe." },
  ancientthunder: { name: "Ancient Thunder Realm", does: "An ancient thunder dragon falls twice on the foe, one strike after another, and shocks it." },
  demonfinger: { name: "Demon Transformation Finger", does: "A finger of demonic qi that silences what it hits." },
  kunjiwhip: { name: "Kunji Whip", does: "A whip that strikes the soul directly at middle reach: silences, and shocks." },
  tigerflag: { name: "Heavenly Tiger Flag", does: "A heavenly tiger leaps from the flag in a straight line and mauls what it meets." },
  timerestrict: { name: "Time Restriction", does: "A restriction of time round the foe: locks blinking while it runs, then closes and holds the foe. Only hits on the ground." },
  lifedeathrestrict: { name: "Life and Death Restriction", does: "Turns the vitality of whoever stands in it into death energy: a cloud that burns for three seconds." },
  soulbanner: { name: "Soul Banner", does: "Spirits swirl in a vortex over the foe, dragging it in and cutting it while it stands there." },
  servitude: { name: "Spirit Servitude Seal", does: "A seal under the foe that drags it in, then closes on it: silences." },
  voidnail: { name: "Seven-Colored Divine Void Nail", does: "A nail of void driven at the foe at great speed: breaks its defence." },
};

/** What each trait does, in a line, read from the rules' `TRAITS` once the engine has loaded. */
export const TRAIT_INFO = {
  get swift() { return `Swift: ${Math.round((1 - TRAITS.swift.cooldown) * 100)}% reduced Cooldown`; },
  get heavy() { return `Heavy: ${Math.round((TRAITS.heavy.power - 1) * 100)}% more Power`; },
  piercing: "Piercing: cannot be Guarded",
  get lingering() { return `Lingering: +${TRAITS.lingering.stacks} Chill or Shock stack`; },
  get cheap() { return `Cheap: ${Math.round((1 - TRAITS.cheap.cost) * 100)}% reduced Qi Cost`; },
};

/** Each buff's name and what it does, in a line (see `buffsOf`). */
export const BUFF_INFO = {
  fury: { name: "Fury", does: "Each blow landed adds +4% attack for the rest of the round, up to 5 stacks." },
  unyielding: { name: "Unyielding", does: "Once a round, gets back up from a killing blow with 1 life." },
  attune: { name: "Attuned", does: "Every second spell cast gives 1 rage." },
  ward: { name: "Ward", does: "Each charge stops a whole blow." },
  barrier: { name: "Barrier", does: "A dome stands round it: the bell counters a close blow, the mirror sends a bolt back." },
  grit: { name: "Grit", does: "Hard controls (freeze, stun, shock's hold) last shorter; from 2, a freeze is only a chill." },
  burn: { name: "Burn", does: "Loses life every half second." },
  chill: { name: "Chill", does: "Each stack slows walking; the third freezes. Fades 2.5 s after the last." },
  shock: { name: "Shock", does: "Twitches now and then, losing life each time, faster with more stacks." },
  frozen: { name: "Frozen", does: "Cannot move or act." },
  defDown: { name: "Defence Down", does: "Takes more damage from every blow." },
  silence: { name: "Silenced", does: "Only the sword answers: no spells." },
  sealed: { name: "Sealed", does: "Locked in place: cannot blink out." },
};

const TIER_NAME = { common: "Common", rare: "Rare", epic: "Epic", legendary: "Legendary" };

/**
 * The tooltip for slot `i` of fighter f's skill bar (0 to 2 the bar skills, 3 the ultimate), in a game tooltip's
 * sections: `{ title, type, tone, props, text, mods }`. `type` is the line under the name (the tier, or "Ultimate"),
 * `tone` what colours the header (a rarity, "ultimate" or null), `props` `[label, value]` pairs, `text` what it does,
 * `mods` its traits.
 */
export function skillInfo(f, i) {
  const slot = skillBar(f)[i];
  if (!slot) return null;
  if (!slot.spell) {
    return { title: "Empty Slot", type: null, tone: null, props: [], text: `Draws a new skill in ${Math.max(0, slot.left).toFixed(1)} s.`, mods: [] };
  }
  const spell = SPELLS[slot.spell];
  const swift = slot.traits.includes("swift") ? TRAITS.swift.cooldown : 1;
  const cheap = slot.traits.includes("cheap") ? TRAITS.cheap.cost : 1;
  const cost = Math.round(spell.cost * cheap);
  const props = [["Cost", cost > 0 ? `${cost} Qi` : "Free"], ["Cooldown", `${(spell.cooldown * swift).toFixed(1)} s`]];
  if (slot.ultimate) props.push(["Rage", `${Math.min(RAGE, Math.round(slot.charge * RAGE))}/${RAGE}`]);
  else props.push(["Power", `${Math.round(slot.power * 100)}%`]);
  return {
    title: SPELL_INFO[slot.spell].name,
    type: slot.ultimate ? "Ultimate" : `${TIER_NAME[slot.rarity]}${slot.evolved ? " · Evolved" : ""}`,
    tone: slot.ultimate ? "ultimate" : slot.rarity,
    props,
    text: SPELL_INFO[slot.spell].does,
    mods: slot.traits.map((t) => TRAIT_INFO[t]),
  };
}

/** The tooltip for a buff from `buffsOf`, in the same sections as a skill's; `tone` is "good" or "bad". */
export function buffInfo(b) {
  const info = BUFF_INFO[b.id];
  return {
    title: info.name,
    type: b.good ? "Buff" : "Debuff",
    tone: b.good ? "good" : "bad",
    props: b.stacks > 0 ? [["Stacks", String(b.stacks)]] : [],
    text: info.does,
    mods: [],
  };
}
