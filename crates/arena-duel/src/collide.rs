//! The fight's shapes and every test of one against another, on `arena_calc::collide` (parry; touching is not
//! overlapping). A fighter is struck where it is drawn, on the hurtboxes of the frame it shows (`frames::hurtboxes`);
//! a blow, a bolt, a field or a blast is a shape made here, and a unit's body too. `debug::shapes` draws these same
//! shapes, so what the hitbox overlay shows is what the rules test.
//!
//! Coordinates are the arena's pixels: `x` from the arena's left as the rules keep it, `y` up from the ground (the
//! rules' `air`).

use crate::fighter::State;
use crate::unit::Unit;
pub use arena_calc::collide::{FAR, Shape, enters, overlap};

/// How far apart two bolts meet head on: their places within these of each other, along x and up (each bolt a box of
/// half these, `bolt_meets`).
pub const BOLTS_MEET: (f64, f64) = (24.0, 32.0);
/// How near a bolt comes to an orb's ball to meet it: each a circle of half this.
pub const ORB_MEETS: f64 = 40.0;

/// Fighter `f`'s body: the hurtboxes of the frame it shows.
pub fn body(state: &State, f: usize) -> Vec<Shape> {
    crate::frames::hurtboxes(state, f)
        .into_iter()
        .map(Shape::rect)
        .collect()
}

/// How much farther apart along x two sets of boxes must stand for none of `a`'s to overlap one of `b`'s with `gap`
/// between them, `b` going on side `dir` of `a` (1 right, -1 left): 0 when they are clear. Each box is grown by half
/// the gap and tested on parry; the boxes that overlap say how far.
pub fn overlap_x(a: &[[f64; 4]], b: &[[f64; 4]], dir: f64, gap: f64) -> f64 {
    let h = gap / 2.0;
    let grown = |r: &[f64; 4]| Shape::rect([r[0] - h, r[1], r[2] + h, r[3]]);
    let mut most: f64 = 0.0;
    for p in a {
        for q in b {
            if overlap(&grown(p), &grown(q)) {
                let need = if dir > 0.0 {
                    p[2] + gap - q[0]
                } else {
                    q[2] + gap - p[0]
                };
                most = most.max(need);
            }
        }
    }
    most
}

/// How far apart two fighters keep: their hurtboxes this far clear of each other (`BodySpec::room`), so that the
/// drawn figures, wider than the boxes inside them, barely touch.
pub fn body_gap(state: &State) -> f64 {
    let [a, b] = &state.fighters;
    (a.body.spec.room + b.body.spec.room) / 2.0
}

/// The side fighter 1 stands on from fighter 0 (1 right, -1 left; when one is on top of the other, the way 0
/// faces), and how much farther apart along x their bodies must stand not to overlap (`overlap_x`, `body_gap`).
pub fn bodies_overlap(state: &State) -> (f64, f64) {
    let [a, b] = &state.fighters;
    let dx = b.body.x - a.body.x;
    let dir = if dx > 0.0 {
        1.0
    } else if dx < 0.0 {
        -1.0
    } else {
        a.body.face
    };
    let need = overlap_x(
        &crate::frames::hurtboxes(state, 0),
        &crate::frames::hurtboxes(state, 1),
        dir,
        body_gap(state),
    );
    (dir, need)
}

/// Does `s` overlap fighter `f`'s body?
pub fn touches(state: &State, f: usize, s: &Shape) -> bool {
    body(state, f).iter().any(|h| overlap(h, s))
}

/// A point (`x`, `y`) give or take `r` either way: the square a bolt or an orb strikes with (`shot::overlaps`).
pub fn probe(x: f64, y: f64, r: f64) -> Shape {
    Shape::rect([x - r, y - r, x + r, y + r])
}

/// A field from `x - radius` to `x + radius`, from `level` below `air` to `level` above it (no lower than the
/// ground; an infinite level is the whole height): a zone, a vortex, a blast, a lunge's path.
pub fn area(lo: f64, hi: f64, air: f64, level: f64) -> Shape {
    Shape::rect([lo, (air - level).max(0.0), hi, air + level])
}

/// A unit's body: a box as wide as its radius either side and twice it high, standing on the ground.
pub fn unit_body(u: &Unit) -> Shape {
    let r = u.radius();
    Shape::rect([u.x - r, 0.0, u.x + r, 2.0 * r])
}

/// Do two bolts at (`ax`, `ay`) and (`bx`, `by`) meet?
pub fn bolt_meets(a: (f64, f64), b: (f64, f64)) -> bool {
    let (w, h) = (BOLTS_MEET.0 / 2.0, BOLTS_MEET.1 / 2.0);
    overlap(
        &Shape::rect([a.0 - w, a.1 - h, a.0 + w, a.1 + h]),
        &Shape::rect([b.0 - w, b.1 - h, b.0 + w, b.1 + h]),
    )
}

/// Does a bolt at `a` meet an orb's ball at `b` (both `y` the same way up)?
pub fn orb_meets(a: (f64, f64), b: (f64, f64)) -> bool {
    let r = ORB_MEETS / 2.0;
    overlap(&Shape::circle(a.0, a.1, r), &Shape::circle(b.0, b.1, r))
}
