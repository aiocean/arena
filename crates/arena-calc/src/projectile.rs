//! Projectiles on the field (design, 17.8): where a shot flies, what it strikes, and how it pierces, forks and
//! chains ([`Stat::Pierce`], [`Stat::Fork`], [`Stat::Chain`], [`Stat::ProjectileSpeed`], `build::Built::projectiles`).
//! Pure geometry over [`Target`]s: [`trace`] gives every strike of a shot against still targets, and
//! [`Flight::step`] moves one projectile a frame for a fight whose targets move.
//!
//! A projectile is a point flying straight at its speed until its range is spent. On striking a target, PoE's
//! order decides what it does next:
//! 1. **pierce**: while it has pierces left, it passes through and flies on;
//! 2. **fork**: else, while it has forks left, it splits in two at ±[`FORK_ANGLE`] from the strike, and ends; each
//!    half keeps what is left of the range and can never strike the target it forked from;
//! 3. **chain**: else, while it has chains left, it turns toward the nearest other live target within
//!    [`CHAIN_RANGE`] of the one struck that this projectile has not struck, and flies straight at it (anything new
//!    in the way is struck first); none in range, it ends;
//! 4. else it ends.
//!
//! A projectile (with its forks and chains) never strikes the same target twice; each extra projectile of a cast
//! keeps its own memory, so two can strike the same target. Extra projectiles fan out [`SPREAD`] apart, centred on
//! the aim. The supports' own less and more (Liên Kích's 25% less) are in the blow already
//! (`build::Built::hit`); a strike's [`Strike::factor`] is what the path adds on top, [`CHAIN_FACTOR`] for each
//! chain, [`FORK_FACTOR`] for each fork, [`PIERCE_FACTOR`] for each pierce (PoE has none: all 1).

use std::collections::VecDeque;

use crate::build::Built;
use crate::hit::Hit;
use crate::play::{Point, Target};
use crate::stat::{Stat, Table};

/// T between the centres of the target struck and the next a chain can turn to.
pub const CHAIN_RANGE: f64 = 4.0;
/// The damage factor each chain multiplies (PoE's chain has no falloff of its own: its less is the support's).
pub const CHAIN_FACTOR: f64 = 1.0;
/// The angle each half of a fork turns from the projectile's flight, in radians (30°).
pub const FORK_ANGLE: f64 = std::f64::consts::PI / 6.0;
/// The damage factor each fork multiplies.
pub const FORK_FACTOR: f64 = 1.0;
/// The damage factor each pierce multiplies.
pub const PIERCE_FACTOR: f64 = 1.0;
/// The angle between two neighbouring projectiles of a fan, in radians (10°).
pub const SPREAD: f64 = std::f64::consts::PI / 18.0;
/// A projectile forks at most this many times, whatever its [`Stat::Fork`] (2⁴ = 16 branches at most).
pub const MAX_FORKS: u32 = 4;
/// Range left under this is none (T).
const EPS: f64 = 1e-12;

/// A shot as the fight aims it: from `origin`, toward `direction` (any length; zero is `+x`), at `speed` T a second
/// before [`Stat::ProjectileSpeed`], flying at most `range` T.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Shot {
    pub origin: Point,
    pub direction: Point,
    pub speed: f64,
    pub range: f64,
}

/// What a cast's table makes of its projectiles: pierces, forks and chains each projectile has, extra projectiles,
/// and the speed factor.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Powers {
    pub pierce: u32,
    pub fork: u32,
    pub chain: u32,
    /// Projectiles besides the first.
    pub extra: u32,
    /// Speed is the shot's times this (a factor from 1).
    pub speed: f64,
}

impl Powers {
    /// None: one projectile at the shot's speed, ending at its first strike.
    pub const NONE: Powers = Powers {
        pierce: 0,
        fork: 0,
        chain: 0,
        extra: 0,
        speed: 1.0,
    };

    /// From a cast's table and its extra projectiles (`build::Built::projectiles`). Counts are rounded, never below
    /// zero; forks at most [`MAX_FORKS`].
    pub fn of(t: &Table, extra: u8) -> Powers {
        let count = |v: f64| libm::round(v.clamp(0.0, f64::from(u16::MAX))) as u32;
        Powers {
            pierce: count(t.get(Stat::Pierce, 0.0)),
            fork: count(t.get(Stat::Fork, 0.0)).min(MAX_FORKS),
            chain: count(t.get(Stat::Chain, 0.0)),
            extra: u32::from(extra),
            speed: t.get(Stat::ProjectileSpeed, 1.0).max(0.0),
        }
    }

