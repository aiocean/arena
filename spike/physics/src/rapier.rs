//! rapier2d-f64: a collider set and its BVH broad phase, moved each tick, queried through its QueryPipeline.

use crate::{Backend, Geo};
use rapier2d_f64::parry::query::DefaultQueryDispatcher;
use rapier2d_f64::prelude::*;

const HURT: usize = 4;
const BALLS: usize = 96;

pub struct Rapier {
    bodies: RigidBodySet,
    colliders: ColliderSet,
    bvh: BroadPhaseBvh,
    params: IntegrationParameters,
    hurt: [Vec<ColliderHandle>; 2],
    balls: [Vec<ColliderHandle>; 2],
}

fn group(f: usize, ball: bool) -> InteractionGroups {
    let g = Group::from_bits_truncate(1 << (f * 2 + ball as usize));
    InteractionGroups::new(g, Group::ALL, InteractionTestMode::And)
}

fn only(f: usize, ball: bool) -> InteractionGroups {
    let g = Group::from_bits_truncate(1 << (f * 2 + ball as usize));
    InteractionGroups::new(Group::ALL, g, InteractionTestMode::And)
}

impl Rapier {
    pub fn new() -> Self {
        let mut colliders = ColliderSet::new();
        let mut hurt = [Vec::new(), Vec::new()];
        let mut balls = [Vec::new(), Vec::new()];
        for f in 0..2 {
            for _ in 0..HURT {
                hurt[f].push(
                    colliders.insert(
                        ColliderBuilder::cuboid(1.0, 1.0)
                            .collision_groups(group(f, false))
                            .build(),
                    ),
                );
            }
            for _ in 0..BALLS {
                balls[f].push(
                    colliders.insert(
                        ColliderBuilder::ball(1.0)
                            .collision_groups(group(f, true))
                            .build(),
                    ),
                );
            }
        }
        Rapier {
            bodies: RigidBodySet::new(),
            colliders,
            bvh: BroadPhaseBvh::new(),
            params: IntegrationParameters::default(),
            hurt,
            balls,
        }
    }

    fn put(&mut self, h: ColliderHandle, shape: Option<(SharedShape, Pose)>, id: u128) {
        let co = &mut self.colliders[h];
        match shape {
            Some((s, p)) => {
                co.set_shape(s);
                co.set_position(p);
                co.set_enabled(true);
                co.user_data = id;
            }
            None => {
                co.set_enabled(false);
                co.set_position(Pose::translation(1e7, 1e7));
            }
        }
        let aabb = co.compute_aabb();
        self.bvh.set_aabb(&self.params, h, aabb);
    }
}

impl Default for Rapier {
    fn default() -> Self {
        Self::new()
    }
}

impl Backend for Rapier {
    fn load(&mut self, g: &Geo) {
        for f in 0..2 {
            for k in 0..HURT {
                let h = self.hurt[f][k];
                let s = g.hurt[f].get(k).map(|b| {
                    let (p, c) = crate::parry::cuboid(*b);
                    (SharedShape::new(c), p)
                });
                self.put(h, s, f as u128);
            }
            for k in 0..BALLS {
                let h = self.balls[f][k];
                assert!(g.circles[f].len() <= BALLS, "{}", g.circles[f].len());
                let c = g.circles[f].get(k);
                debug_assert!(g.circles[f].len() <= BALLS);
                let s =
                    c.map(|&(_, x, y, r)| (SharedShape::ball(r.max(0.0)), Pose::translation(x, y)));
                self.put(h, s, c.map_or(0, |c| c.0 as u128));
            }
        }
    }
    fn strike(&mut self, g: &Geo, me: usize) -> bool {
        let (p, c) = crate::parry::cuboid(g.strike[me]);
        let enabled = |_: ColliderHandle, co: &Collider| co.is_enabled();
        let q = self.bvh.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.bodies,
            &self.colliders,
            QueryFilter::from(only(1 - me, false)).predicate(&enabled),
        );
        q.intersect_shape(p, &c).next().is_some()
    }
    fn probe(&mut self, g: &Geo, i: usize) -> bool {
        let (f, x, y, r) = g.probes[i];
        let c = Cuboid::new(Vector::new(r, r));
        let enabled = |_: ColliderHandle, co: &Collider| co.is_enabled();
        let q = self.bvh.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.bodies,
            &self.colliders,
            QueryFilter::from(only(f, false)).predicate(&enabled),
        );
        q.intersect_shape(Pose::translation(x, y), &c)
            .next()
            .is_some()
    }
    fn bolt(&mut self, g: &Geo, i: usize) -> Option<(u32, f64)> {
        let b = &g.bolts[i];
        let enabled = |_: ColliderHandle, co: &Collider| co.is_enabled();
        let q = self.bvh.as_query_pipeline(
            &DefaultQueryDispatcher,
            &self.bodies,
            &self.colliders,
            QueryFilter::from(only(1 - b.owner, true)).predicate(&enabled),
        );
        let ray = Ray::new(Vector::new(b.px, b.py), Vector::new(b.dx, b.dy));
        q.cast_ray(&ray, b.reach, true)
            .map(|(h, t)| (self.colliders[h].user_data as u32, t))
    }
}
