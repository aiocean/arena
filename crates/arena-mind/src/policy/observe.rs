//! The observation: the fighter, the foe it sees, the nearest threats and its slots, as `OBS` floats in a frame
//! where the foe is ahead (positive x), every value clamped to [-4, 4] and anything not finite read as 0. Times are
//! in seconds unless a scale is given.
//!
//! Layout, in order (index ranges are of the whole observation):
//! - me (`ME` = 17, 0..17): distance to the wall behind and ahead over the arena's width, air over the ceiling,
//!   facing the foe, hp / 100, qi / 100, flying, hover over the ceiling, vx toward the foe / 200, can act, silenced,
//!   sealed, barrier, shield (0..14); winding up, recovering, seconds left in that phase (14..17, zero when it uses
//!   no skill);
//! - foe (`FOE` = 37, 17..54): distance over the width, air over the ceiling, flying, hp / 100, qi / 100 (17..22),
//!   what it does (idle, moving toward me, moving away, guarding, meditating, flying, 22..28), winding up,
//!   recovering (28..30), the kind of skill it uses (12, 30..42), how far into its phase as a share of it (42),
//!   seconds left in the phase (43), the phase's span (44), the skill's reach / 100 (45) and damage / 50 (46)
//!   (28..47 zero when it uses no skill), out for, open in, frozen, shocked, barrier, the share of its skills ready,
//!   seeker out (mine) (47..54);
//! - threats (`THREATS` x `THREAT` = 4 x 12, 54..102), nearest first, zero when absent: present, shape
//!   (projectile, area, seeker, field, barrage), distance ahead / 100, height / 50, speed toward me / 200, lands
//!   in, size / 50, damage / 50;
//! - slots (`SLOTS` x `SLOT` = 6 x 33, 102..300), zero when empty, each: present, ready, kind (12), cost / 50,
//!   damage x power / 50, reach / 100, windup, recover, radius / 50 (+0..+20); then what it inflicts (`Inflicts`,
//!   +20..+33): freeze s, stun s, silence s / 2, defence-down s / 5, defence lost (share), burn s / 3, burn's share
//!   of attack a second x 5, chill stacks a hit adds now (x `Sense::stacks`) over the most it holds, slow per chill
//!   stack x 4, shock stacks a hit adds now over the most it holds, hold per shock twitch x 4, share of attack per
//!   shock twitch x 10, pull / 100;
//! - the game's tag (`TAG` = 5, 300..305).

use super::{SLOTS, ahead};
use crate::sense::{ActPhase, Doing, Foe, Sense, Threat, ThreatShape};
use crate::skill::{Skill, SkillKind};

const ME: usize = 17;
const FOE: usize = 37;
const THREATS: usize = 4;
const THREAT: usize = 12;
const SLOT: usize = 33;
/// The game's tag: who the fighter is (the duel: its look, 2, and its style, 3).
pub const TAG: usize = 5;
pub const OBS: usize = ME + FOE + THREATS * THREAT + SLOTS * SLOT + TAG;

const KINDS: [SkillKind; 12] = [
    SkillKind::Melee,
    SkillKind::Dive,
    SkillKind::Dash,
    SkillKind::Blast,
    SkillKind::Bolt,
    SkillKind::Seeker,
    SkillKind::Area,
    SkillKind::Field,
    SkillKind::Heal,
    SkillKind::Ward,
    SkillKind::Barrier,
    SkillKind::Blink,
];

/// Writes floats in order, each clamped.
struct Out {
    v: [f32; OBS],
    at: usize,
}

impl Out {
    fn put(&mut self, x: f64) {
        let x = if x.is_finite() {
            x.clamp(-4.0, 4.0)
        } else {
            0.0
        };
        if let Some(slot) = self.v.get_mut(self.at) {
            *slot = x as f32;
        }
        self.at += 1;
    }
    fn flag(&mut self, b: bool) {
        self.put(if b { 1.0 } else { 0.0 });
    }
    fn kind(&mut self, k: Option<SkillKind>) {
        for c in KINDS {
            self.flag(k == Some(c));
        }
    }
    fn skip(&mut self, n: usize) {
        self.at += n;
    }
}

