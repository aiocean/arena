//! What a skill makes on the field (design, 17.8): traps, mines and totems (a skill a support made one,
//! `support::Mods::becomes`) and minions (a skill tagged minion). [`spawn`] reads a built cast into a [`Spawn`]:
//! how many each cast makes, their life ([`Stat::DeviceLife`], [`Stat::MinionLife`]), the damage factor their
//! blows carry ([`Stat::MinionDamage`]), how long they last and how many can stand at once ([`Stat::TrapCount`],
//! [`Stat::MineCount`], [`Stat::TotemCount`], [`Stat::MinionCount`]). [`Placed`] keeps those standing, by PoE's
//! rules:
//! - placing past the cap removes the oldest;
//! - a trap arms [`TRAP_ARM`] after it lands, then springs on the first enemy within [`TRAP_RADIUS`];
//! - a mine arms [`MINE_ARM`] after it lands; on the owner's command every armed mine goes off, the oldest first,
//!   [`MINE_STAGGER`] apart (PoE's detonation in sequence);
//! - a totem casts the skill [`Spawn::first_cast`] after it stands, then every [`Spawn::cast_interval`]: the skill's
//!   windup and recover or its cooldown, whichever is longer, but never more than [`TOTEM_CADENCE_SHARE`] of the
//!   totem's lifetime, so a skill whose cooldown outlasts the totem (Vạn Kiếm Trận, 9 s against 8 s) still casts
//!   several times on it, as a PoE totem casts at its own pace;
//! - a minion is a record with life: the fight moves it and has it strike;
//! - a shot of a skill under Lôi Bạo Trễ that a guard, a ward, a barrier or a unit stops becomes a mine there
//!   ([`STOPPED_MINE`]), its blow a share of the stopped one ([`STOPPED_SHARE`]).
//!
//! A device's blow is the built hit itself: the support's less (Bẫy Trận, Địa Lôi Trận 10%, Pháp Đàn 30%) is in it
//! already, so its [`Spawn::damage`] is 1. A minion's blows are the fight's to build; they carry
//! [`Spawn::damage`], `MinionDamage`'s factor.

use crate::build::Built;
use crate::collide::Shape;
use crate::play::{Point, Target};
use crate::stat::Stat;
use crate::tag::Tag;

/// A trap's or a mine's life before [`Stat::DeviceLife`].
pub const DEVICE_LIFE: f64 = 200.0;
/// A totem's life before [`Stat::DeviceLife`].
pub const TOTEM_LIFE: f64 = 600.0;
/// A minion's life before [`Stat::MinionLife`].
pub const MINION_LIFE: f64 = 500.0;
/// Seconds a trap, a mine, a totem stands before it falls apart (a minion stays until it dies).
pub const TRAP_LIFETIME: f64 = 8.0;
pub const MINE_LIFETIME: f64 = 12.0;
pub const TOTEM_LIFETIME: f64 = 8.0;
/// Seconds after it lands before a trap can spring, a mine go off.
pub const TRAP_ARM: f64 = 0.5;
pub const MINE_ARM: f64 = 0.3;
/// T between a trap and an enemy's body that springs it.
pub const TRAP_RADIUS: f64 = 1.0;
/// Seconds between two mines going off on one command.
pub const MINE_STAGGER: f64 = 0.1;
/// Minions standing at once before [`Stat::MinionCount`] (a device's cap is its count stat alone: the support gives
/// it, `support::DEVICES`).
pub const MINION_CAP: f64 = 2.0;
/// The shortest time between two casts of a totem.
pub const MIN_CAST_INTERVAL: f64 = 0.1;
/// The longest time between two casts of a totem, as a share of its lifetime: a totem casts at least about
/// `1 / TOTEM_CADENCE_SHARE` times before it falls apart, whatever the skill's cooldown. The cooldown still paces
/// the hand that raises totems; on the totem it only slows the cadence up to this.
pub const TOTEM_CADENCE_SHARE: f64 = 0.25;

/// Lôi Bạo Trễ (`support::Behaviour::MineOnBlock`): a stopped shot's mine deals this share of the blow it carried
/// when it was stopped. With a guard's chip (a fifth of the blow) it is the blow once, never more: what one shot
/// reaches the foe with stays within its cast (rules, "One cast, 40% of a life").
pub const STOPPED_SHARE: f64 = 0.8;
/// Stopped shots' mines one skill of one owner has standing at once; past it the oldest goes.
pub const STOPPED_CAP: u32 = 3;

