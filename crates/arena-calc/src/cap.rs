//! The one-cast cap (rules, "One cast, 40% of a life"): what a single cast of a skill deals at most. One cast's
//! damage at hit plus the damage over time that cast lays, run out over its full duration (its burns and bleeds, its
//! curse's strikes), against a base fighter at full life with no shield, no resistance and no evasion (its base
//! armour stands), must stay at or under [`CAST_CAP`] of a life and its shield ([`cap_life`]: 1440 of 3600). It
//! stops one-shot bursts; it is not a pacing rule. It is a rule of the data, held by a test over the pool's skills as
//! they stand, bare (`tests/cap.rs`), never a clamp in the fight. Supported forms are measured
//! (`arena-sim`'s `cap` example) but not held to it yet.
//!
//! [`one_cast`] works it out through the damage system's own code, as the fight casts: the skill built at
//! `gem::BASE_LEVEL` by a base striker wearing nothing (`build::build_skill`), its blows (`Effect::Hits` times its
//! repeats, a fraction of a blow last at that fraction; extra projectiles never strike one target twice) resolved
//! one after the other [`PULSE`] apart (`hit::resolve`), the target's time running between them and after the last
//! until every burn, bleed and curse it was given has run out (`Cultivator::tick`, `curse::Curses::tick`). A crit
//! and double damage count at their chances (the expected cast); an instant never crits.
//!
//! What a cast makes on the field counts as its first blows: a trap, a mine, a totem or a minion strikes once with
//! each of the cast's blows, times how many the cast makes and their damage factor (`device::spawn`); a totem's or a
//! minion's later blows are casts of their own. Lưu Lực's stored force is added at its share. A triggered cast is
//! built at its trigger's level and factor, never above the cast it copies (`tests/cap.rs` holds that).

use crate::ailment::Ailment;
use crate::build::{Built, Situation, Skill, build_skill};
use crate::cultivator::{BASE, Cultivator};
use crate::curse::{self, Curse, Curses};
use crate::device;
use crate::element::Element;
use crate::gem::{self, BASE_LEVEL, Effect, Gem};
use crate::hit::{self, Hit, Rolls};
use crate::sheet::EMPTY;
use crate::support::{Behaviour, SUPPORTS, Support};

/// One cast deals at most this share of a life.
pub const CAST_CAP: f64 = 0.4;
/// Seconds between one cast's blows (the fight's pulse).
pub const PULSE: f64 = 0.15;
/// The longest the cast's damage over time is followed, in seconds.
const RUN_OUT: f64 = 60.0;

/// What the cap is a share of: a base fighter's life and shield.
pub fn cap_life() -> f64 {
    BASE.life + BASE.shield
}

/// What one cast deals: at hit, and over time after (damage, not a share).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Dealt {
    pub hit: f64,
    pub over_time: f64,
}

impl Dealt {
    /// The whole of it.
    pub fn total(&self) -> f64 {
        self.hit + self.over_time
    }

    /// The whole of it as a share of `of` (the cap's: [`cap_life`]).
    pub fn share(&self, of: f64) -> f64 {
        if of > 0.0 { self.total() / of } else { 0.0 }
    }

    /// Whether it is over [`CAST_CAP`] of `of`.
    pub fn over(&self, of: f64) -> bool {
        self.share(of) > CAST_CAP + 1e-9
    }
}

/// The fighter the cap is measured on: the base, full, no shield, no resistance (the base's), no evasion.
pub fn bare_target() -> Cultivator {
    let mut c = Cultivator::new();
    c.shield = 0.0;
    c.shield_max = 0.0;
    c.evasion = 0.0;
    c
}

/// How many times the cast's base lands (`Effect::Hits`, else 1).
fn hits(effects: &[Effect]) -> f64 {
    effects
        .iter()
        .find_map(|e| match e {
            Effect::Hits(n) => Some(*n),
            _ => None,
        })
        .unwrap_or(1.0)
}

/// `hit` scaled by `factor` (its parts and its damage over time), as a fraction of a blow or a unit's damage.
fn scale(hit: &mut Hit, factor: f64) {
    if (factor - 1.0).abs() > 1e-12 {
        hit.more.push(factor - 1.0);
        for d in &mut hit.dot {
            d.more *= factor;
        }
    }
}

/// Whether a gem deals damage in a cast (a split, or a curse that strikes): what the cap is read for.
pub fn damaging(g: &Gem) -> bool {
    g.deals_damage() || gem::curse_of(g).is_some()
}

/// `g` as the fight casts it: at `BASE_LEVEL`, by a base striker's might.
pub fn skill_of(g: &Gem) -> Skill {
    Skill::of(g, BASE_LEVEL, BASE.might)
}

