//! What the HUD's tooltips say (from the page's `duel-info.js`): each skill's name and what it does, each trait and
//! buff, and the tooltip for a slot of a fighter's skill bar or a buff under its bars.

use pixfx_duel::bar::{Buff, SlotView};
use pixfx_duel::data::{RAGE, Rarity, TRAITS, Trait, spell};

/// Skill `name`'s name and what it does, in a line.
pub fn spell_info(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "slash" => (
            "Sword Slash",
            "A cut at close range. Free, and always at hand.",
        ),
        "dive" => (
            "Falling Sword",
            "Drops onto the foe sword first; cracks the ground if it lands.",
        ),
        "blink" => ("Blink", "Vanishes and reappears a few steps away."),
        "bolt" => (
            "Thunder Bolt",
            "A bolt of lightning that flies at the foe and shocks it.",
        ),
        "palm" => (
            "Thunder Palm",
            "A thunderclap from the open palm at arm's length: shocks, steals rage, and hits harder per shock stack.",
        ),
        "step" => (
            "Lightning Step",
            "Dashes through the foe, cutting it on the way.",
        ),
        "seal" => (
            "Thunder Seal",
            "Marks the ground under the foe, then strikes it with lightning that shocks.",
        ),
        "orb" => (
            "Ball Lightning",
            "A slow orb that seeks the foe, burning and shocking it.",
        ),
        "bell" => (
            "Golden Bell",
            "A dome that answers the next close blow with a stunning counter.",
        ),
        "heal" => (
            "Healing Light",
            "Heals and washes off burn, silence, defence down, chill and shock.",
        ),
        "spark" => ("Spark", "A quick, cheap bolt that shocks."),
        "burst" => (
            "Thunder Burst",
            "A blast at close range that hits harder per shock stack on the foe.",
        ),
        "quake" => (
            "Quake",
            "The ground under the foe cracks and blows up. Only hits on the ground.",
        ),
        "hex" => (
            "Hex Shield",
            "A dome of hex cells: 1 to 3 charges, each stops a whole blow. Cleanses.",
        ),
        "tribulation" => (
            "Heavenly Tribulation",
            "Ultimate: three thunderclaps fall one after another where the foe will be.",
        ),
        "lotus" => (
            "Ice Lotus",
            "A lotus of ice shards at close range that breaks the foe's defence, harder per chill stack.",
        ),
        "shatter" => (
            "Shatter",
            "A seal under the foe that bursts into cold and freezes it, harder per chill stack.",
        ),
        "icicle" => (
            "Icicle Fall",
            "An icicle drops on the foe, freezing it and breaking its defence. Only hits on the ground.",
        ),
        "collapse" => (
            "Collapse",
            "Space folds in: drags the foe in, then pops, silencing it.",
        ),
        "frostfire" => (
            "Frostfire",
            "A pale fire under the foe that burns and chills whoever stands in it.",
        ),
        "void" => (
            "Void Orb",
            "A dark orb that drags the foe in and burns it while it stands under it.",
        ),
        "mirror" => (
            "Ice Mirror",
            "A dome that sends the next bolt or orb back at its caster.",
        ),
        "ward" => (
            "Feather Ward",
            "1 to 3 charges, each stops a whole blow. Cleanses.",
        ),
        "shard" => ("Ice Shard", "A quick, cheap shard that chills."),
        "jet" => (
            "Water Jet",
            "A jet of freezing water at the foe that chills.",
        ),
        "flurry" => (
            "Flurry",
            "A short snowstorm on the foe that chills it while it stays.",
        ),
        "prison" => (
            "Ice Prison",
            "Ultimate: drags the foe in from far off, freezes and silences it, and locks blinking.",
        ),
        "fireball" => (
            "Fireball",
            "A ball of fire that flies at the foe and sets it burning.",
        ),
        "gravity" => (
            "Gravity Technique",
            "Pulls the foe in toward a point, then crushes it there.",
        ),
        "earthsplit" => (
            "Earth Split",
            "The ground under the foe splits and blows up, breaking its defence. Only hits on the ground.",
        ),
        "finger" => (
            "Finger of Annihilation",
            "A thin, fast beam from one finger that hits hard.",
        ),
        "jithunder" => ("Ji Thunder", "A heavy bolt of the Ji Realm that shocks."),
        "gather" => (
            "Thunder Gathering Seal",
            "Gathers thunder under the foe and releases it in one burst: shocks, and hits harder per shock stack on the foe.",
        ),
        "ancientgod" => (
            "Ancient God Blood",
            "The body swells into an Ancient God phantom for one heavy blow at close range.",
        ),
        "slashluo" => (
            "Slash Luo",
            "A sword form that cuts through space: dashes through the foe along a long line.",
        ),
        "arsenal" => (
            "Magic Arsenal",
            "A ring of flying swords turns over the foe and cuts whoever stands under it, then strikes.",
        ),
        "inchearth" => (
            "Shrinking the Earth",
            "A long blink: the ground folds and the caster steps out far away.",
        ),
        "onestep" => (
            "One Step to Heaven",
            "One stride that carries the caster through the foe, a long way, cutting it on the way.",
        ),
        "mountain" => (
            "Mountain Collapse",
            "A peak falls on the foe: a huge blow after a long mark. Only hits on the ground.",
        ),
        "heartbeat" => (
            "Heartbeat Thunder Sound",
            "A thunderclap at the heart, at close range: shocks.",
        ),
        "lightshield" => (
            "Light and Shadow Shield",
            "A dome of gathered light that sends the next bolt or orb back at its caster.",
        ),
        "nineturn" => (
            "Nine Revolutions Refining",
            "Spirals power inside the body, then releases it in one huge blast at close range. Long to gather.",
        ),
        "indestructible" => (
            "Indestructible Body",
            "Recovery outpaces destruction: heals and washes off burn, silence, defence down, chill and shock.",
        ),
        "ancientprotect" => (
            "Ancient Protection",
            "A golden bell of ancestral souls: 1 to 3 charges, each stops a whole blow. Cleanses.",
        ),
        "vermilion" => (
            "Vermilion Bird Transformation",
            "A burning bird of fire sweeps out of the palm at close range and sets the foe burning.",
        ),
        "summonrain" => (
            "Summon Rain",
            "Rain falls on the foe for two seconds, chilling whoever stands in it.",
        ),
        "summonwind" => (
            "Summon Wind",
            "A black wind-dragon exhales yin wind at the foe at middle reach: chills.",
        ),
        "beans" => (
            "Beans to Soldiers",
            "Souls of the slain rise as soldiers round the foe, dragging it in and cutting it while it stands there.",
        ),
        "yinmoon" => (
            "Yin Moon Clear Sky",
            "Marks the foe with a moon sign, then cracks break open: breaks its defence.",
        ),
        "soulsearch" => (
            "Soul Searching",
            "Pulls at the foe's soul at close range: silences it and steals a point of rage.",
        ),
        "spiritchange" => (
            "Spirit Transformation",
            "A dart that wipes the foe's memory of its spells: silences.",
        ),
        "bodyfix" => (
            "Body Fixation",
            "A seal under the foe that holds its body in place: a long freeze, little damage.",
        ),
        "restriction" => (
            "Destruction Restriction",
            "Rings of runes write themselves round the foe, locking blinking until they close, then crush. Only hits on the ground.",
        ),
        "plumblossom" => (
            "Eighteen Plum Blossom Restrictions",
            "A self-running restriction of rings round the foe that breaks its defence when it closes.",
        ),
        "lifedeath" => (
            "Life and Death Seal",
            "Left hand life, right hand death: a seal that closes on the foe and freezes it briefly.",
        ),
        "karma" => (
            "Cause and Effect Seal",
            "A dome that answers the next close blow with a stunning counter.",
        ),
        "dreamback" => (
            "Dream Return to Ancient Times",
            "Runes close round the caster: 1 to 3 charges, each stops a whole blow. Cleanses.",
        ),
        "dreamdao" => (
            "Dream Dao",
            "Drags the foe into a dream at middle reach: silences it.",
        ),
        "flowingmoon" => (
            "Flowing Moon",
            "Turns time back on the caster: heals and washes off burn, silence, defence down, chill and shock.",
        ),
        "remnantnight" => (
            "Remnant Night",
            "The night sky cracks over the foe and falls: a heavy blow that silences. Slow to land; only hits on the ground.",
        ),
        "heavenflip" => (
            "Heaven Flipping Seal",
            "A giant seal overturns heaven onto the foe: a huge blow that breaks its defence.",
        ),
        "thirdeye" => (
            "Third Eye",
            "A beam from the third eye that breaks the defence of whatever it hits.",
        ),
        "sevencolor" => (
            "Seven-Colored Dao Spear",
            "A spear of seven-coloured light that carries the caster through the foe.",
        ),
        "umbrella" => (
            "World-Burning Umbrella",
            "An ancient umbrella sweeps fire over the foe: burns.",
        ),
        "devour" => (
            "Spirit-Devouring Art",
            "Devours the foe at close range: steals a point of rage.",
        ),
        "starfall" => (
            "Falling Star",
            "Two stars fall one after another where the foe will be.",
        ),
        "seize" => (
            "Foundation Seizing",
            "A dart that takes a point of the foe's rage.",
        ),
        "yellowspring" => (
            "Yellow Spring Finger",
            "A finger of the underworld's cold at middle reach: chills, and hits harder per chill stack on the foe.",
        ),
        "ancientthunder" => (
            "Ancient Thunder Realm",
            "An ancient thunder dragon falls twice on the foe, one strike after another, and shocks it.",
        ),
        "demonfinger" => (
            "Demon Transformation Finger",
            "A finger of demonic qi that silences what it hits.",
        ),
        "kunjiwhip" => (
            "Kunji Whip",
            "A whip that strikes the soul directly at middle reach: silences, and shocks.",
        ),
        "tigerflag" => (
            "Heavenly Tiger Flag",
            "A heavenly tiger leaps from the flag in a straight line and mauls what it meets.",
        ),
        "timerestrict" => (
            "Time Restriction",
            "A restriction of time round the foe: locks blinking while it runs, then closes and holds the foe. Only hits on the ground.",
        ),
        "lifedeathrestrict" => (
            "Life and Death Restriction",
            "Turns the vitality of whoever stands in it into death energy: a cloud that burns for three seconds.",
        ),
        "soulbanner" => (
            "Soul Banner",
            "Spirits swirl in a vortex over the foe, dragging it in and cutting it while it stands there.",
        ),
        "servitude" => (
            "Spirit Servitude Seal",
            "A seal under the foe that drags it in, then closes on it: silences.",
        ),
        "voidnail" => (
            "Seven-Colored Divine Void Nail",
            "A nail of void driven at the foe at great speed: breaks its defence.",
        ),
        "fingerdeath" => (
            "Finger of Death",
            "Three fingers of death fall one after another where the foe will be.",
        ),
        "heavenlychop" => (
            "Heavenly Chop",
            "A sword cut that splits heaven at close range: a heavy blow.",
        ),
        "fireescape" => (
            "Fire Escape",
            "Turns to flame and dashes through the foe, setting it burning.",
        ),
        "flameorigin" => (
            "Rise Three Realm Flame Origin",
            "The origin of flame rises under the foe and burns it.",
        ),
        "warprint" => (
            "War Spirit Print",
            "A seal of war stamped on the foe at close range: a heavy blow that breaks its defence.",
        ),
        "earthescape" => (
            "Earth Escape",
            "Sinks into the ground and comes out a few steps away.",
        ),
        "celestialslaughter" => (
            "Celestial Slaughter Art",
            "A bolt of slaughter intent, slow to gather and heavy.",
        ),
        "ancientcauldron" => (
            "Ancient God Cauldron",
            "Swaps places with the foe, wherever it stands.",
        ),
        "eightextremes" => (
            "Eight Extremes Dao",
            "Eight Dao paths fall on the foe one after another, four strikes that burn.",
        ),
        "daofusion" => (
            "Dao Art Fusion",
            "Treasures, abilities and rules fused into one blow: slow to gather, heavy, and it shocks.",
        ),
        "ghostsummon" => (
            "Ghost Summoning",
            "Ghosts rise round the foe and drag it in while they cut it.",
        ),
        "soulvortex" => (
            "Soul Vortex",
            "A vortex of souls pulls the foe in, then bursts.",
        ),
        "soulrefine" => (
            "Soul Refining",
            "Refines a soul into life: heals and washes off burn, silence, defence down, chill and shock.",
        ),
        "starrotation" => (
            "Star Rotation",
            "Three stars turn down on the foe one after another where it will be.",
        ),
        "meritspirit" => (
            "Merit Spirit",
            "A spirit of merit stands over the caster: 1 to 3 charges, each stops a whole blow. Cleanses.",
        ),
        "bodyformation" => (
            "Body Formation",
            "Forms the body anew: heals and washes off burn, silence, defence down, chill and shock.",
        ),
        "truefalse" => (
            "True and False Seal",
            "Open eyes are truth, closed eyes falsehood: a seal on the foe that silences it and breaks its defence.",
        ),
        "daoyellow" => (
            "Dao Transformation Yellow Spring",
            "A reincarnation cycle forms over the foe: a cold field that chills whoever stands in it for three seconds.",
        ),
        "springsoul" => (
            "Condensed Spring Soul",
            "A hundred million souls condensed into one dart that chills what it hits.",
        ),
        "starsoul" => (
            "Star Soul Extraction",
            "Pulls the soul out of the foe at close range: steals a point of rage.",
        ),
        "spiritsummon" => (
            "Life Destruction Spirit Summoning",
            "Spends 20 of your life (never your last point) to call spirits down in two waves that burn the foe; each point paid makes them stronger.",
        ),
        "starcompass" => (
            "Star Compass",
            "You and the foe trade places: quicker and cheaper than the cauldron.",
        ),
        _ => return None,
    })
}

