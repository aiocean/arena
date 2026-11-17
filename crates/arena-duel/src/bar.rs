//! The rotating skill bar: three slots drawn from the loadout's actives (those cast by hand), spending a slot, what
//! a fighter can cast now, and what the HUD shows of the bar and of a fighter's buffs. The bar's tiers, traits,
//! luck and evolving pairs are off while the damage system is balanced (design 2): every draw is a plain skill.

use crate::data::{BAR, Kind, RAGE, STALE};
use crate::fighter::{Fighter, Slot, State};
use crate::js::js_min;
use crate::kit::Place;
use crate::strike::can_pay;
use arena_calc::ailment::Ailment;
use arena_calc::gem;
use arena_calc::tag::Tag;

/// A die of `n` faces, 0-based: `Math.floor(rng() * n)`.
fn pick(state: &mut State, n: usize) -> usize {
    let i = libm::floor(state.rng.next() * n as f64) as usize;
    i.min(n.saturating_sub(1))
}

/// The bar slot of fighter `f` holding `name`.
pub fn slot_of<'a>(f: &'a Fighter, name: &str) -> Option<&'a Slot> {
    f.bar.iter().find(|slot| slot.spell() == Some(name))
}

/// Can `f` cast `name` now: in its kit and not cast by a trigger, its effect in the registry, on its bar if it is a
/// bar skill, a use stored, a card left, payable, the ultimate only on full rage, a spell not while its soul is lost,
/// a blink not while sealed or rooted, a guard skill not while barred, nothing its curses bar, an intent not on
/// already, a skill that binds not while its last blow plays out.
pub fn ready(f: &Fighter, name: &str) -> bool {
    let Some(k) = f.kit.get(name) else {
        return false;
    };
    let s = k.motion;
    !k.triggered
        && k.usable
        && (!matches!(k.place, Place::Active(_)) || slot_of(f, name).is_some())
        && k.uses.ready()
        && k.card != Some(0)
        && (k.place != Place::Ultimate || f.rage >= RAGE)
        && !(f.silenced() && k.tags.has(Tag::Spell))
        && !((s.kind == Kind::Blink || k.tags.has(Tag::Movement))
            && (f.sealed > 0.0 || f.root > 0.0))
        && !(k.tags.has(Tag::Guard) && f.no_guard > 0.0)
        && !f.curses.barred(k.tags)
        && !(f.bound && k.gem.binds())
        && !gem::aura_of(k.gem).is_some_and(|a| f.auras.on.iter().any(|o| o.kind == a))
        && can_pay(&f.c, &k.costs())
}

/// Put `slot` at `i` of fighter `f`'s bar (one past the end grows it, as a JS array does).
fn put(f: &mut Fighter, i: usize, slot: Slot) {
    match f.bar.get_mut(i) {
        Some(s) => *s = slot,
        None => f.bar.push(slot),
    }
}

/// Fill slot `i` of fighter `f`'s bar with one of its actives not on the bar, at random by the rules' dice; a stale
/// skill's replacement is never that skill (`gone`). With none left it waits.
pub fn draw(state: &mut State, f: usize, i: usize, gone: Option<&str>) {
    let me = &state.fighters[f];
    let held: Vec<&'static str> = me
        .bar
        .iter()
        .enumerate()
        .filter(|&(j, _)| j != i)
        .filter_map(|(_, slot)| slot.spell())
        .collect();
    let choice: Vec<&'static str> = me
        .pool
        .iter()
        .copied()
        .filter(|&name| Some(name) != gone && !held.contains(&name))
        .collect();
    let slot = if choice.is_empty() {
        Slot::Empty {
            left: BAR.wait,
            full: BAR.wait,
        }
    } else {
        let name = choice[pick(state, choice.len())];
        Slot::Held {
            spell: name,
            idle: 0.0,
        }
    };
    put(&mut state.fighters[f], i, slot);
}

/// The cast of slot `i`'s skill empties the slot, which counts down before it draws again.
pub fn spend(state: &mut State, f: usize, i: usize) {
    if state.fighters[f].bar.get(i).and_then(Slot::spell).is_none() {
        return;
    }
    let wait = BAR.wait + BAR.spread * state.rng.next();
    state.fighters[f].bar[i] = Slot::Empty {
        left: wait,
        full: wait,
    };
}

