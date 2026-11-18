//! avian2d (f64 positions, enhanced-determinism): an App running its PhysicsPlugins on a schedule stepped by hand,
//! kinematic bodies moved and reshaped each tick, queried through SpatialQuery.

use crate::{Backend, Geo};
use avian2d::parry::shape::SharedShape;
use avian2d::prelude::*;
use bevy::ecs::schedule::ScheduleLabel;
use bevy::ecs::system::SystemState;
use bevy::math::{DVec2, Dir2, Vec2};
use bevy::prelude::*;
use core::time::Duration;

#[derive(ScheduleLabel, Hash, Debug, Clone, PartialEq, Eq)]
struct Step;

const HURT: usize = 4;
const BALLS: usize = 96; // a lying fighter is a long row of circles

pub struct Avian {
    app: App,
    hurt: [Vec<Entity>; 2],
    balls: [Vec<Entity>; 2],
    query: SystemState<SpatialQuery<'static, 'static>>,
    last: std::collections::HashMap<Entity, Option<(u64, u64, u64, u64)>>,
}

fn layer(f: usize, ball: bool) -> u32 {
    1 << (f * 2 + ball as usize)
}

#[derive(Component)]
struct Id(u32);

impl Avian {
    pub fn new() -> Self {
        let mut app = App::new();
        app.add_plugins((
            bevy::app::TaskPoolPlugin::default(),
            bevy::transform::TransformPlugin,
            PhysicsPlugins::new(Step),
        ));
        app.insert_resource(Gravity(Vec2::ZERO));
        app.init_resource::<Time>();
        app.finish();
        app.cleanup();
        let mut hurt = [Vec::new(), Vec::new()];
        let mut balls = [Vec::new(), Vec::new()];
        let w = app.world_mut();
        for f in 0..2 {
            for _ in 0..HURT {
                hurt[f].push(
                    w.spawn((
                        RigidBody::Kinematic,
                        Collider::from(SharedShape::cuboid(1.0, 1.0)),
                        CollisionLayers::new(layer(f, false), 0u32),
                        Position(DVec2::new(1e7, 1e7)),
                        Id(f as u32),
                    ))
                    .id(),
                );
            }
            for _ in 0..BALLS {
                balls[f].push(
                    w.spawn((
                        RigidBody::Kinematic,
                        Collider::from(SharedShape::ball(1.0)),
                        CollisionLayers::new(layer(f, true), 0u32),
                        Position(DVec2::new(1e7, 1e7)),
                        Id(f as u32),
                    ))
                    .id(),
                );
            }
        }
        let query = SystemState::new(app.world_mut());
        Avian {
            app,
            hurt,
            balls,
            query,
            last: Default::default(),
        }
    }

    /// Moves slot `e`, reshaping it only when its size changed (`key`: its half extents' bits).
    fn put(&mut self, e: Entity, shape: Option<(SharedShape, DVec2, (u64, u64))>) {
        let key = shape
            .as_ref()
            .map(|(_, p, k)| (k.0, k.1, p.x.to_bits(), p.y.to_bits()));
        let was = self.last.get(&e).copied().flatten();
        if was == key && self.last.contains_key(&e) {
            return;
        }
        let same_size = matches!((was, key), (Some(a), Some(b)) if a.0 == b.0 && a.1 == b.1);
        self.last.insert(e, key);
        let mut ent = self.app.world_mut().entity_mut(e);
        match shape {
            Some((s, p, _)) => {
                if same_size {
                    ent.insert(Position(p));
                } else {
                    ent.insert((Collider::from(s), Position(p)));
                }
            }
            None => {
                ent.insert(Position(DVec2::new(1e7, 1e7)));
            }
        }
    }
}

impl Default for Avian {
    fn default() -> Self {
        Self::new()
    }
}

fn shape(b: [f64; 4]) -> (SharedShape, DVec2) {
    (
        SharedShape::cuboid((b[2] - b[0]) / 2.0, (b[3] - b[1]) / 2.0),
        DVec2::new((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0),
    )
}

fn keyed(b: [f64; 4]) -> (SharedShape, DVec2, (u64, u64)) {
    let (s, p) = shape(b);
    (s, p, ((b[2] - b[0]).to_bits(), (b[3] - b[1]).to_bits()))
}

impl Backend for Avian {
    fn load(&mut self, g: &Geo) {
        for f in 0..2 {
            for k in 0..HURT {
                self.put(self.hurt[f][k], g.hurt[f].get(k).map(|b| keyed(*b)));
            }
            assert!(g.circles[f].len() <= BALLS);
            for k in 0..BALLS {
                let s = g.circles[f].get(k).map(|&(_, x, y, r)| {
                    (
                        SharedShape::ball(r.max(0.0)),
                        DVec2::new(x, y),
                        (r.to_bits(), 0),
                    )
                });
                self.put(self.balls[f][k], s);
            }
        }
        let w = self.app.world_mut();
        w.resource_mut::<Time>()
            .advance_by(Duration::from_secs_f64(1.0 / 60.0));
        w.run_schedule(Step);
    }
    fn strike(&mut self, g: &Geo, me: usize) -> bool {
        let (s, p) = shape(g.strike[me]);
        let q = self.query.get(self.app.world()).unwrap();
        !q.shape_intersections(
            &Collider::from(s),
            p,
            0.0,
            &SpatialQueryFilter::from_mask(layer(1 - me, false)),
        )
        .is_empty()
    }
    fn probe(&mut self, g: &Geo, i: usize) -> bool {
        let (f, x, y, r) = g.probes[i];
        let q = self.query.get(self.app.world()).unwrap();
        !q.shape_intersections(
            &Collider::from(SharedShape::cuboid(r, r)),
            DVec2::new(x, y),
            0.0,
            &SpatialQueryFilter::from_mask(layer(f, false)),
        )
        .is_empty()
    }
    fn bolt(&mut self, g: &Geo, i: usize) -> Option<(u32, f64)> {
        let b = &g.bolts[i];
        let world = self.app.world();
        let q = self.query.get(world).unwrap();
        let dir = Dir2::new(Vec2::new(b.dx as f32, b.dy as f32)).ok()?;
        let hit = q.cast_ray(
            DVec2::new(b.px, b.py),
            dir,
            b.reach as f32,
            true,
            &SpatialQueryFilter::from_mask(layer(1 - b.owner, true)),
        )?;
        let id = world.get::<Id>(hit.entity)?.0;
        Some((id, f64::from(hit.distance)))
    }
}
