//! Every hit test of the arena, on parry (`parry2d-f64`, the collision library under Bevy's physics, Avian and
//! rapier; `docs/specs/2026-11-18-physics-design.md`): whether two shapes overlap, and where a point moving along a
//! line first enters one (a bolt's sweep, so a fast bolt cannot pass through a body between two ticks).
//!
//! **Touching is not overlapping**: two shapes overlap when they share some area, parry's contact between them deeper
//! than zero. Boxes stacked edge to edge (a fighter's legs under its torso) or a point on an edge do not.
//!
//! The numbers are f64 and parry is built with `enhanced-determinism` (libm, scalar maths): the server and a page's
//! WebAssembly get the same answers bit for bit. Any coordinates will do (pixels or T, `y` up or down), as long as
//! both shapes use the same; an infinite side is cut at [`FAR`].

use parry2d_f64::math::{Pose, Vector};
use parry2d_f64::query::{Ray, ShapeCastOptions, cast_shapes, contact};
use parry2d_f64::shape::{Ball, Cuboid};

/// How far an infinite side reaches: farther than any field.
pub const FAR: f64 = 1e9;

/// A shape to test.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Shape {
    /// An upright box from (`x0`, `y0`) to (`x1`, `y1`); either side may be the larger.
    Rect { x0: f64, y0: f64, x1: f64, y1: f64 },
    /// A circle; radius 0 is a point.
    Circle { x: f64, y: f64, r: f64 },
}

impl Shape {
    /// The box `[x0, y0, x1, y1]`, as the rules keep their boxes.
    pub fn rect(b: [f64; 4]) -> Shape {
        Shape::Rect {
            x0: b[0],
            y0: b[1],
            x1: b[2],
            y1: b[3],
        }
    }

    /// A circle at (`x`, `y`) of radius `r` (none below 0).
    pub fn circle(x: f64, y: f64, r: f64) -> Shape {
        Shape::Circle { x, y, r }
    }

    /// Its pose and parry's shape.
    fn parry(&self) -> (Pose, Part) {
        match *self {
            Shape::Rect { x0, y0, x1, y1 } => {
                let (x0, x1) = (far(x0.min(x1)), far(x0.max(x1)));
                let (y0, y1) = (far(y0.min(y1)), far(y0.max(y1)));
                (
                    Pose::translation((x0 + x1) / 2.0, (y0 + y1) / 2.0),
                    Part::Box(Cuboid::new(Vector::new((x1 - x0) / 2.0, (y1 - y0) / 2.0))),
                )
            }
            Shape::Circle { x, y, r } => {
                (Pose::translation(x, y), Part::Ball(Ball::new(r.max(0.0))))
            }
        }
    }
}

enum Part {
    Box(Cuboid),
    Ball(Ball),
}

impl Part {
    fn shape(&self) -> &dyn parry2d_f64::shape::Shape {
        match self {
            Part::Box(c) => c,
            Part::Ball(b) => b,
        }
    }
}

fn far(v: f64) -> f64 {
    v.clamp(-FAR, FAR)
}

/// Do `a` and `b` overlap (share some area; touching is not overlapping)?
pub fn overlap(a: &Shape, b: &Shape) -> bool {
    let ((pa, sa), (pb, sb)) = (a.parry(), b.parry());
    contact(&pa, sa.shape(), &pb, sb.shape(), 0.0)
        .ok()
        .flatten()
        .is_some_and(|c| c.dist < 0.0)
}

/// How far a point leaving `from` along `dir` (length 1) goes before it enters `s`, if it does within `reach`; 0 when
/// it starts inside.
pub fn enters(from: (f64, f64), dir: (f64, f64), reach: f64, s: &Shape) -> Option<f64> {
    let (pose, part) = s.parry();
    let ray = Ray::new(Vector::new(from.0, from.1), Vector::new(dir.0, dir.1));
    part.shape().cast_ray(&pose, &ray, reach.max(0.0), true)
}

