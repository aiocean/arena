//! The field a fight plays on, as plain geometry (design, 17.8): what the behaviour modules (`projectile`, `motion`,
//! `device`, `charge`) and the area rules read. No engine and no bodies: a fight (the headless simulator, the duel,
//! a Bevy port) describes its targets with [`Target`] and asks.
//!
//! The field is a side view in T (a fighter's height): `x` runs along the floor from the left wall (0) to the right
//! wall ([`Field::width`]), `y` up from the floor. A body is a circle: a centre and a radius.
//!
//! Areas: an increase to [`Stat::Area`] scales the *area*, so a radius scales by its square root (PoE's rule): 40%
//! increased area is `√1.4 ≈ 1.183` times the radius. Skill durations (zones, buffs, curses a skill lays) scale by
//! [`Stat::SkillDuration`].

use crate::stat::{Stat, Table};

/// A point (or a vector) on the field, in T.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    /// The point `(x, y)`.
    pub const fn new(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    /// The distance to `o`.
    pub fn distance(self, o: Point) -> f64 {
        libm::hypot(self.x - o.x, self.y - o.y)
    }

    /// Its length.
    pub fn length(self) -> f64 {
        libm::hypot(self.x, self.y)
    }

    /// The same direction at length 1; `+x` for a zero vector.
    pub fn unit(self) -> Point {
        let l = self.length();
        if l > 0.0 && l.is_finite() {
            Point::new(self.x / l, self.y / l)
        } else {
            Point::new(1.0, 0.0)
        }
    }

    /// Turned by `angle` radians, counter-clockwise (`y` up).
    pub fn rotated(self, angle: f64) -> Point {
        let (s, c) = (libm::sin(angle), libm::cos(angle));
        Point::new(self.x * c - self.y * s, self.x * s + self.y * c)
    }

    /// `self + d × k`.
    pub fn plus(self, d: Point, k: f64) -> Point {
        Point::new(self.x + d.x * k, self.y + d.y * k)
    }
}

/// Something that can be struck: an id the fight knows it by, its centre and radius, whether it still stands, and
/// whether the shot asking has struck it already (a real-time fight carries that between frames; a projectile never
/// strikes the same target twice).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Target {
    pub id: u32,
    pub x: f64,
    pub y: f64,
    pub radius: f64,
    pub alive: bool,
    pub hit_before: bool,
}

impl Target {
    /// A live target never struck, at `(x, y)` with `radius`.
    pub const fn new(id: u32, x: f64, y: f64, radius: f64) -> Target {
        Target {
            id,
            x,
            y,
            radius,
            alive: true,
            hit_before: false,
        }
    }

    /// Its centre.
    pub fn at(&self) -> Point {
        Point::new(self.x, self.y)
    }
}

/// The field: two walls, at 0 and `width`.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Field {
    pub width: f64,
}

impl Field {
    /// `x` held so a body of `radius` stays between the walls (the middle, on a field narrower than the body).
    pub fn clamp_x(&self, x: f64, radius: f64) -> f64 {
        let (lo, hi) = (radius, self.width - radius);
        if lo > hi {
            self.width / 2.0
        } else {
            x.clamp(lo, hi)
        }
    }
}

/// The shape of an area: a circle (a blast, a nova, an aura), or a band, an upright rectangle (a zone on the floor,
/// a line).
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Shape {
    Circle {
        centre: Point,
        radius: f64,
    },
    Band {
        left: f64,
        right: f64,
        bottom: f64,
        top: f64,
    },
}

impl Shape {
    /// Whether `t` is inside: alive and its body touching the shape (an overlap counts, an edge touching too).
    pub fn holds(&self, t: &Target) -> bool {
        if !t.alive {
            return false;
        }
        match *self {
            Shape::Circle { centre, radius } => centre.distance(t.at()) <= radius + t.radius,
            Shape::Band {
                left,
                right,
                bottom,
                top,
            } => {
                // the nearest point of the band to the centre, then the distance to it
                let nx = t.x.clamp(left.min(right), left.max(right));
                let ny = t.y.clamp(bottom.min(top), bottom.max(top));
                Point::new(nx, ny).distance(t.at()) <= t.radius
            }
        }
    }

    /// The ids of the targets inside, in the order given.
    pub fn inside(&self, targets: &[Target]) -> Vec<u32> {
        targets
            .iter()
            .filter(|t| self.holds(t))
            .map(|t| t.id)
            .collect()
    }
}

/// The factor a skill's area takes from its table (`(1 + increased) × more`, never below zero).
pub fn area_factor(t: &Table) -> f64 {
    t.get(Stat::Area, 1.0).max(0.0)
}

/// A radius (or any length of an area: a band's half-width) after [`Stat::Area`]: `base × √area`.
pub fn radius(base: f64, t: &Table) -> f64 {
    base * libm::sqrt(area_factor(t))
}

/// How long a skill's own effect lasts (a zone, a buff, a curse it lays) after [`Stat::SkillDuration`], never below
/// zero.
pub fn duration(base: f64, t: &Table) -> f64 {
    t.get(Stat::SkillDuration, base).max(0.0)
}