/// The supports of the shared pool that fit `skill`, but Dung (which needs a partner: `support::fuse`).
pub fn fitting(skill: &Skill) -> Vec<&'static Support> {
    let tags = skill.all_tags();
    SUPPORTS
        .iter()
        .filter(|x| x.fits(tags) && !x.is_fuse())
        .collect()
}

/// What one cast of `g` with `supports` deals (see the module).
pub fn cast_of(g: &Gem, supports: &[&Support]) -> Dealt {
    one_cast(&skill_of(g), supports, gem::curse_of(g))
}

/// What one cast of `skill` with `supports` deals, expected over its crit and double damage (see the module),
/// laying `curse` when it is a curse skill.
pub fn one_cast(skill: &Skill, supports: &[&Support], curse: Option<curse::Kind>) -> Dealt {
    let striker = Cultivator::new();
    let target = bare_target();
    let sit = Situation {
        target: Some(&target),
        ..Situation::default()
    };
    let built = build_skill(skill, supports, &striker, &EMPTY, &sit);
    let h = &built.hit;
    let crit = if h.instant {
        0.0
    } else {
        h.crit.clamp(0.0, 1.0)
    };
    let double = h.double.clamp(0.0, 1.0);
    let mut out = Dealt::default();
    for (c, pc) in [(false, 1.0 - crit), (true, crit)] {
        for (d, pd) in [(false, 1.0 - double), (true, double)] {
            let p = pc * pd;
            if p <= 0.0 {
                continue;
            }
            let rolls = Rolls {
                crit: if c { 0.0 } else { 1.0 },
                double: if d { 0.0 } else { 1.0 },
                ..Rolls::NONE
            };
            let s = run(skill, &built, curse, &striker, rolls);
            out.hit += p * s.hit;
            out.over_time += p * s.over_time;
        }
    }
    out
}

/// One cast with fixed `rolls`.
fn run(
    skill: &Skill,
    built: &Built,
    curse: Option<curse::Kind>,
    striker: &Cultivator,
    rolls: Rolls,
) -> Dealt {
    let mut target = bare_target();
    let mut curses = Curses::new(1, false);
    let mut theirs = Curses::new(0, false);
    let mut at_hit = 0.0;
    let mut later = 0.0;
    if let Some(kind) = curse {
        let effect = curse::curse_effect(striker, &EMPTY);
        curse::cast_with(
            Curse::new(kind, built.rank, 0, 1),
            &mut curses,
            &mut theirs,
            effect,
        );
        curses.sync(&mut target, false);
    }
    let unit = device::spawn(built).map_or(1.0, |s| f64::from(s.count) * s.damage);
    let stored: f64 = built
        .behaviours
        .iter()
        .map(|b| match b {
            Behaviour::StoredForce { share, .. } => *share,
            _ => 0.0,
        })
        .sum();
    let total = if skill.split.is_empty() || skill.base <= 0.0 {
        0.0
    } else {
        hits(skill.effects) * f64::from(built.strikes)
    };
    let count = libm::ceil(total - 1e-9).max(0.0) as usize;
    let frac = total - (count as f64 - 1.0);
    let pass = |target: &mut Cultivator, curses: &mut Curses, dt: f64| -> f64 {
        let mut lost = target.tick(dt, false);
        for e in curses.tick(dt, target, false) {
            if let curse::Event::Strike { element, share, .. } = e {
                let h = curse::strike_hit(element, share, target.life_max);
                let o = hit::resolve(&h, None, target, Rolls::NONE);
                lost += o.to_shield + o.to_life + o.to_sense;
            }
        }
        lost
    };
    for i in 0..count {
        if target.life <= 0.0 {
            break;
        }
        let mut h = built.hit.clone();
        if i + 1 == count && frac < 1.0 - 1e-9 {
            scale(&mut h, frac.max(0.0));
        }
        scale(&mut h, unit);
        let o = hit::resolve(&h, Some(striker), &mut target, rolls);
        at_hit += o.to_shield + o.to_life + o.to_sense;
        at_hit += stored * o.dealt[Element::Force.index()];
        if i + 1 < count {
            later += pass(&mut target, &mut curses, PULSE);
        }
    }
    let mut t = 0.0;
    while t < RUN_OUT && target.life > 0.0 && (dots_left(&target) || !curses.on.is_empty()) {
        later += pass(&mut target, &mut curses, PULSE);
        t += PULSE;
    }
    Dealt {
        hit: at_hit,
        over_time: later,
    }
}

/// Whether the target still carries a burn or a bleed.
fn dots_left(c: &Cultivator) -> bool {
    [Ailment::Burn, Ailment::Bleed]
        .into_iter()
        .any(|a| c.ailments.dots(a).is_some_and(|d| !d.is_empty()))
}