/// A slot of the bar as the screen shows it.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct SlotView {
    pub spell: Option<&'static str>,
    pub empty: bool,
    pub ultimate: bool,
    pub ready: bool,
    /// The cooldown (or the empty slot's wait) left, out of `full`.
    pub left: f64,
    pub full: f64,
    /// How charged it is: an ultimate charges with rage.
    pub charge: f64,
    /// Its uses stored, of its most (`arena_calc::charge::Uses`).
    pub uses: u8,
    pub max_uses: u8,
    /// How near it is to leaving the bar unused, 0 to 1.
    pub stale: f64,
}

const NO_SLOT: SlotView = SlotView {
    spell: None,
    empty: true,
    ultimate: false,
    ready: false,
    left: 0.0,
    full: 0.0,
    charge: 1.0,
    uses: 0,
    max_uses: 0,
    stale: 0.0,
};

/// The cooldown view of `f`'s skill `name`: the time to its next use (0 with one stored) and its whole cooldown, its
/// uses stored and their most.
fn cooling(f: &Fighter, name: &str) -> (f64, f64, u8, u8) {
    f.kit.get(name).map_or((0.0, 0.0, 0, 0), |k| {
        let u = k.uses;
        let left = if u.stored > 0 { 0.0 } else { u.recharge_left };
        (left, u.cooldown, u.stored, u.max)
    })
}

/// What fighter `f`'s skill bar shows, slot by slot (1, 2, 3, then the ultimate).
pub fn skill_bar(f: &Fighter) -> Vec<SlotView> {
    let mut slots: Vec<SlotView> = f
        .bar
        .iter()
        .map(|slot| match slot {
            Slot::Empty { left, full } => SlotView {
                left: *left,
                full: *full,
                ..NO_SLOT
            },
            Slot::Held { spell, idle } => {
                let (left, full, uses, max_uses) = cooling(f, spell);
                SlotView {
                    spell: Some(spell),
                    empty: false,
                    ready: f.alive() && ready(f, spell),
                    left,
                    full,
                    uses,
                    max_uses,
                    stale: js_min(1.0, idle / STALE),
                    ..NO_SLOT
                }
            }
        })
        .collect();
    while slots.len() < BAR.slots {
        slots.push(NO_SLOT);
    }
    let ult = f
        .kit
        .skills
        .iter()
        .find(|k| k.place == Place::Ultimate)
        .map(|k| k.gem.id);
    let (left, full, uses, max_uses) = ult.map_or((0.0, 0.0, 0, 0), |u| cooling(f, u));
    slots.push(SlotView {
        spell: ult,
        empty: ult.is_none(),
        ultimate: true,
        ready: ult.is_some_and(|u| f.alive() && ready(f, u)),
        left,
        full,
        uses,
        max_uses,
        charge: f.rage / RAGE,
        ..NO_SLOT
    });
    slots
}

/// A buff or debuff the HUD shows under a fighter's bars, with its stacks (0: none to show).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Buff {
    pub id: &'static str,
    pub good: bool,
    pub stacks: f64,
}

/// What the HUD shows under fighter `f`'s bars, good first, then bad: its keystones' states, its ward, barrier and
/// grit, then each ailment on it (by its id, `Ailment::name`) and each control. Nothing once it is knocked out.
pub fn buffs_of(f: &Fighter) -> Vec<Buff> {
    if !f.alive() {
        return Vec::new();
    }
    use arena_calc::keystone::Keystone;
    let mut out: Vec<Buff> = [
        (
            "fury",
            f.has(Keystone::Fury) && f.fury > 0,
            f64::from(f.fury),
        ),
        (
            "unyielding",
            f.has(Keystone::Unyielding) && !f.unyielding_used,
            0.0,
        ),
        ("attune", f.has(Keystone::Attuned), f64::from(f.attuned)),
        ("ward", f.charges > 0.0, f.charges),
        ("barrier", f.barrier > 0.0, 0.0),
        ("grit", f.c.grit >= 1.0, libm::floor(f.c.grit)),
        ("souls", f.c.souls >= 1.0, libm::floor(f.c.souls)),
    ]
    .into_iter()
    .filter(|&(_, on, _)| on)
    .map(|(id, _, stacks)| Buff {
        id,
        good: true,
        stacks,
    })
    .collect();
    for a in Ailment::ALL {
        let n = f.c.ailments.stacks(a);
        if n > 0.0 {
            out.push(Buff {
                id: a.name(),
                good: false,
                stacks: libm::floor(n),
            });
        }
    }
    for (id, on) in [
        ("silence", f.silenced()),
        ("root", f.root > 0.0),
        ("sealed", f.sealed > 0.0),
        ("cursed", !f.curses.on.is_empty()),
    ] {
        if on {
            out.push(Buff {
                id,
                good: false,
                stacks: 0.0,
            });
        }
    }
    out
}
