//! Damage over time (design, 17.5): burn (Thiêu) and bleed (Thương) as their own branch of damage, scaled by the
//! striker and not by the target's life. A blow with a fire part lays a burn **instance**, a blow whose force cuts
//! (`ForceAilment::Bleed`) lays a bleed instance; each instance snapshots its damage a second when it is laid (PoE's
//! ignite and bleed) and runs out its own time.
//!
//! - **Base.** The part of that kind after step 7 and before mitigation, worked out as if step 4 read the damage-over-
//!   time modifiers instead of the hit ones ([`DotScale`], [`crate::hit::dot_scale`]): `Damage`, `DotDamage`,
//!   `AilmentDamage(a)` and `DamageOf(k)` for every kind the part has been, never `HitDamage`. Steps 5 to 7 (the
//!   crit, the striker's power, the root, an overcome's cash-in) count as they do for the hit.
//! - **Snapshot.** `dps = rate × base × (1 + DotMulti + DotMultiOf(kind)) × ailment effect × fire tier (burn) ×
//!   (1 − the target's ailment resistance) × fed`; `time = base time × ailment duration`.
//! - **Stacking.** Burn: only the strongest instance deals damage (Ignite), the rest wait out their own time. Bleed:
//!   every instance deals damage. Each keeps at most [`DOT_CAP`]; past it the weakest of the nine goes (a new one
//!   weaker than all eight is dropped). Reaching the cap from below is the ailment's "full".
//! - **Overcome.** A blow that overcomes burn (water) clears every instance and cashes in what they would still have
//!   dealt ([`Instances::remaining`] × [`OVERCOME_REMAINING`]), not a share of life per instance.
//! - **Mitigation**, at each tick, from the bearer's live defensive table ([`mitigation`]): burn by fire resistance
//!   (penetration is a property of hits and does not reach damage over time), straight to life past armour and
//!   shield; bleed past armour, the shield first. Both take `Taken`, `TakenOf(kind)` and `DotTaken`.

use crate::ailment::Ailment;
use crate::cultivator::{BASE, Cultivator};
use crate::element::Element;
use crate::stat::{Stat, Table};

/// Thiêu: a burn deals this share of the blow's fire part a second...
pub const BURN_RATE: f64 = 0.5;
/// ...for this many seconds (before the ailment duration).
pub const BURN_TIME: f64 = 4.0;
/// Thương: a bleed deals this share of the blow's force part a second...
pub const BLEED_RATE: f64 = 0.35;
/// ...for this many seconds (before the ailment duration)...
pub const BLEED_TIME: f64 = 5.0;
/// ...and this many times as much while its bearer moves (1: a bleed runs the same whether its bearer moves or not).
pub const BLEED_MOVING: f64 = 1.0;
/// Instances of one ailment a fighter carries at most; reaching it is the ailment's full.
pub const DOT_CAP: usize = 8;
/// Burn more by the fire's tier (rules, 4.3): red, blue, purple, black (dao fire).
pub const FIRE_TIER: [f64; 4] = [1.0, 1.25, 1.5, 2.0];
/// Hư hỏa: a cast while burning lays a copy of the strongest burn at this share of its damage, with a fresh full
/// time. Weaker than the strongest, it deals nothing until the strongest runs out: hollow fire keeps a burn alive
/// rather than making it hotter.
pub const HOLLOW_FIRE_EFFECT: f64 = 0.5;
/// Khắc on burn or bleed (design 4.1, research 3.1: "gây thêm sát thương bằng phần ailment còn lại"): an overcoming
/// blow that clears the instances adds this share of the damage they would still have dealt
/// ([`Instances::remaining`]) to its overcoming part. Damage, not a count: many tiny burns cash in as little as they
/// would have burnt. (Stacking ailments keep `ailment::AIL.overcome`, a share of full life a stack.)
pub const OVERCOME_REMAINING: f64 = 1.0;

/// Whether the ailment deals damage over time as instances (burn and bleed).
pub fn is_dot(a: Ailment) -> bool {
    matches!(a, Ailment::Burn | Ailment::Bleed)
}

