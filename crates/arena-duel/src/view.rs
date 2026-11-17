//! The duel as the page reads it: each fighter's body, bars, cast, skill bar and buffs, the units on the field, and
//! the round.

use crate::bar::{Buff, SlotView, buffs_of, skill_bar};
use crate::data::Look;
use crate::fighter::{Fighter, Phase, State};
use crate::pose::Pose;
use crate::style::Style;
use crate::unit::Unit;

/// The duel this tick.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct DuelView {
    pub time: f64,
    pub round: u32,
    pub wins: [u32; 2],
    pub winner: Option<usize>,
    pub left: f64,
    pub right: f64,
    pub ground: f64,
    pub fighters: [FighterView; 2],
    /// Traps, mines, totems and minions standing, in id order.
    pub units: Vec<UnitView>,
}

/// A fighter this tick. Bars are shares, 0 to 100.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct FighterView {
    pub index: usize,
    /// How it is drawn.
    pub look: Look,
    /// How it fights this fight, shown before the fight so a watcher can study it.
    pub style: Style,
    pub hp: f64,
    pub qi: f64,
    /// Hộ thân linh quang (energy shield) and thần thức (sense), as shares.
    pub shield: f64,
    pub sense: f64,
    pub souls: f64,
    pub x: f64,
    pub air: f64,
    pub vx: f64,
    pub vy: f64,
    pub face: f64,
    pub pose: Pose,
    pub pose_t: f64,
    pub flash: f64,
    pub hit_t: f64,
    pub hit_power: f64,
    pub shocked: f64,
    pub dodged: f64,
    pub rang: f64,
    pub charges: f64,
    pub barrier: f64,
    /// It is burning.
    pub burn: bool,
    pub act: Option<ActView>,
    pub untouchable: bool,
    pub skill_bar: Vec<SlotView>,
    pub buffs: Vec<Buff>,
}

/// A cast under way: its skill, phase and time in it.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ActView {
    pub spell: &'static str,
    pub phase: Phase,
    pub t: f64,
    /// The whole span of the phase (`act::phase_span`): the drawn body stretches the phase's frames over it.
    pub span: f64,
}

/// A unit this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct UnitView {
    pub id: u32,
    pub owner: usize,
    pub kind: arena_calc::device::Kind,
    pub skill: &'static str,
    pub x: f64,
    /// Its life as a share of its full life.
    pub hp: f64,
    /// It lies on the floor (a trap, a mine).
    pub floor: bool,
}

/// The duel in `state` as the page reads it.
pub fn view(state: &State) -> DuelView {
    DuelView {
        time: state.time,
        round: state.round,
        wins: state.wins,
        winner: state.winner,
        left: state.left,
        right: state.right,
        ground: state.ground,
        fighters: [
            fighter_view(&state.fighters[0]),
            fighter_view(&state.fighters[1]),
        ],
        units: state.units.iter().map(unit_view).collect(),
    }
}

fn share(v: f64, max: f64) -> f64 {
    if max > 0.0 {
        (v / max * 100.0).clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn unit_view(u: &Unit) -> UnitView {
    UnitView {
        id: u.id,
        owner: u.owner,
        kind: u.kind,
        skill: u.skill,
        x: u.x,
        hp: u.hp(),
        floor: u.on_floor(),
    }
}

fn fighter_view(f: &Fighter) -> FighterView {
    let b = &f.body;
    FighterView {
        index: f.index,
        look: f.look,
        style: f.style,
        hp: f.hp(),
        qi: f.qi(),
        shield: share(f.c.shield, f.c.shield_max),
        sense: share(f.c.sense, f.c.sense_max),
        souls: f.c.souls,
        x: b.x,
        air: b.air,
        vx: b.vx,
        vy: b.vy,
        face: b.face,
        pose: b.pose,
        pose_t: b.pose_t,
        flash: b.flash,
        hit_t: b.hit_t,
        hit_power: b.hit_power,
        shocked: b.shocked,
        dodged: f.dodged,
        rang: f.rang,
        charges: f.charges,
        barrier: f.barrier,
        burn: f.c.ailments.stacks(arena_calc::ailment::Ailment::Burn) > 0.0,
        act: f.act.map(|a| ActView {
            spell: a.spell,
            phase: a.phase,
            t: a.t,
            span: crate::act::phase_span(&a),
        }),
        untouchable: b.untouchable(),
        skill_bar: skill_bar(f),
        buffs: buffs_of(f),
    }
}
