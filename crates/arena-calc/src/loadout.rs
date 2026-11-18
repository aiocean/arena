//! Bộ đồ: what a player builds from the shared pool (design, 2). Exactly [`ACTIVES`] distinct skills (no basic),
//! each with up to [`SUPPORTS_PER_SKILL`] supports and [`SUPPORTS_MAX`] in all, at most [`FAITH_MAX`] of rank Tín
//! Niệm, up to [`KEYSTONES_MAX`] keystones, and up to one root, within a [`BUDGET`] of points. [`check`] is the one
//! judge of a legal loadout, for training, the ladder and admitting a bot; [`random`] draws a legal one for training.

use crate::build::{Skill, kinds};
use crate::element::{Element, N, is_root};
use crate::gem::{GEMS, Gem};
use crate::grade::Rank;
use crate::keystone::Keystone;
use crate::support::{SUPPORTS, Support, excluded, support};

/// Skills a loadout holds besides the basics.
pub const ACTIVES: usize = 8;
/// Skills of rank Tín Niệm (`Rank::Faith`, the top tier's đỉnh cấp skills) a loadout may hold at most.
pub const FAITH_MAX: usize = 1;
/// Supports on one skill, and in the whole loadout.
pub const SUPPORTS_PER_SKILL: usize = 3;
pub const SUPPORTS_MAX: usize = 12;
/// Keystones at most, and what each costs.
pub const KEYSTONES_MAX: usize = 2;
pub const KEYSTONE_POINTS: u32 = 4;
/// The loadout's points at most.
pub const BUDGET: u32 = 50;
/// The one basic every fighter has, which no loadout lists: độn, the blink. Nothing that deals damage is given:
/// every blow a fighter strikes is one its loadout chose (owner, 2026-11-18).
pub const BASICS: [&str; 1] = ["blink"];
/// The roots a fighter may take: the five phases, light and yin.
pub const ROOTS: [Element; 7] = [
    Element::Metal,
    Element::Wood,
    Element::Water,
    Element::Fire,
    Element::Earth,
    Element::Light,
    Element::Yin,
];

/// One skill of a loadout and the supports hung on it.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Slot {
    /// The skill's code name.
    pub skill: String,
    /// The supports' code names.
    pub supports: Vec<String>,
    /// With Dung: the other skill of the loadout this one is fused with.
    pub fuse: Option<String>,
}

/// A loadout.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Loadout {
    pub actives: Vec<Slot>,
    pub keystones: Vec<Keystone>,
    /// Bản mệnh.
    pub root: Option<Element>,
}

/// A rule a loadout breaks.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase", tag = "type"))]
pub enum LoadoutError {
    /// Not exactly [`ACTIVES`] skills.
    Actives { count: usize },
    /// A skill listed twice.
    DuplicateActive { skill: String },
    /// No such skill in the pool.
    UnknownSkill { skill: String },
    /// A basic among the actives.
    NotActive { skill: String },
    /// More than [`FAITH_MAX`] skills of rank Tín Niệm.
    Faith { count: usize },
    /// More than [`SUPPORTS_PER_SKILL`] on one skill.
    SlotSupports { skill: String, count: usize },
    /// More than [`SUPPORTS_MAX`] in all.
    Supports { count: usize },
    /// One support twice on one skill.
    DuplicateSupport { skill: String, support: String },
    /// No such support.
    UnknownSupport { support: String },
    /// The support needs tags the skill lacks.
    SupportTags { skill: String, support: String },
    /// Two supports that exclude each other on one skill.
    Excluded { skill: String, a: String, b: String },
    /// Dung without a skill to fuse with.
    FuseMissing { skill: String },
    /// Dung naming a skill that is not another active of the loadout.
    FusePartner { skill: String, partner: String },
    /// A skill to fuse with but no Dung.
    FuseUnneeded { skill: String },
    /// More than [`KEYSTONES_MAX`].
    Keystones { count: usize },
    /// One keystone twice.
    DuplicateKeystone { keystone: Keystone },
    /// A keystone forbids a support on a skill.
    KeystoneSupport {
        keystone: Keystone,
        skill: String,
        support: String,
    },
    /// A keystone forbids a skill.
    KeystoneSkill { keystone: Keystone, skill: String },
    /// A keystone needs a root.
    KeystoneRoot { keystone: Keystone },
    /// A keystone forbids the loadout's kinds of damage (Thái Cổ Lôi Long).
    KeystoneKinds { keystone: Keystone },
    /// A root that is not a phase, light or yin.
    Root { root: Element },
    /// Over [`BUDGET`].
    Budget { points: u32 },
}