/// How far a circle of radius `r` leaving `from` along `dir` (length 1) goes before it meets `s`, if it does within
/// `reach`; 0 when it starts overlapping. The circle sweeps a capsule: a bolt as thick as its drawn body. Radius 0 is
/// [`enters`].
pub fn sweeps(from: (f64, f64), dir: (f64, f64), reach: f64, r: f64, s: &Shape) -> Option<f64> {
    if r <= 0.0 {
        return enters(from, dir, reach, s);
    }
    let (pose, part) = s.parry();
    let ball = Ball::new(r);
    let options = ShapeCastOptions {
        max_time_of_impact: reach.max(0.0),
        stop_at_penetration: true,
        ..ShapeCastOptions::default()
    };
    cast_shapes(
        &Pose::translation(from.0, from.1),
        Vector::new(dir.0, dir.1),
        &ball,
        &pose,
        Vector::ZERO,
        part.shape(),
        options,
    )
    .ok()
    .flatten()
    .map(|hit| hit.time_of_impact)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_is_not_overlapping() {
        let legs = Shape::rect([-40.0, 0.0, 34.0, 109.0]);
        let torso = Shape::rect([-35.0, 109.0, 23.0, 233.0]);
        assert!(!overlap(&legs, &torso));
        assert!(overlap(&legs, &Shape::rect([-35.0, 108.5, 23.0, 233.0])));
        assert!(!overlap(&Shape::circle(34.0, 50.0, 0.0), &legs));
        assert!(overlap(&Shape::circle(33.5, 50.0, 0.0), &legs));
        assert!(!overlap(
            &Shape::circle(0.0, 0.0, 1.0),
            &Shape::circle(2.0, 0.0, 1.0)
        ));
        assert!(overlap(
            &Shape::circle(0.0, 0.0, 1.0),
            &Shape::circle(1.9, 0.0, 1.0)
        ));
    }

    #[test]
    fn infinite_sides_are_cut() {
        let column = Shape::rect([10.0, f64::NEG_INFINITY, 20.0, f64::INFINITY]);
        assert!(overlap(&column, &Shape::circle(15.0, 1e6, 0.0)));
        assert!(!overlap(&column, &Shape::circle(25.0, 0.0, 4.0)));
    }

    #[test]
    fn a_sweep_enters_where_it_meets() {
        let c = Shape::circle(10.0, 0.0, 2.0);
        let near = |t: Option<f64>, want: f64| t.is_some_and(|t| (t - want).abs() < 1e-12);
        assert!(near(enters((0.0, 0.0), (1.0, 0.0), 20.0, &c), 8.0));
        assert_eq!(enters((0.0, 0.0), (1.0, 0.0), 7.9, &c), None);
        assert_eq!(enters((0.0, 0.0), (-1.0, 0.0), 20.0, &c), None);
        assert_eq!(enters((10.0, 1.0), (1.0, 0.0), 20.0, &c), Some(0.0));
        let b = Shape::rect([5.0, -1.0, 6.0, 1.0]);
        assert!(near(enters((0.0, 0.0), (1.0, 0.0), 20.0, &b), 5.0));
    }

    #[test]
    fn a_thick_sweep_meets_sooner_and_wider() {
        let c = Shape::circle(10.0, 0.0, 2.0);
        let near = |t: Option<f64>, want: f64| t.is_some_and(|t| (t - want).abs() < 1e-9);
        assert!(near(sweeps((0.0, 0.0), (1.0, 0.0), 20.0, 3.0, &c), 5.0));
        // a point passes 4 above the circle; a circle of 3 does not
        assert_eq!(enters((0.0, 4.0), (1.0, 0.0), 20.0, &c), None);
        assert!(sweeps((0.0, 4.0), (1.0, 0.0), 20.0, 3.0, &c).is_some());
        assert_eq!(sweeps((0.0, 6.0), (1.0, 0.0), 20.0, 3.0, &c), None);
        assert_eq!(sweeps((9.0, 0.0), (1.0, 0.0), 20.0, 3.0, &c), Some(0.0));
        let b = Shape::rect([5.0, -1.0, 6.0, 1.0]);
        assert!(near(sweeps((0.0, 0.0), (1.0, 0.0), 20.0, 1.0, &b), 4.0));
        assert_eq!(
            sweeps((0.0, 0.0), (1.0, 0.0), 20.0, 0.0, &c),
            enters((0.0, 0.0), (1.0, 0.0), 20.0, &c)
        );
    }
}