/// The share of the blow's part an instance deals a second (0 for an ailment that is not damage over time).
pub fn rate(a: Ailment) -> f64 {
    match a {
        Ailment::Burn => BURN_RATE,
        Ailment::Bleed => BLEED_RATE,
        _ => 0.0,
    }
}

/// An instance's base time, before the ailment duration (0 for an ailment that is not damage over time).
pub fn base_time(a: Ailment) -> f64 {
    match a {
        Ailment::Burn => BURN_TIME,
        Ailment::Bleed => BLEED_TIME,
        _ => 0.0,
    }
}

/// The fire tier's factor (a tier past black counts as black).
pub fn tier_factor(tier: u8) -> f64 {
    FIRE_TIER[usize::from(tier.min(3))]
}

/// The striker's damage-over-time modifiers for one ailment, gathered in `build` from its table: what step 12 reads
/// in place of step 4's hit modifiers. The per-kind increases and mores (`DamageOf`) are the hit's own
/// (`Hit::increased_by`, `Hit::more_by`), read for every kind a part has been.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DotScale {
    /// Σ increased of `Damage`, `DotDamage` and `AilmentDamage(a)`.
    pub increased: f64,
    /// Π more of the same.
    pub more: f64,
    /// The damage-over-time multiplier: `1 + DotMulti + DotMultiOf(the ailment's kind)`.
    pub multi: f64,
}

impl Default for DotScale {
    /// Nothing: no increase, no more, a multiplier of 1.
    fn default() -> Self {
        DotScale {
            increased: 0.0,
            more: 1.0,
            multi: 1.0,
        }
    }
}

impl DotScale {
    /// Ailment `a`'s scale from a striker's gathered table.
    pub fn of(t: &Table, a: Ailment) -> DotScale {
        let acc = t.combined(&[Stat::Damage, Stat::DotDamage, Stat::AilmentDamage(a)]);
        DotScale {
            increased: acc.increased,
            more: acc.more,
            multi: t
                .combined(&[Stat::DotMulti, Stat::DotMultiOf(a.element())])
                .apply(1.0)
                .max(0.0),
        }
    }
}

/// One burn or bleed on a fighter: its damage a second as snapshot when laid (before mitigation and before the
/// moving factor), the seconds it has left and its full time, and the fire's tier it was laid with.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Dot {
    pub ailment: Ailment,
    pub dps: f64,
    pub left: f64,
    pub time: f64,
    pub tier: u8,
}

impl Dot {
    /// An instance of `a` dealing `dps` for `time` seconds, red fire.
    pub fn new(a: Ailment, dps: f64, time: f64) -> Dot {
        Dot {
            ailment: a,
            dps: dps.max(0.0),
            left: time,
            time,
            tier: 0,
        }
    }

    /// The kind of damage it deals (fire for a burn, force for a bleed).
    pub fn kind(&self) -> Element {
        self.ailment.element()
    }
}

/// The instances of one ailment, in the order laid.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Instances(pub Vec<Dot>);

impl Instances {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The strongest by damage a second (the first laid among equals).
    pub fn strongest(&self) -> Option<&Dot> {
        self.0
            .iter()
            .fold(None, |best: Option<&Dot>, d| match best {
                Some(b) if b.dps >= d.dps => Some(b),
                _ => Some(d),
            })
    }

    /// Lays `d`. Under [`DOT_CAP`] it joins; at the cap it replaces the weakest (the oldest among equals) when it is
    /// at least as strong, else it is dropped. Returns whether this made the count reach the cap (the full).
    pub fn lay(&mut self, d: Dot) -> bool {
        let before = self.0.len();
        if before < DOT_CAP {
            self.0.push(d);
            return self.0.len() >= DOT_CAP;
        }
        let weakest =
            self.0
                .iter()
                .enumerate()
                .fold(None, |w: Option<(usize, f64)>, (i, x)| match w {
                    Some((_, v)) if v <= x.dps => w,
                    _ => Some((i, x.dps)),
                });
        if let Some((i, v)) = weakest
            && d.dps >= v
        {
            self.0.remove(i);
            self.0.push(d);
        }
        false
    }

    /// Removes up to `n` of the earliest laid; returns how many.
    pub fn remove(&mut self, n: usize) -> usize {
        let k = n.min(self.0.len());
        self.0.drain(..k);
        k
    }

