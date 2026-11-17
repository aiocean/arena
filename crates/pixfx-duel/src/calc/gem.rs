//! The shared pool: every skill (thần thông) any fighter may take, with its tags, kinds of damage, rank, base, costs
//! and timings (design, sections 3 and 11). No class owns a skill. `spell` names the `data::SPELLS` entry a skill
//! borrows its motion and effect from until it has its own.

use super::ailment::{Control, ForceAilment};
use super::cultivator::Star;
use super::element::Element;
use super::grade::Rank;
use super::tag::{Tag, Tags};

/// A resource a skill spends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Resource {
    Qi,
    /// Máu: paid at the cast, never the last point; each 1% of full life paid adds `BLOOD_POWER`.
    Life,
    /// Thọ nguyên: never regrows in a fight.
    Lifespan,
    /// Sát khí.
    Intent,
    /// Hồn.
    Souls,
    /// Thần thức.
    Sense,
    /// Tử khí: what a life-death cultivator pays in place of life below 10%.
    DeathQi,
    /// Tinh điểm of a kind.
    Star(Star),
}

/// A skill's tier, from measurement (design, 12.3); it sets the loadout points the skill costs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Tier {
    C,
    B,
    A,
    S,
}

impl Tier {
    /// Loadout points (design, 2).
    pub fn points(self) -> u32 {
        match self {
            Tier::C => 3,
            Tier::B => 4,
            Tier::A => 5,
            Tier::S => 6,
        }
    }
}

/// What a skill does besides its damage. The fight reads these; the calculator reads only what changes a blow.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Effect {
    /// A hard control of this many seconds (through grit).
    Control(Control, f64),
    /// Extra stacks of the ailment its kind lays.
    Ailment(f64),
}

/// A skill in the shared pool.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gem {
    /// Its code name (a `data::SPELLS` name where the skill already exists).
    pub id: &'static str,
    /// Its name in the novel.
    pub name: &'static str,
    pub tags: &'static [Tag],
    /// Its damage by kind, shares summing to 1 (empty for a skill that deals none).
    pub split: &'static [(Element, f64)],
    /// The rank it starts at; `restriction` puts it outside the ladder.
    pub rank: Rank,
    pub restriction: bool,
    /// A spell's base at level 1, or an attack's effectiveness (a share of might).
    pub base: f64,
    pub costs: &'static [(Resource, f64)],
    pub cooldown: f64,
    pub windup: f64,
    pub recover: f64,
    /// Uses a match (lá bài một lần), and whether it can be shown without casting (Lộ bài).
    pub uses: Option<u8>,
    pub bluff: bool,
    pub force: ForceAilment,
    pub tier: Tier,
    /// The `data::SPELLS` entry it borrows its motion and effect from.
    pub spell: &'static str,
    /// Where it comes from: `TN<chapter>` or `CM<chapter>`, or "arena" for the arena's own.
    pub source: &'static str,
    pub effects: &'static [Effect],
}

impl Gem {
    pub fn tags(&self) -> Tags {
        Tags::of(self.tags)
    }

    pub fn is_attack(&self) -> bool {
        self.tags().has(Tag::Attack)
    }

    pub fn is_ultimate(&self) -> bool {
        self.tags().has(Tag::Ultimate)
    }

    /// The tags including those of its kinds of damage.
    pub fn all_tags(&self) -> Tags {
        self.split
            .iter()
            .fold(self.tags(), |t, &(e, _)| t.with(Tag::of(e)))
    }
}

/// Every skill in the pool, basics and ultimates included.
pub const GEMS: &[Gem] = &[];

/// The skill named `id`.
pub fn gem(id: &str) -> Option<&'static Gem> {
    GEMS.iter().find(|g| g.id == id)
}
