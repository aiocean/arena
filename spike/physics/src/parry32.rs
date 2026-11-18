//! rapier2d (f32) parry: every number cut to f32: shapes and queries, no world.

use crate::{Backend, Geo};
use rapier2d::parry::math::{Pose, Vector};
use rapier2d::parry::query::{Ray, RayCast, intersection_test};
use rapier2d::parry::shape::{Ball, Cuboid};

pub struct Parry32;

pub fn cuboid(b: [f64; 4]) -> (Pose, Cuboid) {
    let b = b.map(|v| v as f32);
    (
        Pose::translation((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0),
        Cuboid::new(Vector::new((b[2] - b[0]) / 2.0, (b[3] - b[1]) / 2.0)),
    )
}

impl Backend for Parry32 {
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
        let (x, y, r) = (x as f32, y as f32, r as f32);
        let (p1, s1) = (Pose::translation(x, y), Cuboid::new(Vector::new(r, r)));
        g.hurt[f].iter().any(|h| {
            let (p2, s2) = cuboid(*h);
            intersection_test(&p1, &s1, &p2, &s2).is_ok_and(|i| i.intersecting)
        })
    }
    fn bolt(&mut self, g: &Geo, i: usize) -> Option<(u32, f64)> {
        let b = &g.bolts[i];
        let ray = Ray::new(
            Vector::new(b.px as f32, b.py as f32),
            Vector::new(b.dx as f32, b.dy as f32),
        );
        let mut best: Option<(u32, f64)> = None;
        for &(id, x, y, r) in &g.circles[1 - b.owner] {
            let (x, y) = (x as f32, y as f32);
            let ball = Ball::new(r.max(0.0) as f32);
            if let Some(s) = ball
                .cast_ray(&Pose::translation(x, y), &ray, b.reach as f32, true)
                .map(f64::from)
                && best.is_none_or(|(_, t)| s < t)
            {
                best = Some((id, s));
            }
        }
        best
    }
}