    /// From a built cast.
    pub fn of_built(b: &Built) -> Powers {
        Powers::of(&b.table, b.projectiles)
    }
}

/// How the leg that struck began: the shot's own flight, after a pierce, a fork or a chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Via {
    Direct,
    Pierce,
    Fork,
    Chain,
}

/// One strike of a shot.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Strike {
    /// The target's id.
    pub target: u32,
    /// Seconds from the cast.
    pub time: f64,
    /// The damage factor the path adds ([`Strike::blow`] puts it on the hit).
    pub factor: f64,
    /// Which projectile of the fan (0 the first, from the most clockwise).
    pub projectile: u32,
    /// Which branch of its forks: 0 the projectile, a fork of branch `b` makes `2b + 1` (its counter-clockwise half)
    /// and `2b + 2`.
    pub branch: u32,
    /// How the leg that struck began.
    pub via: Via,
    /// Where it struck (the edge of the target's body).
    pub at: Point,
    /// T the projectile had flown (`build::Situation::travelled`, Dịch Linh).
    pub travelled: f64,
}

impl Strike {
    /// `hit` as this strike lands it: [`Strike::factor`] as one more (the same blow when the factor is 1).
    pub fn blow(&self, hit: &Hit) -> Hit {
        let mut h = hit.clone();
        if self.factor != 1.0 {
            h.more.push(self.factor - 1.0);
        }
        h
    }
}

/// One projectile in flight: where it is, where it flies, how fast, the range it has left, the time and distance
/// since the cast, the pierces, forks and chains it has left, its damage factor, which projectile and branch it
/// is, how its leg began, the targets it has struck, and whether it has ended.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Flight {
    pub pos: Point,
    /// Length 1.
    pub dir: Point,
    pub speed: f64,
    pub range_left: f64,
    pub time: f64,
    pub travelled: f64,
    pub pierce: u32,
    pub fork: u32,
    pub chain: u32,
    pub factor: f64,
    pub projectile: u32,
    pub branch: u32,
    pub via: Via,
    pub struck: Vec<u32>,
    pub done: bool,
}

/// What one [`Flight::step`] did: its strikes in order, and the forks it made (each to be stepped on its own, from
/// the time of the fork).
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Step {
    pub strikes: Vec<Strike>,
    pub forks: Vec<Flight>,
}

/// T along `dir` (length 1) from `p` to where it first enters the circle, if it does within `reach`; 0 when `p` is
/// inside it.
fn entry(p: Point, dir: Point, c: Point, r: f64, reach: f64) -> Option<f64> {
    let (fx, fy) = (p.x - c.x, p.y - c.y);
    let b = fx * dir.x + fy * dir.y;
    let cc = fx * fx + fy * fy - r * r;
    if cc <= 0.0 {
        return Some(0.0);
    }
    let disc = b * b - cc;
    if disc < 0.0 || b >= 0.0 {
        // misses, or the circle is behind
        return None;
    }
    let s = -b - libm::sqrt(disc);
    (s >= 0.0 && s <= reach).then_some(s)
}

impl Flight {
    /// The projectiles of `shot` with `powers`: `1 + extra`, fanned [`SPREAD`] apart around the aim, the first the
    /// most clockwise. None when the speed is not above zero.
    pub fn launch(shot: &Shot, powers: &Powers) -> Vec<Flight> {
        let speed = shot.speed * powers.speed;
        if speed.is_nan() || speed <= 0.0 || !speed.is_finite() {
            return Vec::new();
        }
        let aim = shot.direction.unit();
        let n = powers.extra.saturating_add(1);
        let mid = f64::from(n - 1) / 2.0;
        (0..n)
            .map(|k| Flight {
                pos: shot.origin,
                dir: aim.rotated((f64::from(k) - mid) * SPREAD),
                speed,
                range_left: shot.range.max(0.0),
                time: 0.0,
                travelled: 0.0,
                pierce: powers.pierce,
                fork: powers.fork.min(MAX_FORKS),
                chain: powers.chain,
                factor: 1.0,
                projectile: k,
                branch: 0,
                via: Via::Direct,
                struck: Vec::new(),
                done: false,
            })
            .collect()
    }

    /// Whether it may strike `t`: alive, not struck by this projectile, not struck before (the fight's mark).
    fn can_strike(&self, t: &Target) -> bool {
        t.alive && !t.hit_before && !self.struck.contains(&t.id)
    }

