//! What a policy remembers between decisions (ABI 5): the last acts it chose, and the last things it saw the foe
//! start, each with when. Without it a policy that ducked a moment ago cannot tell, and ducks, stands and ducks again.
//! ABI 6 remembers further back (8 of each) and, for each kind of skill, when it last saw the foe start one, so it can
//! tell what the foe has just spent. The mind keeps it (`PolicyMind`); `observe_with` writes it as the "My acts",
//! "Foe's acts" (the newest 3 and 4, ABI 5's), "My older acts", "Foe's older acts" and "Foe's kinds" blocks.

use super::ahead;
use super::observe::KINDS;
use crate::sense::{Doing, Foe, Me};
use crate::skill::SkillKind;

/// The acts of its own it remembers, newest first.
pub const MY_ACTS: usize = 8;
/// The foe's starts it remembers, newest first.
pub const FOE_ACTS: usize = 8;
/// How many of each ABI 5 had, which keep their place in the observation (the rest are appended after them).
pub const MY_ACTS_ABI5: usize = 3;
pub const FOE_ACTS_ABI5: usize = 4;

/// What the foe was seen doing, in the policy's frame (toward and away are relative to the fighter).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FoeAct {
    Idle,
    Toward,
    Away,
    Guard,
    Meditate,
    Fly,
    Skill(SkillKind),
}

impl FoeAct {
    /// What `foe` is doing, seen from `me`; `None` while it is moving in no clear way (a skill's recovery is the
    /// skill still).
    pub fn of(me: &Me, foe: &Foe) -> FoeAct {
        let dir = ahead(me, foe);
        match foe.doing {
            Doing::Idle => FoeAct::Idle,
            Doing::Move { dir: d } => {
                if d * dir < 0.0 {
                    FoeAct::Toward
                } else {
                    FoeAct::Away
                }
            }
            Doing::Guard => FoeAct::Guard,
            Doing::Meditate => FoeAct::Meditate,
            Doing::Fly => FoeAct::Fly,
            Doing::Act { kind, .. } => FoeAct::Skill(kind),
        }
    }
}

/// The policy's memory: its own acts (the act's index, the time it chose it) and the foe's starts (what, when), each
/// newest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memory {
    pub mine: [Option<(usize, f64)>; MY_ACTS],
    pub foe: [Option<(FoeAct, f64)>; FOE_ACTS],
    /// When it last saw the foe start a skill of each kind (`KINDS`' order), none before.
    pub foe_kinds: [Option<f64>; KINDS.len()],
}

impl Memory {
    /// It chose act `act` at `time`.
    pub fn chose(&mut self, act: usize, time: f64) {
        push(&mut self.mine, (act, time));
    }

    /// It sees the foe doing `what` at `time`: remembered when it differs from the last thing seen.
    pub fn saw(&mut self, what: FoeAct, time: f64) {
        if self.foe[0].map(|(w, _)| w) != Some(what) {
            push(&mut self.foe, (what, time));
            if let FoeAct::Skill(k) = what
                && let Some(i) = KINDS.iter().position(|&c| c == k)
            {
                self.foe_kinds[i] = Some(time);
            }
        }
    }

    /// A new round: it forgets.
    pub fn clear(&mut self) {
        *self = Memory::default();
    }
}

fn push<T: Copy, const N: usize>(list: &mut [Option<T>; N], x: T) {
    list.rotate_right(1);
    list[0] = Some(x);
}