/// Buff `id`'s name and what it does, in a line (see `buffs_of`).
pub fn buff_info(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        "fury" => (
            "Fury",
            "Each blow landed adds +4% attack for the rest of the round, up to 5 stacks.",
        ),
        "unyielding" => (
            "Unyielding",
            "Once a round, gets back up from a killing blow with 1 life.",
        ),
        "attune" => ("Attuned", "Every second spell cast gives 1 rage."),
        "ward" => ("Ward", "Each charge stops a whole blow."),
        "barrier" => (
            "Barrier",
            "A dome stands round it: the bell counters a close blow, the mirror sends a bolt back.",
        ),
        "grit" => (
            "Grit",
            "Hard controls (freeze, stun, shock's hold) last shorter; from 2, a freeze is only a chill.",
        ),
        "burn" => ("Burn", "Loses life every half second."),
        "chill" => (
            "Chill",
            "Each stack slows walking; the third freezes. Fades 2.5 s after the last.",
        ),
        "shock" => (
            "Shock",
            "Twitches now and then, losing life each time, faster with more stacks.",
        ),
        "frozen" => ("Frozen", "Cannot move or act."),
        "defDown" => ("Defence Down", "Takes more damage from every blow."),
        "silence" => ("Silenced", "Only the sword answers: no spells."),
        "sealed" => ("Sealed", "Locked in place: cannot blink out."),
        _ => return None,
    })
}

