//! Phẩm, the ladder where the higher overrides the lower whatever the numbers (research 5, rules 5). Every blow,
//! shield, zone, buff, curse and dispel carries the rank of the skill that made it. A restriction (cấm chế) stands
//! outside the ladder: no dispel, no reflection, no theft; only damage breaks it, and only a rank-5 skill wipes it.

/// A rank on the ladder, or a restriction outside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Rank {
    /// Thuật.
    Art = 1,
    /// Thần thông: hỏa hầu 10.
    Divine = 2,
    /// Quy tắc: the Quy Tắc support or an awakened gem.
    Rule = 3,
    /// Bản nguyên: origin skills.
    Origin = 4,
    /// Tín niệm: one ultimate a fighter.
    Faith = 5,
}

impl Rank {
    pub fn level(self) -> u8 {
        self as u8
    }

    pub fn from_level(n: u8) -> Rank {
        match n {
            0 | 1 => Rank::Art,
            2 => Rank::Divine,
            3 => Rank::Rule,
            4 => Rank::Origin,
            _ => Rank::Faith,
        }
    }

    /// A skill's rank: its own, a step for the Quy Tắc support (at most Rule), a step for an awakened gem (at most
    /// Origin); Origin and Faith only from the skill itself.
    pub fn of_skill(own: Rank, rule_support: bool, awakened: bool) -> Rank {
        let mut n = own.level();
        if rule_support && n < Rank::Rule.level() {
            n += 1;
        }
        if awakened && n < Rank::Origin.level() {
            n += 1;
        }
        Rank::from_level(n.max(own.level()))
    }
}

/// What a skill's effect is graded as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Grade {
    Ranked(Rank),
    /// Cấm chế.
    Restriction,
}

impl Grade {
    pub fn rank(self) -> Option<Rank> {
        match self {
            Grade::Ranked(r) => Some(r),
            Grade::Restriction => None,
        }
    }
}

/// A shield of `shield` stops a blow of `blow` when the blow is not higher. A restriction's blow is damage like any
/// other, stopped by any shield; a restriction shield stops every blow below Faith.
pub fn shield_stops(shield: Grade, blow: Grade) -> bool {
    match (shield, blow) {
        (Grade::Restriction, Grade::Ranked(b)) => b < Rank::Faith,
        (Grade::Restriction, Grade::Restriction) => true,
        (Grade::Ranked(_), Grade::Restriction) => true,
        (Grade::Ranked(s), Grade::Ranked(b)) => b <= s,
    }
}

/// A dispel of `dispel` removes an effect of `effect` (a buff, curse, zone, aura) when the effect is not higher. A
/// restriction never goes; a hollow (hư) origin buff goes only to Faith.
pub fn dispels(dispel: Rank, effect: Grade, hollow: bool) -> bool {
    match effect {
        Grade::Restriction => false,
        Grade::Ranked(e) if hollow => dispel == Rank::Faith && e <= Rank::Faith,
        Grade::Ranked(e) => e <= dispel,
    }
}

/// A projectile of `shot` passing through a zone of `zone` is touched by the zone only when it is not higher.
pub fn zone_touches(zone: Grade, shot: Grade) -> bool {
    match (zone, shot) {
        (Grade::Restriction, _) => true,
        (Grade::Ranked(_), Grade::Restriction) => true,
        (Grade::Ranked(z), Grade::Ranked(s)) => s <= z,
    }
}

/// A sword or palm blow of rank Rule or more breaks a lower ranked zone it passes; restrictions only break by damage.
pub fn breaks_zone(zone: Grade, blow: Rank, sword_or_palm: bool) -> bool {
    match zone {
        Grade::Restriction => false,
        Grade::Ranked(z) => sword_or_palm && blow >= Rank::Rule && z < blow,
    }
}

/// Whether a thing of `grade` can be reflected, stolen or devoured (Thôn Cấm): never a restriction.
pub fn takeable(grade: Grade) -> bool {
    grade != Grade::Restriction
}

/// A Faith skill wipes every domain, aura, zone and restriction on the field.
pub fn wipes_field(by: Rank) -> bool {
    by == Rank::Faith
}