    /// The damage they would still deal, before mitigation, if left to run out: [`Instances::damage`] over the
    /// longest time any has left. For burn, only the strongest standing at each moment (the strongest's `dps × left`,
    /// then whichever takes over when it runs out); for bleed, every one's `dps × left`. The moving factor is not
    /// counted (whether the bearer would have moved is not known).
    pub fn remaining(&self) -> f64 {
        let longest = self.0.iter().map(|d| d.left).fold(0.0, f64::max);
        self.damage(longest, false)
    }

    /// Time passes: every instance runs down, the spent ones go.
    pub fn tick(&mut self, dt: f64) {
        for d in &mut self.0 {
            d.left -= dt;
        }
        self.0.retain(|d| d.left > 0.0);
    }

    /// The damage they deal over the next `dt` seconds before mitigation, each for no longer than it has left:
    /// every one (bleed), or only the strongest standing at each moment (burn: when it runs out, the next strongest
    /// takes over for the rest of `dt`). `moving` multiplies a bleed by [`BLEED_MOVING`].
    pub fn damage(&self, dt: f64, moving: bool) -> f64 {
        let Some(first) = self.0.first() else {
            return 0.0;
        };
        match first.ailment {
            Ailment::Burn => {
                let mut t = 0.0;
                let mut sum = 0.0;
                while t < dt {
                    let top =
                        self.0
                            .iter()
                            .filter(|d| d.left > t)
                            .fold(None, |best: Option<&Dot>, d| match best {
                                Some(b) if b.dps >= d.dps => Some(b),
                                _ => Some(d),
                            });
                    let Some(top) = top else {
                        break;
                    };
                    let span = (top.left - t).min(dt - t);
                    sum += top.dps * span;
                    t += span;
                }
                sum
            }
            _ => {
                let moved = if moving && first.ailment == Ailment::Bleed {
                    BLEED_MOVING
                } else {
                    1.0
                };
                self.0
                    .iter()
                    .map(|d| d.dps * d.left.min(dt).max(0.0))
                    .sum::<f64>()
                    * moved
            }
        }
    }
}

/// The share of damage over time of kind `e` that `bearer`, with table `t`, takes: `(1 − resistance)` for every
/// kind but force (capped by `ResistCap`, floored at `BASE.resist_floor`, no penetration), times `taken`,
/// `Taken`, `TakenOf(e)` and `DotTaken`. An element the bearer is immune to deals nothing.
pub fn mitigation(bearer: &Cultivator, t: &Table, e: Element) -> f64 {
    if bearer.immune == Some(e) {
        return 0.0;
    }
    let cut = match e {
        Element::Force => 1.0,
        _ => {
            let cap = t.get(Stat::ResistCap(e), BASE.resist_cap);
            let r = t
                .get(Stat::Resist(e), bearer.resist[e.index()])
                .clamp(BASE.resist_floor, cap.max(BASE.resist_floor));
            1.0 - r
        }
    };
    (cut * bearer.taken
        * t.get(Stat::Taken, 1.0)
        * t.get(Stat::TakenOf(e), 1.0)
        * t.get(Stat::DotTaken, 1.0))
    .max(0.0)
}

/// One ailment's damage over one tick, after mitigation.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct DotTick {
    pub ailment: Ailment,
    pub kind: Element,
    pub to_shield: f64,
    pub to_life: f64,
}

impl DotTick {
    /// What it took off the bars.
    pub fn total(&self) -> f64 {
        self.to_shield + self.to_life
    }
}

/// What a fighter's tick dealt it: each damaging ailment's tick (in [`Ailment::ALL`]'s order, burn and bleed only,
/// those that dealt something), the life Tâm ma took, and the life lost in all.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Ticked {
    pub dots: Vec<DotTick>,
    pub demon: f64,
    /// Life lost to damage over time and Tâm ma together (the shield's share of a bleed not counted).
    pub lost: f64,
}

impl Ticked {
    /// The tick of one ailment, if it dealt something.
    pub fn of(&self, a: Ailment) -> Option<&DotTick> {
        self.dots.iter().find(|d| d.ailment == a)
    }
}