/// What a stopped shot of a skill under Lôi Bạo Trễ becomes: one mine where it stopped, a trap's life, armed after
/// [`MINE_ARM`], standing [`MINE_LIFETIME`], at most [`STOPPED_CAP`] of them, each striking with the stopped blow
/// times [`STOPPED_SHARE`]. It goes off as any mine does, on its owner's command.
pub const STOPPED_MINE: Spawn = Spawn {
    kind: Kind::Mine,
    count: 1,
    life: DEVICE_LIFE,
    damage: STOPPED_SHARE,
    lifetime: Some(MINE_LIFETIME),
    cap: STOPPED_CAP,
    arm: MINE_ARM,
    trigger_radius: 0.0,
    first_cast: 0.0,
    cast_interval: 0.0,
};

/// What a skill makes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Kind {
    Trap,
    Mine,
    Totem,
    Minion,
}

/// What each cast of a skill makes, and their numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spawn {
    pub kind: Kind,
    /// Made by each cast.
    pub count: u32,
    /// Each one's life.
    pub life: f64,
    /// The damage factor their blows carry on top of the built hit.
    pub damage: f64,
    /// Seconds each stands; `None` until it dies (a minion).
    pub lifetime: Option<f64>,
    /// Standing at once, at most.
    pub cap: u32,
    /// Seconds before a trap or a mine is armed (0 for the rest).
    pub arm: f64,
    /// T that springs a trap (0 for the rest).
    pub trigger_radius: f64,
    /// A totem's first cast, seconds after it stands, and the seconds between casts (0 for the rest).
    pub first_cast: f64,
    pub cast_interval: f64,
}

/// A count as a whole number, never below `min`.
fn whole(v: f64, min: u32) -> u32 {
    (libm::round(v.clamp(0.0, f64::from(u16::MAX))) as u32).max(min)
}

/// What `b` makes, if it is a trap, a mine, a totem (the first of them its tags carry) or a minion; `None` for a
/// skill that makes nothing.
pub fn spawn(b: &Built) -> Option<Spawn> {
    let tags = b.hit.tags;
    let t = &b.table;
    let device = |kind, cap: f64, life, lifetime| Spawn {
        kind,
        count: 1,
        life: t.get(Stat::DeviceLife, life).max(0.0),
        damage: 1.0,
        lifetime: Some(lifetime),
        cap: whole(cap, 1),
        arm: 0.0,
        trigger_radius: 0.0,
        first_cast: 0.0,
        cast_interval: 0.0,
    };
    if tags.has(Tag::Trap) {
        Some(Spawn {
            arm: TRAP_ARM,
            trigger_radius: TRAP_RADIUS,
            ..device(
                Kind::Trap,
                t.get(Stat::TrapCount, 0.0),
                DEVICE_LIFE,
                TRAP_LIFETIME,
            )
        })
    } else if tags.has(Tag::Mine) {
        Some(Spawn {
            arm: MINE_ARM,
            ..device(
                Kind::Mine,
                t.get(Stat::MineCount, 0.0),
                DEVICE_LIFE,
                MINE_LIFETIME,
            )
        })
    } else if tags.has(Tag::Totem) {
        Some(Spawn {
            first_cast: b.windup.max(0.0),
            cast_interval: (b.windup + b.recover)
                .max(b.cooldown)
                .clamp(MIN_CAST_INTERVAL, TOTEM_LIFETIME * TOTEM_CADENCE_SHARE),
            ..device(
                Kind::Totem,
                t.get(Stat::TotemCount, 0.0),
                TOTEM_LIFE,
                TOTEM_LIFETIME,
            )
        })
    } else if tags.has(Tag::Minion) {
        Some(Spawn {
            kind: Kind::Minion,
            count: 1,
            life: t.get(Stat::MinionLife, MINION_LIFE).max(0.0),
            damage: t.acc(Stat::MinionDamage).factor().max(0.0),
            lifetime: None,
            cap: whole(t.get(Stat::MinionCount, MINION_CAP), 1),
            arm: 0.0,
            trigger_radius: 0.0,
            first_cast: 0.0,
            cast_interval: 0.0,
        })
    } else {
        None
    }
}

/// One thing standing: its id (given in order of placing), where it is, its life, and seconds since it stood. A
/// totem keeps the time of its next cast.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Device {
    pub id: u32,
    pub at: Point,
    pub life: f64,
    pub age: f64,
    pub next_cast: f64,
}

impl Device {
    /// Whether a trap or a mine has armed.
    pub fn armed(&self, spawn: &Spawn) -> bool {
        self.age >= spawn.arm
    }
}

/// What placing did: the ids made, and the ids of the oldest it removed past the cap.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Placement {
    pub made: Vec<u32>,
    pub removed: Vec<u32>,
}

