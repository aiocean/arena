//! A skill's range band (owner, 2026-11-19): how far from the foe a skill thrown at it can be begun, in game pixels
//! between the two fighters' centres. Its reach tag decides it: a near blow is begun close, a middle-reach area at
//! a middle distance, a far bolt only from far off, so a fighter who wants its far skills has to step away first.
//! Outside its band a skill cannot be cast at all (the duel's `bar::castable`): nothing fizzles, nothing is spent.
//!
//! Skills that are not thrown at the foe have no band: the whole arena (`Whole`), fields and domains, auras,
//! blessings, stances and heralds, guards, recoveries and movements (a blink), the devices (traps, mines, totems,
//! minions: they are laid, then strike as they will), and every skill that deals no damage (buffs, wards, curses
//! laid with no blow of their own).

use crate::gem::Gem;
use crate::tag::{Tag, Tags};

/// The distances between the fighters a skill can be begun at, `min..=max` game pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Band {
    pub min: f64,
    pub max: f64,
}

impl Band {
    /// Whether a skill of this band can be begun `gap` pixels from its foe.
    pub fn holds(self, gap: f64) -> bool {
        gap >= self.min && gap <= self.max
    }
}

/// Near (a palm, a burst, a close blast): from touching to a short step off. The near blasts reach 104 to 208 px; at
/// most 192 keeps all but the very end of the longest.
pub const NEAR: Band = Band {
    min: 0.0,
    max: 192.0,
};
/// Middle reach (a seal, a quake, an area landed on the foe): not in its face, not across the arena.
pub const MID: Band = Band {
    min: 112.0,
    max: 400.0,
};
/// Far (a bolt, a finger, an arrow, a banner): only from well away, and not from the far wall's other side of the
/// arena (it is 880 px wide, the fighters' centres at most about 830 apart).
pub const FAR: Band = Band {
    min: 224.0,
    max: 640.0,
};

/// The tags that free a skill of any band: it is not thrown at the foe (the module's doc).
pub const FREE: &[Tag] = &[
    Tag::Whole,
    Tag::Field,
    Tag::Domain,
    Tag::Aura,
    Tag::Blessing,
    Tag::Herald,
    Tag::Stance,
    Tag::Guard,
    Tag::Recovery,
    Tag::Movement,
    Tag::Trap,
    Tag::Mine,
    Tag::Totem,
    Tag::Minion,
];

/// The band of a skill with `tags` (its own and its supports' `becomes`) that deals damage or not (`Gem::deals_damage`);
/// `None` when it can be cast at any distance.
pub fn band(tags: Tags, damaging: bool) -> Option<Band> {
    if !damaging || tags.any(FREE) {
        return None;
    }
    if tags.has(Tag::Near) {
        Some(NEAR)
    } else if tags.has(Tag::Mid) {
        Some(MID)
    } else if tags.has(Tag::Far) {
        Some(FAR)
    } else {
        None
    }
}

/// The band of gem `g` cast bare (no support turning it into something else).
pub fn of(g: &Gem) -> Option<Band> {
    band(Tags::of(g.tags), g.deals_damage())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gem::gem;

    /// The band of the pool's gem `id`; `None` for no such gem.
    fn of_id(id: &str) -> Option<Option<Band>> {
        gem(id).map(of)
    }

    #[test]
    fn bands_follow_reach() {
        assert_eq!(of_id("fireball"), Some(Some(FAR)));
        assert_eq!(of_id("tigerflag"), Some(Some(FAR)));
        assert_eq!(of_id("voidnail"), Some(Some(FAR)));
        assert_eq!(of_id("libow"), Some(Some(FAR)));
        assert_eq!(of_id("qibolt"), Some(Some(FAR)));
        assert_eq!(of_id("seal"), Some(Some(MID)));
        assert_eq!(of_id("quake"), Some(Some(MID)));
        assert_eq!(of_id("palm"), Some(Some(NEAR)));
        assert_eq!(of_id("qiburst"), Some(Some(NEAR)));
    }

    #[test]
    fn what_is_not_thrown_has_no_band() {
        for id in [
            "blink",
            "daoyellow",
            "arsenal",
            "beans",
            "restriction",
            "summonrain",
        ] {
            assert_eq!(of_id(id), Some(None), "{id}");
        }
    }

    #[test]
    fn bands_overlap_so_every_distance_has_one() {
        // walking out, a fighter goes near -> middle -> far with no distance where none of the three can be cast
        const { assert!(MID.min < NEAR.max && FAR.min < MID.max) };
        assert!(NEAR.holds(0.0) && !NEAR.holds(NEAR.max + 1.0));
        assert!(!FAR.holds(FAR.min - 1.0) && FAR.holds(FAR.min) && FAR.holds(FAR.max));
        const { assert!(FAR.max < 880.0) };
    }
}