/// What trait `t` does, in a line, read from the rules' `TRAITS`.
pub fn trait_info(t: Trait) -> String {
    let pct = |x: f64| (x * 100.0 + 0.5) as i64;
    match t {
        Trait::Swift => format!(
            "Swift: {}% reduced Cooldown",
            pct(1.0 - TRAITS.swift_cooldown)
        ),
        Trait::Heavy => format!("Heavy: {}% more Power", pct(TRAITS.heavy_power - 1.0)),
        Trait::Piercing => "Piercing: cannot be Guarded".to_string(),
        Trait::Lingering => format!(
            "Lingering: +{} Chill or Shock stack",
            TRAITS.lingering_stacks as i64
        ),
        Trait::Cheap => format!("Cheap: {}% reduced Qi Cost", pct(1.0 - TRAITS.cheap_cost)),
    }
}

/// What colours a tooltip's header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Rarity(Rarity),
    Ultimate,
    Good,
    Bad,
}

/// A tooltip, in a game tooltip's sections: its title, the line under it (the tier, "Ultimate", "Buff"), what
/// colours its header, `[label, value]` pairs, what it does, and its traits.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub title: String,
    pub kind: Option<String>,
    pub tone: Tone,
    pub props: Vec<(String, String)>,
    pub text: String,
    pub mods: Vec<String>,
}