/// What the policy sees: `sense` for itself and its slots, `foe` and `threats` as it sees them (late, through
/// `Delayed`), `skills` as handed to the mind, and the game's `tag`.
pub fn observe(
    sense: &Sense,
    foe: &Foe,
    threats: &[Threat],
    skills: &[Skill],
    tag: &[f32; TAG],
) -> [f32; OBS] {
    let me = &sense.me;
    let a = &sense.arena;
    let dir = ahead(me, foe);
    let width = (a.right - a.left).max(1.0);
    let top = a.top.max(1.0);
    let mut o = Out {
        v: [0.0; OBS],
        at: 0,
    };

    let (behind, front) = if dir > 0.0 {
        (me.x - a.left, a.right - me.x)
    } else {
        (a.right - me.x, me.x - a.left)
    };
    o.put(behind / width);
    o.put(front / width);
    o.put(me.air / top);
    o.flag(me.face * dir > 0.0);
    o.put(me.hp / 100.0);
    o.put(me.qi / 100.0);
    o.flag(me.flying);
    o.put(me.hover / top);
    o.put(me.vx * dir / 200.0);
    o.flag(me.can_act);
    o.flag(me.silenced);
    o.flag(me.sealed);
    o.flag(me.barrier);
    o.flag(me.shield);
    match &me.act {
        Some(a) => {
            o.flag(a.phase == ActPhase::Windup);
            o.flag(a.phase == ActPhase::Recover);
            o.put(span_of(a.phase, a.windup, a.recover) - a.t);
        }
        None => o.skip(3),
    }

    o.put((foe.x - me.x) * dir / width);
    o.put(foe.air / top);
    o.flag(foe.flying);
    o.put(foe.hp / 100.0);
    o.put(foe.qi / 100.0);
    let (moving, toward) = match foe.doing {
        Doing::Move { dir: d } => (true, d * dir < 0.0),
        _ => (false, false),
    };
    o.flag(foe.doing == Doing::Idle);
    o.flag(moving && toward);
    o.flag(moving && !toward);
    o.flag(foe.doing == Doing::Guard);
    o.flag(foe.doing == Doing::Meditate);
    o.flag(foe.doing == Doing::Fly);
    match &foe.doing {
        Doing::Act {
            kind,
            phase,
            t,
            windup,
            recover,
            reach,
            damage,
        } => {
            o.flag(*phase == ActPhase::Windup);
            o.flag(*phase == ActPhase::Recover);
            o.kind(Some(*kind));
            let span = span_of(*phase, *windup, *recover);
            o.put(t / span.max(1e-3));
            o.put(span - t);
            o.put(span);
            o.put(reach / 100.0);
            o.put(damage / 50.0);
        }
        _ => o.skip(19),
    }
    o.put(foe.out_for);
    o.put(foe.open_in);
    o.put(foe.frozen);
    o.put(foe.shocked);
    o.flag(foe.barrier);
    let ready = foe.ready.iter().filter(|&&r| r).count() as f64 / (foe.ready.len().max(1) as f64);
    o.put(ready);
    o.flag(me.seeker_out);

    let mut near: Vec<(f64, &Threat)> = threats.iter().map(|t| (distance(t, me.x), t)).collect();
    near.sort_by(|x, y| x.0.total_cmp(&y.0));
    for i in 0..THREATS {
        let Some((_, t)) = near.get(i) else {
            o.skip(THREAT);
            continue;
        };
        o.flag(true);
        let (shape, x, y, vx, lands, size) = match t.shape {
            ThreatShape::Projectile { x, y, vx } => (0, x, y, vx, 0.0, 0.0),
            ThreatShape::Area {
                x,
                radius,
                reach,
                lands_in,
                ..
            } => (1, x, 0.0, 0.0, lands_in, radius.max(reach)),
            ThreatShape::Seeker { x, y, radius } => (2, x, y, 0.0, 0.0, radius),
            ThreatShape::Field { x, reach } => (3, x, 0.0, 0.0, 0.0, reach),
            ThreatShape::Barrage { lands_in } => (4, me.x, 0.0, 0.0, lands_in, 0.0),
        };
        for s in 0..5 {
            o.flag(s == shape);
        }
        o.put((x - me.x) * dir / 100.0);
        o.put(y / 50.0);
        o.put(-vx * dir / 200.0);
        o.put(lands);
        o.put(size / 50.0);
        o.put(t.damage / 50.0);
    }

    for i in 0..SLOTS {
        let skill = sense
            .slots
            .get(i)
            .copied()
            .flatten()
            .and_then(|k| skills.get(k).map(|s| (k, s)));
        let Some((k, s)) = skill else {
            o.skip(SLOT);
            continue;
        };
        o.flag(true);
        o.flag(sense.ready.get(k).copied().unwrap_or(false));
        o.kind(Some(s.kind));
        o.put(s.cost / 50.0);
        o.put(s.damage * sense.power.get(k).copied().unwrap_or(1.0) / 50.0);
        o.put(s.reach / 100.0);
        o.put(s.windup);
        o.put(s.recover);
        o.put(s.radius / 50.0);
        let i = &s.inflicts;
        let stacks = sense.stacks.get(k).copied().unwrap_or(1.0);
        o.put(i.freeze);
        o.put(i.stun);
        o.put(i.silence / 2.0);
        o.put(i.def_down / 5.0);
        o.put(i.def_loss);
        o.put(i.burn / 3.0);
        o.put(i.burn_rate * 5.0);
        o.put(i.chill * stacks / i.chill_max.max(1.0));
        o.put(i.slow * 4.0);
        o.put(i.shock * stacks / i.shock_max.max(1.0));
        o.put(i.shock_hold * 4.0);
        o.put(i.shock_rate * 10.0);
        o.put(i.pull / 100.0);
    }

    for &t in tag {
        o.put(f64::from(t));
    }
    o.v
}

/// Seconds phase `phase` of a skill lasts.
fn span_of(phase: ActPhase, windup: f64, recover: f64) -> f64 {
    if phase == ActPhase::Windup {
        windup
    } else {
        recover
    }
}

/// How far a threat is from `x`: what "nearest" sorts by (a barrage is upon the fighter already).
fn distance(t: &Threat, x: f64) -> f64 {
    match t.shape {
        ThreatShape::Projectile { x: tx, .. }
        | ThreatShape::Area { x: tx, .. }
        | ThreatShape::Seeker { x: tx, .. }
        | ThreatShape::Field { x: tx, .. } => libm::fabs(tx - x),
        ThreatShape::Barrage { .. } => 0.0,
    }
}