    /// The first target it would enter within `reach` T: the nearest, the first in the list on a tie.
    fn first(&self, reach: f64, targets: &[Target]) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (i, t) in targets.iter().enumerate() {
            if !self.can_strike(t) {
                continue;
            }
            if let Some(s) = entry(self.pos, self.dir, t.at(), t.radius.max(0.0), reach)
                && best.is_none_or(|(_, b)| s < b)
            {
                best = Some((i, s));
            }
        }
        best
    }

    /// The nearest live target it has not struck within [`CHAIN_RANGE`] of `from`'s centre (the first in the list
    /// on a tie).
    fn chain_target<'t>(&self, from: &Target, targets: &'t [Target]) -> Option<&'t Target> {
        let mut best: Option<(&Target, f64)> = None;
        for t in targets {
            if t.id == from.id || !self.can_strike(t) {
                continue;
            }
            let d = from.at().distance(t.at());
            if d <= CHAIN_RANGE && best.is_none_or(|(_, b)| d < b) {
                best = Some((t, d));
            }
        }
        best.map(|(t, _)| t)
    }

    fn advance(&mut self, s: f64) {
        self.pos = self.pos.plus(self.dir, s);
        self.range_left = (self.range_left - s).max(0.0);
        self.travelled += s;
        self.time += s / self.speed;
    }

    /// Flies `dt` seconds (`f64::INFINITY`: until it ends) among `targets` where they stand now, striking, piercing,
    /// forking and chaining in PoE's order. A fork ends it and returns its two halves in [`Step::forks`].
    pub fn step(&mut self, dt: f64, targets: &[Target]) -> Step {
        let mut out = Step::default();
        if self.speed.is_nan() || self.speed <= 0.0 {
            self.done = true;
        }
        let mut fly = if dt.is_finite() {
            (self.speed * dt).max(0.0)
        } else {
            f64::INFINITY
        };
        while !self.done {
            let reach = fly.min(self.range_left);
            let Some((i, s)) = self.first(reach, targets) else {
                self.advance(reach);
                if self.range_left <= EPS {
                    self.done = true;
                }
                break;
            };
            self.advance(s);
            fly -= s;
            let t = &targets[i];
            self.struck.push(t.id);
            out.strikes.push(Strike {
                target: t.id,
                time: self.time,
                factor: self.factor,
                projectile: self.projectile,
                branch: self.branch,
                via: self.via,
                at: self.pos,
                travelled: self.travelled,
            });
            if self.pierce > 0 {
                self.pierce -= 1;
                self.factor *= PIERCE_FACTOR;
                self.via = Via::Pierce;
            } else if self.fork > 0 {
                for (k, side) in [(1, 1.0), (2, -1.0)] {
                    let mut half = self.clone();
                    half.dir = self.dir.rotated(side * FORK_ANGLE);
                    half.fork -= 1;
                    half.factor *= FORK_FACTOR;
                    half.branch = self.branch.saturating_mul(2).saturating_add(k);
                    half.via = Via::Fork;
                    out.forks.push(half);
                }
                self.done = true;
            } else if self.chain > 0 {
                match self.chain_target(t, targets) {
                    Some(next) => {
                        let to = next.at();
                        let d = self.pos.distance(to);
                        if d > 0.0 {
                            self.dir = Point::new(to.x - self.pos.x, to.y - self.pos.y).unit();
                        }
                        self.range_left = d;
                        self.chain -= 1;
                        self.factor *= CHAIN_FACTOR;
                        self.via = Via::Chain;
                    }
                    None => self.done = true,
                }
            } else {
                self.done = true;
            }
        }
        out
    }
}

/// Every strike of `shot` with `powers` among still `targets`, by time (then projectile, then branch).
pub fn trace(shot: &Shot, powers: &Powers, targets: &[Target]) -> Vec<Strike> {
    let mut queue: VecDeque<Flight> = Flight::launch(shot, powers).into();
    let mut out = Vec::new();
    while let Some(mut f) = queue.pop_front() {
        let s = f.step(f64::INFINITY, targets);
        out.extend(s.strikes);
        queue.extend(s.forks);
    }
    out.sort_by(|a, b| {
        a.time
            .total_cmp(&b.time)
            .then(a.projectile.cmp(&b.projectile))
            .then(a.branch.cmp(&b.branch))
    });
    out
}

/// Seconds a projectile of `speed` (before `powers`) takes to fly `range` T; infinite when it does not move.
pub fn flight_time(range: f64, speed: f64, powers: &Powers) -> f64 {
    let v = speed * powers.speed;
    if v > 0.0 {
        range.max(0.0) / v
    } else {
        f64::INFINITY
    }
}