/// What a tick did: totem casts in time order (the totem's id, seconds into the tick), and the ids that fell apart.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Ticked {
    pub casts: Vec<(u32, f64)>,
    pub expired: Vec<u32>,
}

/// The things one skill of one owner has standing, oldest first.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Placed {
    pub spawn: Spawn,
    pub standing: Vec<Device>,
    pub next_id: u32,
}

impl Placed {
    /// None standing yet.
    pub fn new(spawn: Spawn) -> Placed {
        Placed {
            spawn,
            standing: Vec::new(),
            next_id: 0,
        }
    }

    /// A cast lands at `at`: [`Spawn::count`] new ones there, the oldest removed while more than the cap stand.
    pub fn place(&mut self, at: Point) -> Placement {
        let mut out = Placement::default();
        for _ in 0..self.spawn.count {
            let id = self.next_id;
            self.next_id = self.next_id.wrapping_add(1);
            self.standing.push(Device {
                id,
                at,
                life: self.spawn.life,
                age: 0.0,
                next_cast: self.spawn.first_cast,
            });
            out.made.push(id);
        }
        let cap = self.spawn.cap.max(1) as usize;
        while self.standing.len() > cap {
            let old = self.standing.remove(0);
            out.removed.push(old.id);
            out.made.retain(|&m| m != old.id);
        }
        out
    }

    /// Time passes: each ages; a totem casts every time its next cast comes within the tick (no later than its
    /// lifetime); whatever has outlived its lifetime falls apart.
    pub fn tick(&mut self, dt: f64) -> Ticked {
        let dt = dt.max(0.0);
        let mut out = Ticked::default();
        let totem = self.spawn.kind == Kind::Totem && self.spawn.cast_interval > 0.0;
        for d in &mut self.standing {
            let start = d.age;
            d.age += dt;
            if totem {
                let end = self.spawn.lifetime.map_or(d.age, |l| d.age.min(l));
                while d.next_cast <= end {
                    out.casts.push((d.id, d.next_cast - start));
                    d.next_cast += self.spawn.cast_interval;
                }
            }
        }
        out.casts
            .sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        if let Some(l) = self.spawn.lifetime {
            out.expired = self
                .standing
                .iter()
                .filter(|d| d.age >= l)
                .map(|d| d.id)
                .collect();
            self.standing.retain(|d| d.age < l);
        }
        out
    }

    /// Traps spring: each armed trap, the oldest first, takes the nearest live enemy whose body is within
    /// [`Spawn::trigger_radius`] (the first in the list on a tie) and is gone. Returns `(trap, enemy)` pairs.
    pub fn spring(&mut self, enemies: &[Target]) -> Vec<(u32, u32)> {
        if self.spawn.kind != Kind::Trap {
            return Vec::new();
        }
        let spawn = self.spawn;
        let mut out = Vec::new();
        self.standing.retain(|d| {
            if !d.armed(&spawn) {
                return true;
            }
            let mut best: Option<(u32, f64)> = None;
            for e in enemies.iter().filter(|e| e.alive) {
                let gap = d.at.distance(e.at()) - e.radius.max(0.0);
                let reach = Shape::circle(d.at.x, d.at.y, spawn.trigger_radius);
                let body = Shape::circle(e.x, e.y, e.radius);
                if crate::collide::overlap(&reach, &body) && best.is_none_or(|(_, b)| gap < b) {
                    best = Some((e.id, gap));
                }
            }
            match best {
                Some((e, _)) => {
                    out.push((d.id, e));
                    false
                }
                None => true,
            }
        });
        out
    }

    /// The owner's command: every armed mine goes off, the oldest first, [`MINE_STAGGER`] apart. Returns
    /// `(mine, seconds from the command, where)`; the mines are gone.
    pub fn detonate(&mut self) -> Vec<(u32, f64, Point)> {
        if self.spawn.kind != Kind::Mine {
            return Vec::new();
        }
        let spawn = self.spawn;
        let mut out = Vec::new();
        self.standing.retain(|d| {
            if d.armed(&spawn) {
                out.push((d.id, out.len() as f64 * MINE_STAGGER, d.at));
                false
            } else {
                true
            }
        });
        out
    }

    /// `amount` of damage to the one with `id`; whether it was destroyed (then it is gone).
    pub fn hurt(&mut self, id: u32, amount: f64) -> bool {
        let Some(i) = self.standing.iter().position(|d| d.id == id) else {
            return false;
        };
        self.standing[i].life -= amount.max(0.0);
        if self.standing[i].life <= 0.0 {
            self.standing.remove(i);
            return true;
        }
        false
    }
}
