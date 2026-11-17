//! The scripted brain that plays every loadout, reasonably and not perfectly. Each tick it is free to act it:
//!
//! - at low life (`LOW_LIFE`), heals if it can, and raises a ward when the foe is gathering a blow nearby;
//! - otherwise casts the ready skill it can afford with the highest value: a striking skill's expected damage
//!   against the foe now (`Fight::expected`, refreshed every second), an intent, a stance, a blessing, a domain or a
//!   curse not yet standing `UTILITY`;
//! - walks in instead when a blow out of reach is worth `CLOSE_IN` times the best blow in reach (a utility cast is
//!   made first);
//! - when nothing worth casting reaches, runs to the preferred range of its best ready skill (or blinks there when
//!   far), meditates when its qi is low and the foe far, and otherwise waits.
//!
//! Small seeded randomness (`JITTER` on every value, `HESITATE` a tick) makes mirror fights differ. Its dice are
//! its own (`Rng::salted` per seat), so they never move the fight's rolls.

use arena_calc::gem::Effect;

use crate::delivery::{BLINK_LENGTH, Delivery};
use crate::fight::{Fight, Order};
use crate::rng::Rng;

/// Life share under which the brain heals and wards.
pub const LOW_LIFE: f64 = 0.4;
/// Life share under which a heal is worth casting at all.
pub const HEAL_BELOW: f64 = 0.7;
/// The value of a skill that deals no damage but stands (intent, stance, blessing, domain, curse), in damage.
pub const UTILITY: f64 = 250.0;
/// The value of a ward raised against a foe's blow, in damage.
pub const WARD_VALUE: f64 = 300.0;
/// It walks in for a blow out of reach worth this many times the best one in reach (the run costs time).
pub const CLOSE_IN: f64 = 1.5;
/// Every value is scaled by `1 ± JITTER`.
pub const JITTER: f64 = 0.15;
/// The chance a free tick is spent waiting.
pub const HESITATE: f64 = 0.2;
/// Qi share under which, with the foe far (`MEDITATE_FAR` T), it meditates.
pub const MEDITATE_QI: f64 = 0.3;
pub const MEDITATE_FAR: f64 = 4.0;
/// How near the preferred range it stays: a share of the skill's range, at least `MIN_RANGE` T.
pub const RANGE_SHARE: f64 = 0.8;
pub const MIN_RANGE: f64 = 0.7;

/// Seat `me`'s order this tick.
pub fn decide(fight: &Fight, me: usize, rng: &mut Rng) -> Order {
    let f = &fight.f[me];
    let c = &fight.c[me];
    if !f.free() {
        return Order::Wait;
    }
    if rng.next_f64() < HESITATE {
        return Order::Wait;
    }
    let foe = 1 - me;
    let d = fight.distance();
    let life = c.life / c.life_max.max(1.0);
    let foe_striking = fight.f[foe].in_windup();

    let mut best: Option<(f64, usize, f64)> = None;
    // the best striking skill ready but out of reach: where to go
    let mut want: Option<(f64, f64)> = None;
    for (i, s) in f.slots.iter().enumerate() {
        let costs = fight.costs(me, i);
        if !fight.can_cast(me, i, &costs) {
            continue;
        }
        let jitter = 1.0 + JITTER * (2.0 * rng.next_f64() - 1.0);
        let in_reach = d <= s.range;
        let (value, dir) = match s.delivery {
            Delivery::Heal => {
                let share: f64 = s
                    .gem
                    .effects
                    .iter()
                    .map(|e| match e {
                        Effect::Heal(h) => *h,
                        _ => 0.0,
                    })
                    .sum();
                let v = if life < HEAL_BELOW {
                    share * c.life_max * if life < LOW_LIFE { 2.0 } else { 1.0 }
                } else {
                    0.0
                };
                (v, 1.0)
            }
            Delivery::Guard => {
                let v = if foe_striking && d <= 5.0 {
                    WARD_VALUE * if life < LOW_LIFE { 2.0 } else { 1.0 }
                } else {
                    0.0
                };
                (v, 1.0)
            }
            Delivery::Movement => {
                // close in for a melee kit, or step out when the foe crowds a ranged one
                let pref = preferred(fight, me);
                if d > pref + BLINK_LENGTH * 0.6 {
                    (UTILITY * 0.5, 1.0)
                } else if pref > 2.5 && d < pref - BLINK_LENGTH * 0.6 {
                    (UTILITY * 0.5, -1.0)
                } else {
                    (0.0, 1.0)
                }
            }
            Delivery::Aura | Delivery::Buff => {
                let on = c
                    .effects
                    .iter()
                    .any(|e| e.source == arena_calc::stat::Source::Skill(s.gem.id));
                (if on { 0.0 } else { UTILITY }, 1.0)
            }
            Delivery::Curse => {
                let on = fight.f[foe].curses.on.iter().any(|k| {
                    arena_calc::gem::curse_of(s.gem).is_some_and(|m| m.index() == k.kind.index())
                });
                (if on || !in_reach { 0.0 } else { UTILITY }, 1.0)
            }
            Delivery::Domain { damaging } => {
                let up = fight.field.domains[me].is_some();
                (
                    if up { 0.0 } else { UTILITY } + if damaging { s.estimate } else { 0.0 },
                    1.0,
                )
            }
            _ => {
                let v = s.estimate;
                if !in_reach {
                    if want.is_none_or(|w| v > w.0) {
                        want = Some((v, s.range));
                    }
                    (0.0, 1.0)
                } else {
                    (v, 1.0)
                }
            }
        };
        let v = value * jitter;
        if v > 0.0 && best.is_none_or(|b| v > b.0) {
            best = Some((v, i, dir));
        }
    }
    let toward = if fight.f[foe].x >= f.x { 1.0 } else { -1.0 };
    match (best, want) {
        // a blow out of reach worth clearly more than the best blow in reach: go and get it (a utility cast, which
        // needs no reach, is made first)
        (Some((v, slot, _)), Some((w, r)))
            if f.slots[slot].delivery.strikes()
                && w > CLOSE_IN * v
                && d > (r * RANGE_SHARE).max(MIN_RANGE) =>
        {
            return Order::Move(toward);
        }
        (Some((_, slot, dir)), _) => return Order::Cast { slot, dir },
        _ => {}
    }

    let pref = match want {
        Some((_, r)) => (r * RANGE_SHARE).max(MIN_RANGE),
        None => preferred(fight, me),
    };
    if c.qi < MEDITATE_QI * c.qi_max && d > MEDITATE_FAR {
        return Order::Meditate;
    }
    if d > pref + 0.2 {
        Order::Move(toward)
    } else if d < pref - 0.6 {
        Order::Move(-toward)
    } else {
        Order::Wait
    }
}

/// The range seat `me` likes to fight at: its best striking skill's (by estimate), whatever its cooldown.
pub fn preferred(fight: &Fight, me: usize) -> f64 {
    fight.f[me]
        .slots
        .iter()
        .filter(|s| s.delivery.strikes() && s.range.is_finite())
        .max_by(|a, b| a.estimate.total_cmp(&b.estimate))
        .map_or(3.0, |s| (s.range * RANGE_SHARE).max(MIN_RANGE))
}
