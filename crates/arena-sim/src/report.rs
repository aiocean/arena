//! What a match measured, skill by skill: casts, blows landed and missed, damage dealt (blows, and the burns,
//! bleeds and curse strikes the skill laid), and how far apart the fighters stood at each cast and each hit (design
//! 12.2's "real reach").

/// Where a skill sits in a fighter's kit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// A basic (`loadout::BASICS`: the blink).
    Basic,
    /// One of the loadout's actives, by its index in `Loadout::actives`.
    Active(usize),
    /// A skill a trigger casts that the loadout does not carry (a linked attack, `event::Scale::Event`).
    Linked,
}

/// One skill's numbers over a match.
#[derive(Clone, Debug, PartialEq)]
pub struct SkillLine {
    /// The skill's pool id.
    pub id: &'static str,
    pub place: Place,
    /// Casts by hand, and casts its trigger made.
    pub casts: u32,
    pub triggered: u32,
    /// Blows that landed (dealt damage or were resolved without a dodge), and blows that missed, were dodged or
    /// were stopped by a ward.
    pub landed: u32,
    pub missed: u32,
    /// Damage it dealt to the foe's shield, life and sense, its burns, bleeds and curse strikes included.
    pub damage: f64,
    /// Damage it dealt to the foe's traps, mines, totems and minions (not in `damage`).
    pub unit_damage: f64,
    /// Blows its traps, mines, totems and minions struck (among `landed`).
    pub device_blows: u32,
    /// Sums of the distance between the fighters (T) at each cast and at each landed blow.
    pub dist_cast: f64,
    pub dist_hit: f64,
}

impl SkillLine {
    /// An empty line for skill `id` at `place`.
    pub fn new(id: &'static str, place: Place) -> SkillLine {
        SkillLine {
            id,
            place,
            casts: 0,
            triggered: 0,
            landed: 0,
            missed: 0,
            damage: 0.0,
            unit_damage: 0.0,
            device_blows: 0,
            dist_cast: 0.0,
            dist_hit: 0.0,
        }
    }

    /// Whether it was used at all (cast by hand or by its trigger).
    pub fn used(&self) -> bool {
        self.casts + self.triggered > 0
    }
}

/// One side's numbers over a match: a line per skill in the fighter's slot order (the basic, the actives
/// in loadout order).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SideReport {
    pub skills: Vec<SkillLine>,
    /// Damage dealt by everything, and the part of it from burns and bleeds.
    pub damage: f64,
    pub dot: f64,
}