/// A legal loadout or every rule it breaks, against the pool [`GEMS`].
pub fn check(l: &Loadout) -> Result<(), Vec<LoadoutError>> {
    check_in(l, GEMS)
}

/// [`check`] against another pool.
pub fn check_in(l: &Loadout, pool: &[Gem]) -> Result<(), Vec<LoadoutError>> {
    use LoadoutError as E;
    let find = |id: &str| pool.iter().find(|g| g.id == id);
    let mut errs = Vec::new();

    if l.actives.len() != ACTIVES {
        errs.push(E::Actives {
            count: l.actives.len(),
        });
    }
    let total: usize = l.actives.iter().map(|s| s.supports.len()).sum();
    if total > SUPPORTS_MAX {
        errs.push(E::Supports { count: total });
    }
    if l.keystones.len() > KEYSTONES_MAX {
        errs.push(E::Keystones {
            count: l.keystones.len(),
        });
    }
    for (i, k) in l.keystones.iter().enumerate() {
        if l.keystones[..i].iter().any(|o| o.same(*k)) {
            errs.push(E::DuplicateKeystone { keystone: *k });
        }
        if k.needs_root() && l.root.is_none() {
            errs.push(E::KeystoneRoot { keystone: *k });
        }
    }
    if let Some(r) = l.root
        && !is_root(r)
    {
        errs.push(E::Root { root: r });
    }

    let mut loadout_kinds = [false; N];
    for (i, slot) in l.actives.iter().enumerate() {
        let name = || slot.skill.clone();
        if l.actives[..i].iter().any(|o| o.skill == slot.skill) {
            errs.push(E::DuplicateActive { skill: name() });
        }
        let Some(gem) = find(&slot.skill) else {
            errs.push(E::UnknownSkill { skill: name() });
            continue;
        };
        if BASICS.contains(&gem.id) {
            errs.push(E::NotActive { skill: name() });
        }
        if slot.supports.len() > SUPPORTS_PER_SKILL {
            errs.push(E::SlotSupports {
                skill: name(),
                count: slot.supports.len(),
            });
        }
        let tags = gem.all_tags();
        let mut known: Vec<&Support> = Vec::new();
        for (j, id) in slot.supports.iter().enumerate() {
            if slot.supports[..j].contains(id) {
                errs.push(E::DuplicateSupport {
                    skill: name(),
                    support: id.clone(),
                });
                continue;
            }
            let Some(s) = support(id) else {
                errs.push(E::UnknownSupport {
                    support: id.clone(),
                });
                continue;
            };
            if !s.fits(tags) {
                errs.push(E::SupportTags {
                    skill: name(),
                    support: id.clone(),
                });
            }
            for o in &known {
                if excluded(s, o) {
                    errs.push(E::Excluded {
                        skill: name(),
                        a: o.id.to_string(),
                        b: s.id.to_string(),
                    });
                }
            }
            for k in &l.keystones {
                if k.forbids_support(s) {
                    errs.push(E::KeystoneSupport {
                        keystone: *k,
                        skill: name(),
                        support: id.clone(),
                    });
                }
            }
            known.push(s);
        }
        let fused = known.iter().any(|s| s.is_fuse());
        match (&slot.fuse, fused) {
            (None, true) => errs.push(E::FuseMissing { skill: name() }),
            (Some(_), false) => errs.push(E::FuseUnneeded { skill: name() }),
            (Some(p), true) if *p == slot.skill || !l.actives.iter().any(|o| o.skill == *p) => errs
                .push(E::FusePartner {
                    skill: name(),
                    partner: p.clone(),
                }),
            _ => {}
        }
        let rank = Rank::of_skill(gem.rank, known.iter().any(|s| s.mods.rank_step), false);
        for k in &l.keystones {
            if k.forbids_skill(gem.tags(), rank) {
                errs.push(E::KeystoneSkill {
                    keystone: *k,
                    skill: name(),
                });
            }
        }
        let skill_kinds = kinds(&Skill::of(gem, 1, 0.0), &known);
        for (a, b) in loadout_kinds.iter_mut().zip(skill_kinds) {
            *a |= b;
        }
    }
    for k in &l.keystones {
        if !k.kinds_allowed(&loadout_kinds) {
            errs.push(E::KeystoneKinds { keystone: *k });
        }
    }

    let faith = l
        .actives
        .iter()
        .filter(|s| find(&s.skill).is_some_and(|g| g.rank == Rank::Faith))
        .count();
    if faith > FAITH_MAX {
        errs.push(E::Faith { count: faith });
    }
    let p = points_in(l, pool);
    if p > BUDGET {
        errs.push(E::Budget { points: p });
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// The loadout's points, against the pool [`GEMS`]: each active its tier's, each support its own, each keystone
/// [`KEYSTONE_POINTS`], the root none. Unknown names count nothing.
pub fn points(l: &Loadout) -> u32 {
    points_in(l, GEMS)
}

/// [`points`] against another pool.
pub fn points_in(l: &Loadout, pool: &[Gem]) -> u32 {
    let actives: u32 = l
        .actives
        .iter()
        .filter_map(|s| pool.iter().find(|g| g.id == s.skill))
        .map(|g| g.tier.points())
        .sum();
    let supports: u32 = l
        .actives
        .iter()
        .flat_map(|s| s.supports.iter())
        .filter_map(|id| support(id))
        .map(|s| s.points)
        .sum();
    actives + supports + KEYSTONE_POINTS * l.keystones.len() as u32
}

/// A legal loadout drawn from [`GEMS`] on `roll` (each call a number in [0, 1)), or `None` when the pool cannot make
/// one. The same rolls draw the same loadout.
pub fn random(roll: &mut impl FnMut() -> f64) -> Option<Loadout> {
    random_in(GEMS, roll)
}

/// One of `n` on a roll.
fn pick(n: usize, r: f64) -> usize {
    if n == 0 {
        return 0;
    }
    ((r.clamp(0.0, 1.0) * n as f64) as usize).min(n - 1)
}

/// [`random`] from another pool. Draws the keystones (0..=2), the root, the actives (each within what
/// the budget leaves for the rest), then 0..=3 supports a skill; falls back to no keystones when they leave too
/// few skills.
pub fn random_in(pool: &[Gem], roll: &mut impl FnMut() -> f64) -> Option<Loadout> {
    let mut keystones = Vec::new();
    for _ in 0..pick(KEYSTONES_MAX + 1, roll()) {
        let free: Vec<Keystone> = Keystone::ALL
            .iter()
            .copied()
            .filter(|k| !keystones.iter().any(|o: &Keystone| o.same(*k)))
            .collect();
        let mut k = free[pick(free.len(), roll())];
        if let Keystone::Extreme(_) = k {
            k = Keystone::Extreme(Element::ALL[pick(N, roll())]);
        }
        keystones.push(k);
    }
    let root = if keystones.iter().any(|k| k.needs_root()) {
        Some(ROOTS[pick(ROOTS.len(), roll())])
    } else {
        let i = pick(ROOTS.len() + 1, roll());
        i.checked_sub(1).map(|i| ROOTS[i])
    };
    let mut l = draw(pool, keystones, root, roll).or_else(|| draw(pool, Vec::new(), root, roll))?;
    // a Cực Cảnh takes a kind the loadout deals (its actives with their supports, and the basics): one on a kind it
    // never deals is four dead points
    let dealt = dealt_kinds(&l, pool);
    let kinds: Vec<Element> = Element::ALL
        .into_iter()
        .filter(|e| dealt[e.index()])
        .collect();
    for k in &mut l.keystones {
        if let Keystone::Extreme(e) = *k
            && !dealt[e.index()]
            && !kinds.is_empty()
        {
            *k = Keystone::Extreme(kinds[pick(kinds.len(), roll())]);
        }
    }
    Some(l)
}

/// The kinds of damage loadout `l` deals, as a row by [`Element::index`]: its actives with their fitting supports,
/// and the basics every loadout holds.
pub fn dealt_kinds(l: &Loadout, pool: &[Gem]) -> [bool; N] {
    let mut has = [false; N];
    let ids = l
        .actives
        .iter()
        .map(|s| (s.skill.as_str(), s.supports.as_slice()))
        .chain(BASICS.iter().map(|b| (*b, &[][..])));
    for (id, supports) in ids {
        let Some(g) = pool.iter().chain(GEMS).find(|g| g.id == id) else {
            continue;
        };
        let on: Vec<&Support> = supports.iter().filter_map(|x| support(x)).collect();
        for (a, b) in has.iter_mut().zip(kinds(&Skill::of(g, 1, 0.0), &on)) {
            *a |= b;
        }
    }
    has
}

fn draw(
    pool: &[Gem],
    keystones: Vec<Keystone>,
    root: Option<Element>,
    roll: &mut impl FnMut() -> f64,
) -> Option<Loadout> {
    let allowed = |g: &Gem, rank: Rank| !keystones.iter().any(|k| k.forbids_skill(g.tags(), rank));
    let mut left = BUDGET.checked_sub(KEYSTONE_POINTS * keystones.len() as u32)?;
    let mut kinds_so_far = [false; N];
    let mut chosen: Vec<&Gem> = Vec::new();
    for i in 0..ACTIVES {
        let fits = |g: &&Gem| {
            !BASICS.contains(&g.id)
                && !chosen.iter().any(|c| c.id == g.id)
                && !(g.rank == Rank::Faith
                    && chosen.iter().filter(|c| c.rank == Rank::Faith).count() >= FAITH_MAX)
                && allowed(g, g.rank)
                && {
                    let mut k = kinds_so_far;
                    for (a, b) in k.iter_mut().zip(kinds(&Skill::of(g, 1, 0.0), &[])) {
                        *a |= b;
                    }
                    keystones.iter().all(|ks| ks.kinds_allowed(&k))
                }
        };
        let open: Vec<&Gem> = pool.iter().filter(fits).collect();
        let cheapest = open.iter().map(|g| g.tier.points()).min()?;
        let reserve = cheapest * (ACTIVES - i - 1) as u32;
        let can: Vec<&Gem> = open
            .into_iter()
            .filter(|g| g.tier.points() + reserve <= left)
            .collect();
        if can.is_empty() {
            return None;
        }
        let g = can[pick(can.len(), roll())];
        left -= g.tier.points();
        for (a, b) in kinds_so_far
            .iter_mut()
            .zip(kinds(&Skill::of(g, 1, 0.0), &[]))
        {
            *a |= b;
        }
        chosen.push(g);
    }

    let mut total = 0;
    let mut actives = Vec::new();
    for (i, g) in chosen.iter().enumerate() {
        let mut on: Vec<&Support> = Vec::new();
        let mut fuse = None;
        for _ in 0..pick(SUPPORTS_PER_SKILL + 1, roll()) {
            if total >= SUPPORTS_MAX {
                break;
            }
            let tags = g.all_tags();
            let can: Vec<&Support> = SUPPORTS
                .iter()
                .filter(|s| {
                    s.fits(tags)
                        && s.points <= left
                        && !on.iter().any(|o| o.id == s.id || excluded(s, o))
                        && !keystones.iter().any(|k| k.forbids_support(s))
                        && allowed(g, Rank::of_skill(g.rank, s.mods.rank_step, false))
                        && {
                            let mut k = kinds_so_far;
                            for e in s.kinds() {
                                k[e.index()] = true;
                            }
                            keystones.iter().all(|ks| ks.kinds_allowed(&k))
                        }
                })
                .collect();
            if can.is_empty() {
                break;
            }
            let s = can[pick(can.len(), roll())];
            if s.is_fuse() {
                let others: Vec<&&Gem> = chosen
                    .iter()
                    .enumerate()
                    .filter(|&(j, _)| j != i)
                    .map(|(_, o)| o)
                    .collect();
                fuse = Some(others[pick(others.len(), roll())].id.to_string());
            }
            left -= s.points;
            total += 1;
            for e in s.kinds() {
                kinds_so_far[e.index()] = true;
            }
            on.push(s);
        }
        actives.push(Slot {
            skill: g.id.to_string(),
            supports: on.iter().map(|s| s.id.to_string()).collect(),
            fuse,
        });
    }
    let l = Loadout {
        actives,
        keystones,
        root,
    };
    check_in(&l, pool).ok().map(|_| l)
}
