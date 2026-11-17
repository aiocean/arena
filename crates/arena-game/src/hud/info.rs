//! What the HUD's tooltips say: each skill's name (the page's `duel-info.js` names, else the novel's) and what it is in
//! the damage system (its kinds of damage and its tags), each buff, and the tooltip for a slot of a fighter's skill
//! bar or a buff under its bars.

use arena_duel::bar::{Buff, SlotView};
use arena_duel::data::spell;

/// Skill `name`'s English name, where the page named it.
pub fn spell_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "slash" => "Sword Slash",
        "dive" => "Falling Sword",
        "blink" => "Blink",
        "bolt" => "Thunder Bolt",
        "palm" => "Thunder Palm",
        "step" => "Lightning Step",
        "seal" => "Thunder Seal",
        "orb" => "Ball Lightning",
        "bell" => "Golden Bell",
        "heal" => "Healing Light",
        "spark" => "Spark",
        "burst" => "Thunder Burst",
        "quake" => "Quake",
        "hex" => "Hex Shield",
        "tribulation" => "Heavenly Tribulation",
        "lotus" => "Ice Lotus",
        "collapse" => "Collapse",
        "void" => "Void Orb",
        "mirror" => "Ice Mirror",
        "ward" => "Feather Ward",
        "jet" => "Water Jet",
        "fireball" => "Fireball",
        "gravity" => "Gravity Technique",
        "earthsplit" => "Earth Split",
        "finger" => "Finger of Annihilation",
        "jithunder" => "Ji Thunder",
        "gather" => "Thunder Gathering Seal",
        "ancientgod" => "Ancient God Blood",
        "slashluo" => "Slash Luo",
        "arsenal" => "Magic Arsenal",
        "inchearth" => "Shrinking the Earth",
        "onestep" => "One Step to Heaven",
        "mountain" => "Mountain Collapse",
        "heartbeat" => "Heartbeat Thunder Sound",
        "lightshield" => "Light and Shadow Shield",
        "nineturn" => "Nine Revolutions Refining",
        "indestructible" => "Indestructible Body",
        "ancientprotect" => "Ancient Protection",
        "vermilion" => "Vermilion Bird Transformation",
        "summonrain" => "Summon Rain",
        "summonwind" => "Summon Wind",
        "beans" => "Beans to Soldiers",
        "yinmoon" => "Yin Moon Clear Sky",
        "soulsearch" => "Soul Searching",
        "spiritchange" => "Spirit Transformation",
        "bodyfix" => "Body Fixation",
        "restriction" => "Destruction Restriction",
        "plumblossom" => "Eighteen Plum Blossom Restrictions",
        "lifedeath" => "Life and Death Seal",
        "karma" => "Cause and Effect Seal",
        "dreamback" => "Dream Return to Ancient Times",
        "dreamdao" => "Dream Dao",
        "flowingmoon" => "Flowing Moon",
        "remnantnight" => "Remnant Night",
        "heavenflip" => "Heaven Flipping Seal",
        "thirdeye" => "Third Eye",
        "sevencolor" => "Seven-Colored Dao Spear",
        "umbrella" => "World-Burning Umbrella",
        "devour" => "Spirit-Devouring Art",
        "starfall" => "Falling Star",
        "seize" => "Foundation Seizing",
        "yellowspring" => "Yellow Spring Finger",
        "ancientthunder" => "Ancient Thunder Realm",
        "demonfinger" => "Demon Transformation Finger",
        "kunjiwhip" => "Kunji Whip",
        "tigerflag" => "Heavenly Tiger Flag",
        "timerestrict" => "Time Restriction",
        "lifedeathrestrict" => "Life and Death Restriction",
        "soulbanner" => "Soul Banner",
        "servitude" => "Spirit Servitude Seal",
        "voidnail" => "Seven-Colored Divine Void Nail",
        "fingerdeath" => "Finger of Death",
        "heavenlychop" => "Heavenly Chop",
        "fireescape" => "Fire Escape",
        "flameorigin" => "Rise Three Realm Flame Origin",
        "warprint" => "War Spirit Print",
        "earthescape" => "Earth Escape",
        "celestialslaughter" => "Celestial Slaughter Art",
        "ancientcauldron" => "Ancient God Cauldron",
        "eightextremes" => "Eight Extremes Dao",
        "daofusion" => "Dao Art Fusion",
        "ghostsummon" => "Ghost Summoning",
        "soulvortex" => "Soul Vortex",
        "soulrefine" => "Soul Refining",
        "starrotation" => "Star Rotation",
        "meritspirit" => "Merit Spirit",
        "bodyformation" => "Body Formation",
        "truefalse" => "True and False Seal",
        "daoyellow" => "Dao Transformation Yellow Spring",
        "springsoul" => "Condensed Spring Soul",
        "starsoul" => "Star Soul Extraction",
        "spiritsummon" => "Life Destruction Spirit Summoning",
        "starcompass" => "Star Compass",
        _ => return None,
    })
}

