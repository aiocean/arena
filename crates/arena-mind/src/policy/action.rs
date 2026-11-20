//! What the policy can do: nine plain acts, a cast from each slot at one of three aims, then (ABI 5) a long duck and
//! a long guard, `ACTIONS` in all, each with a fixed index (the long holds come last, so the older indices keep
//! their meaning); which of them can happen now (`legal`); and the `Choice` each one is.

use super::ahead;
use crate::choice::Choice;
use crate::sense::{Foe, Sense};
use crate::skill::{Skill, SkillKind};

/// The castable slots a game lays out (the duel: three for basics, the blink and two empty; three bar slots).
pub const SLOTS: usize = 6;
const PLAIN: usize = 9;
const AIMS: usize = 3;
/// The acts ABI 5 appended after the casts: a duck and a guard held long.
const LONG: usize = 2;
pub const ACTIONS: usize = PLAIN + SLOTS * AIMS + LONG;

/// How far ahead of or behind the foe an aimed cast lands, game pixels.
const AIM: f64 = 30.0;
/// How far from the foe a blink lands beside it.
const BESIDE: f64 = 20.0;
/// How high it flies, as a share of the ceiling.
const FLY: f64 = 0.6;
/// How long a duck or guard is held, seconds.
const HOLD: f64 = 0.4;
/// How long a long duck or guard is held, seconds (ABI 5): one decision that commits, where three short ones flicker.
const HOLD_LONG: f64 = 1.0;

/// Where a cast aims, seen from the fighter: at the foe, past it, or short of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Aim {
    Foe,
    Ahead,
    Behind,
}

/// One thing the policy can do. Directions are relative to the foe: `Toward` walks at it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Act {
    Wait,
    Toward,
    Away,
    Jump,
    Duck,
    Guard,
    FlyHigh,
    Land,
    Meditate,
    Cast {
        slot: usize,
        aim: Aim,
    },
    /// A duck held `HOLD_LONG` (ABI 5).
    DuckLong,
    /// A guard held `HOLD_LONG` (ABI 5).
    GuardLong,
}

const PLAINS: [Act; PLAIN] = [
    Act::Wait,
    Act::Toward,
    Act::Away,
    Act::Jump,
    Act::Duck,
    Act::Guard,
    Act::FlyHigh,
    Act::Land,
    Act::Meditate,
];
const AIMED: [Aim; AIMS] = [Aim::Foe, Aim::Ahead, Aim::Behind];

impl Act {
    pub fn index(self) -> usize {
        match self {
            Act::Cast { slot, aim } => {
                PLAIN + slot * AIMS + AIMED.iter().position(|&a| a == aim).unwrap_or(0)
            }
            Act::DuckLong => PLAIN + SLOTS * AIMS,
            Act::GuardLong => PLAIN + SLOTS * AIMS + 1,
            plain => PLAINS.iter().position(|&a| a == plain).unwrap_or(0),
        }
    }

    pub fn from_index(i: usize) -> Option<Act> {
        if i < PLAIN {
            return PLAINS.get(i).copied();
        }
        let c = i - PLAIN;
        let slot = c / AIMS;
        if slot >= SLOTS {
            return match i - PLAIN - SLOTS * AIMS {
                0 => Some(Act::DuckLong),
                1 => Some(Act::GuardLong),
                _ => None,
            };
        }
        AIMED.get(c % AIMS).map(|&aim| Act::Cast { slot, aim })
    }
}

/// Whether a skill of `kind` lands where it is aimed (the others take `Aim::Foe` alone).
fn aims(kind: SkillKind) -> bool {
    matches!(kind, SkillKind::Area | SkillKind::Blink)
}

/// The skill in `slot`, with its index in `skills`.
fn slotted<'a>(sense: &Sense, skills: &'a [Skill], slot: usize) -> Option<(usize, &'a Skill)> {
    let k = sense.slots.get(slot).copied().flatten()?;
    skills.get(k).map(|s| (k, s))
}

/// Which acts can happen now: waiting always; the rest only while it can act; a jump, or a take-off into flight
/// (from the ground or the air), only with the qi it costs; meditating only on the ground; a cast only from a filled slot whose skill is ready, and aimed off the foe
/// only for areas and blinks.
pub fn legal(sense: &Sense, skills: &[Skill]) -> [bool; ACTIONS] {
    let me = &sense.me;
    let mut out = [false; ACTIONS];
    for (i, o) in out.iter_mut().enumerate() {
        let Some(act) = Act::from_index(i) else {
            continue;
        };
        *o = match act {
            Act::Wait => true,
            _ if !me.can_act => false,
            Act::Toward | Act::Away | Act::Duck | Act::Guard | Act::DuckLong | Act::GuardLong => {
                true
            }
            // it sits down only from the ground (`Choice::Meditate` in the air is dropped)
            Act::Meditate => !me.flying && me.air <= 0.0,
            // leaving the ground costs qi, and so does taking off from the air (RULES 26)
            Act::Jump => !me.flying && me.can_leap(),
            Act::FlyHigh => !me.flying && me.can_leap(),
            Act::Land => me.flying,
            Act::Cast { slot, aim } => slotted(sense, skills, slot).is_some_and(|(k, s)| {
                sense.ready.get(k).copied().unwrap_or(false) && (aim == Aim::Foe || aims(s.kind))
            }),
        };
    }
    out
}

/// The `Choice` act `act` is, for a fighter that senses `sense` and sees `foe`.
pub fn choice(act: Act, sense: &Sense, foe: &Foe, skills: &[Skill]) -> Choice {
    let me = &sense.me;
    let a = &sense.arena;
    let dir = ahead(me, foe);
    match act {
        Act::Wait => Choice::Wait,
        Act::Toward => Choice::Move { dir },
        Act::Away => Choice::Move { dir: -dir },
        Act::Jump => Choice::Jump,
        Act::Duck => Choice::Duck { hold: HOLD },
        Act::Guard => Choice::Guard { hold: HOLD },
        Act::DuckLong => Choice::Duck { hold: HOLD_LONG },
        Act::GuardLong => Choice::Guard { hold: HOLD_LONG },
        Act::FlyHigh => Choice::Fly {
            height: a.top * FLY,
        },
        Act::Land => Choice::Fly { height: 0.0 },
        Act::Meditate => Choice::Meditate,
        Act::Cast { slot, aim } => {
            let Some((_, s)) = slotted(sense, skills, slot) else {
                return Choice::Wait;
            };
            let target = match (s.kind, aim) {
                (SkillKind::Area, Aim::Foe) => Some(foe.x),
                (SkillKind::Area, Aim::Ahead) => Some(foe.x + dir * AIM),
                (SkillKind::Area, Aim::Behind) => Some(foe.x - dir * AIM),
                (SkillKind::Blink, Aim::Foe) => Some(foe.x - dir * BESIDE),
                (SkillKind::Blink, Aim::Ahead) => Some(foe.x + dir * BESIDE),
                (SkillKind::Blink, Aim::Behind) => Some(me.x - dir * s.reach),
                _ => None,
            };
            let target = target.map(|x| {
                if x.is_finite() {
                    x.clamp(a.left, a.right)
                } else {
                    me.x
                }
            });
            Choice::Cast {
                skill: s.name.clone(),
                target,
            }
        }
    }
}