fn tier_name(r: Rarity) -> &'static str {
    match r {
        Rarity::Common => "Common",
        Rarity::Rare => "Rare",
        Rarity::Epic => "Epic",
        Rarity::Legendary => "Legendary",
    }
}

/// The tooltip for a slot of a skill bar (the bar skills, then the ultimate).
pub fn skill_tip(slot: &SlotView) -> Tip {
    let Some((name, s)) = slot.spell.and_then(|n| Some((spell_info(n)?, spell(n)?))) else {
        return Tip {
            title: "Empty Slot".into(),
            kind: None,
            tone: Tone::Plain,
            props: Vec::new(),
            text: format!("Draws a new skill in {:.1} s.", slot.left.max(0.0)),
            mods: Vec::new(),
        };
    };
    let swift = if slot.traits.contains(&Trait::Swift) {
        TRAITS.swift_cooldown
    } else {
        1.0
    };
    let cheap = if slot.traits.contains(&Trait::Cheap) {
        TRAITS.cheap_cost
    } else {
        1.0
    };
    let cost = (s.cost * cheap + 0.5) as i64;
    let mut props = vec![
        (
            "Cost".to_string(),
            if cost > 0 {
                format!("{cost} Qi")
            } else {
                "Free".to_string()
            },
        ),
        (
            "Cooldown".to_string(),
            format!("{:.1} s", s.cooldown * swift),
        ),
    ];
    if slot.ultimate {
        let rage = ((slot.charge * RAGE + 0.5) as i64).min(RAGE as i64);
        props.push(("Rage".into(), format!("{rage}/{}", RAGE as i64)));
    } else {
        props.push((
            "Power".into(),
            format!("{}%", (slot.power * 100.0 + 0.5) as i64),
        ));
    }
    Tip {
        title: name.0.into(),
        kind: Some(if slot.ultimate {
            "Ultimate".into()
        } else {
            format!(
                "{}{}",
                tier_name(slot.rarity),
                if slot.evolved { " · Evolved" } else { "" }
            )
        }),
        tone: if slot.ultimate {
            Tone::Ultimate
        } else {
            Tone::Rarity(slot.rarity)
        },
        props,
        text: name.1.into(),
        mods: slot.traits.iter().map(|t| trait_info(*t)).collect(),
    }
}

/// The tooltip for a buff under a fighter's bars.
pub fn buff_tip(b: &Buff) -> Option<Tip> {
    let (name, does) = buff_info(b.id)?;
    Some(Tip {
        title: name.into(),
        kind: Some(if b.good { "Buff" } else { "Debuff" }.into()),
        tone: if b.good { Tone::Good } else { Tone::Bad },
        props: if b.stacks > 0.0 {
            vec![("Stacks".into(), format!("{}", b.stacks))]
        } else {
            Vec::new()
        },
        text: does.into(),
        mods: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixfx_duel::data::{School, school};

    #[test]
    fn every_skill_and_buff_has_words() {
        for s in [School::Thunder, School::Frost] {
            for name in school(s).pool.iter().chain([&school(s).ultimate]) {
                assert!(spell_info(name).is_some(), "{name}");
            }
        }
        for id in [
            "fury",
            "unyielding",
            "attune",
            "ward",
            "barrier",
            "grit",
            "burn",
            "chill",
            "shock",
            "frozen",
            "defDown",
            "silence",
            "sealed",
        ] {
            assert!(buff_info(id).is_some(), "{id}");
        }
    }
}