/// Skill `name`'s title (its English name, else the novel's) and what it is in the damage system: its kinds of
/// damage and its tags.
pub fn spell_info(name: &str) -> Option<(String, String)> {
    let g = arena_calc::gem::gem(name)?;
    let title = spell_name(name).unwrap_or(g.name).to_string();
    let kinds: Vec<String> = g
        .split
        .iter()
        .map(|&(e, share)| {
            if share < 1.0 {
                format!("{} {}%", e.name(), (share * 100.0 + 0.5) as i64)
            } else {
                e.name().to_string()
            }
        })
        .collect();
    let tags: Vec<String> = g.tags.iter().map(|t| format!("{t:?}")).collect();
    let mut text = if kinds.is_empty() {
        tags.join(" · ")
    } else {
        format!("{} · {}", kinds.join(" + "), tags.join(" · "))
    };
    if g.uses.is_some() {
        text.push_str(" · once a match");
    }
    Some((title, text))
}

/// Buff `id`'s name and what it does, in a line (see `buffs_of`).
pub fn buff_info(id: &str) -> Option<(&'static str, &'static str)> {
    Some(match id {
        "fury" => (
            "Fury",
            "Each blow landed adds +4% damage for the rest of the round, up to 5 stacks.",
        ),
        "unyielding" => (
            "Unyielding",
            "Once a round, gets back up from a killing blow with 1 life.",
        ),
        "ward" => ("Ward", "Each charge stops a whole blow."),
        "barrier" => (
            "Barrier",
            "A dome stands round it: the bell counters a close blow, the mirror sends a bolt back.",
        ),
        "grit" => (
            "Grit",
            "Each hard control halves the next; one stack falls every 2 s without one.",
        ),
        "souls" => (
            "Souls",
            "Spent by soul skills; one per 100 soul damage dealt and per minion dying near.",
        ),
        "bleed" => ("Bleed", "Each cut bleeds; twice as fast while it runs."),
        "daze" => ("Daze", "Full, it is stunned for a moment."),
        "sunder" => ("Sunder", "Less armour each stack; full, its shield breaks."),
        "bind" => ("Bind", "Slower each stack; full, it is rooted."),
        "soak" => (
            "Soak",
            "Less qi back and more thunder taken; full, it chokes.",
        ),
        "burn" => (
            "Burn",
            "Only the strongest burn burns, past armour and shield.",
        ),
        "heavy" => (
            "Heavy",
            "Shorter dashes and blinks; full, it is knocked down.",
        ),
        "jolt" => (
            "Jolt",
            "Takes more damage each stack; full, it is jolted still.",
        ),
        "glare" => ("Glare", "Its yin buffs weaken; full, they are washed off."),
        "wither" => ("Wither", "Heals less, takes more yin; full, healing hurts."),
        "dread" => ("Dread", "Slower windups each stack."),
        "silence" => ("Soul Lost", "Only attacks answer: no spells, no intents."),
        "root" => ("Rooted", "Cannot walk or blink, but can still strike."),
        "sealed" => ("Sealed", "Cannot blink out."),
        "cursed" => ("Cursed", "A curse is on it."),
        _ => return None,
    })
}

/// What colours a tooltip's header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Plain,
    Good,
    Bad,
}

/// A tooltip, in a game tooltip's sections: its title, the line under it (the tier, "Skill", "Buff"), what
/// colours its header, `[label, value]` pairs, what it does, and what modifies it.
#[derive(Clone, Debug, PartialEq)]
pub struct Tip {
    pub title: String,
    pub kind: Option<String>,
    pub tone: Tone,
    pub props: Vec<(String, String)>,
    pub text: String,
    pub mods: Vec<String>,
}

/// The tooltip for a slot of a skill bar.
pub fn skill_tip(slot: &SlotView) -> Tip {
    let Some(((title, text), s)) = slot.spell.and_then(|n| Some((spell_info(n)?, spell(n)?)))
    else {
        return Tip {
            title: "Empty Slot".into(),
            kind: None,
            tone: Tone::Plain,
            props: Vec::new(),
            text: format!("Draws a new skill in {:.1} s.", slot.left.max(0.0)),
            mods: Vec::new(),
        };
    };
    let cost = (s.cost + 0.5) as i64;
    let mut props = vec![
        (
            "Cost".to_string(),
            if cost > 0 {
                format!("{cost} Qi")
            } else {
                "Free".to_string()
            },
        ),
        ("Cooldown".to_string(), format!("{:.1} s", s.cooldown)),
    ];
    if slot.max_uses > 1 {
        props.push(("Uses".into(), format!("{}/{}", slot.uses, slot.max_uses)));
    }
    Tip {
        title,
        kind: Some("Skill".into()),
        tone: Tone::Plain,
        props,
        text,
        mods: Vec::new(),
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

    #[test]
    fn every_skill_and_buff_has_words() {
        for g in arena_calc::gem::GEMS {
            assert!(spell_info(g.id).is_some(), "{}", g.id);
        }
        for id in [
            "fury",
            "unyielding",
            "ward",
            "barrier",
            "grit",
            "souls",
            "bleed",
            "daze",
            "sunder",
            "bind",
            "soak",
            "burn",
            "heavy",
            "jolt",
            "glare",
            "wither",
            "dread",
            "silence",
            "root",
            "sealed",
            "cursed",
        ] {
            assert!(buff_info(id).is_some(), "{id}");
            assert!(super::super::glyphs::buff(id).is_some(), "{id}");
        }
    }
}
