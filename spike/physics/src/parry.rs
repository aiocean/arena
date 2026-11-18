//! parry2d-f64 directly: shapes and queries, no world.

use crate::{Backend, Geo};
use parry2d_f64::math::{Pose, Vector};
use parry2d_f64::query::{Ray, RayCast, intersection_test};
use parry2d_f64::shape::{Ball, Cuboid};

pub struct Parry;

pub fn cuboid(b: [f64; 4]) -> (Pose, Cuboid) {
    (
        Pose::translation((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0),
        Cuboid::new(Vector::new((b[2] - b[0]) / 2.0, (b[3] - b[1]) / 2.0)),
    )
}

impl Backend for Parry {
    fn load(&mut self, _: &Geo) {}
    fn strike(&mut self, g: &Geo, me: usize) -> bool {
        let (p1, s1) = cuboid(g.strike[me]);
        g.hurt[1 - me].iter().any(|h| {
            let (p2, s2) = cuboid(*h);
            intersection_test(&p1, &s1, &p2, &s2).is_ok_and(|i| i.intersecting)
        })
    }
    fn probe(&mut self, g: &Geo, i: usize) -> bool {
        let (f, x, y, r) = g.probes[i];
        let (p1, s1) = (Pose::translation(x, y), Cuboid::new(Vector::new(r, r)));
        g.hurt[f].iter().any(|h| {
            let (p2, s2) = cuboid(*h);
            intersection_test(&p1, &s1, &p2, &s2).is_ok_and(|i| i.intersecting)
        })
    }
    fn bolt(&mut self, g: &Geo, i: usize) -> Option<(u32, f64)> {
        let b = &g.bolts[i];
        let ray = Ray::new(Vector::new(b.px, b.py), Vector::new(b.dx, b.dy));
        let mut best: Option<(u32, f64)> = None;
        for &(id, x, y, r) in &g.circles[1 - b.owner] {
            let ball = Ball::new(r.max(0.0));
            if let Some(s) = ball.cast_ray(&Pose::translation(x, y), &ray, b.reach, true)
                && best.is_none_or(|(_, t)| s < t)
            {
                best = Some((id, s));
            }
        }
        best
    }
}

/// parry with the rules' strict test: boxes that only touch do not overlap (a contact at a distance below zero).
pub struct ParryStrict;

fn strictly(p1: &Pose, s1: &Cuboid, p2: &Pose, s2: &Cuboid) -> bool {
    parry2d_f64::query::contact(p1, s1, p2, s2, 0.0)
        .ok()
        .flatten()
        .is_some_and(|c| c.dist < 0.0)
}

impl Backend for ParryStrict {
    fn load(&mut self, _: &Geo) {}
    fn strike(&mut self, g: &Geo, me: usize) -> bool {
        let (p1, s1) = cuboid(g.strike[me]);
        g.hurt[1 - me].iter().any(|h| {
            let (p2, s2) = cuboid(*h);
            strictly(&p1, &s1, &p2, &s2)
        })
    }
    fn probe(&mut self, g: &Geo, i: usize) -> bool {
        let (f, x, y, r) = g.probes[i];
        let (p1, s1) = (Pose::translation(x, y), Cuboid::new(Vector::new(r, r)));
        g.hurt[f].iter().any(|h| {
            let (p2, s2) = cuboid(*h);
            strictly(&p1, &s1, &p2, &s2)
        })
    }
    fn bolt(&mut self, g: &Geo, i: usize) -> Option<(u32, f64)> {
        Parry.bolt(g, i)
    }
}
