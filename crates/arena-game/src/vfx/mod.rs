//! The spells' effects. Those drawn by an image model play from their drawn sheets, moved by code (`drawn`); nearly
//! all the rest are sprite sheets baked from the effect painters of `arena-vfx` (`sprites`): the rules play one
//! (`Out::Play`), move it (`Out::Move`) and stop it (`Out::Stop`), and each playing is an entity whose sprites show its
//! frame behind and over the fighters. An effect that reads the fighter it plays on,
//! or that fills the whole scene, is painted live on the world's pictures instead (`Playing`).

pub mod drawn;
pub mod sprites;

use crate::layers::{Canvases, Cleared};
use crate::layout::Layout;
use crate::rules::{Fx, Pace, Shown};
use arena_duel::out::{Out, PlayOpts};
pub use arena_vfx::{Effect, Instance, Paint, Spawn, kit, recipes};
use bevy::prelude::*;
use std::collections::BTreeMap;

/// An effect played by the game itself rather than the rules (the scene's braziers): at world pixel (`x`, `y`).
#[derive(Message, Clone, Debug)]
pub struct PlayEffect {
    pub fx: &'static str,
    pub x: f32,
    pub y: f32,
    pub opts: PlayOpts,
}

/// The effects painted live, by the rules' key, and those the rules left (stopped or keyless, ending by themselves).
#[derive(Resource, Default)]
pub struct Playing {
    pub keyed: BTreeMap<u32, Box<dyn Instance>>,
    pub loose: Vec<Box<dyn Instance>>,
    /// The fight these belong to (a new fight sweeps them away).
    fight: u32,
}

impl Playing {
    /// Paint effect `fx` live from `at` on; an id with no painter plays nothing.
    pub fn play(&mut self, key: Option<u32>, fx: &str, at: &Spawn) {
        let Some(effect) = recipes::find(fx) else {
            return;
        };
        let instance = effect.spawn(at);
        match key {
            Some(k) => {
                if let Some(old) = self.keyed.insert(k, instance) {
                    self.loose.push(old);
                }
            }
            None => self.loose.push(instance),
        }
    }
}

/// The effects: their commands handled, then each played on by the fight's time.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Played;

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Playing>()
            .add_message::<PlayEffect>()
            .add_plugins((sprites::SpritesPlugin, drawn::DrawnPlugin))
            .add_systems(
                Update,
                (live, advance)
                    .chain()
                    .in_set(Played)
                    .in_set(crate::Frame::React),
            )
            .add_systems(Update, paint.in_set(crate::Frame::Paint).after(Cleared));
    }
}

/// The rules' commands for the effects painted live: play, move, stop.
fn live(
    mut fx: MessageReader<Fx>,
    mut playing: ResMut<Playing>,
    layout: Res<Layout>,
    shown: Res<Shown>,
) {
    if playing.fight != shown.fight {
        *playing = Playing {
            fight: shown.fight,
            ..default()
        };
    }
    for Fx(out) in fx.read() {
        match out {
            Out::Play {
                key,
                fx,
                x,
                y,
                opts,
            } if !arena_vfx::baked(fx) => {
                let at = Spawn::of(Layout::world(*x), Layout::world(*y), layout.ground, opts);
                playing.play(Some(*key), fx, &at);
            }
            Out::Move { key, x, y } => {
                if let Some(i) = playing.keyed.get_mut(key) {
                    i.set_position(Layout::world(*x), Layout::world(*y));
                }
            }
            Out::Stop { key } => {
                if let Some(mut i) = playing.keyed.remove(key) {
                    i.stop();
                    playing.loose.push(i);
                }
            }
            _ => {}
        }
    }
}

/// Every live effect moves on by the fight's time; those over are dropped.
fn advance(mut playing: ResMut<Playing>, pace: Res<Pace>) {
    let dt = pace.run as f32;
    let p = playing.as_mut();
    p.keyed.retain(|_, i| i.update(dt));
    p.loose.retain_mut(|i| i.update(dt));
}

fn paint(playing: Res<Playing>, mut canvases: ResMut<Canvases>) {
    let c = canvases.as_mut();
    let mut paint = Paint {
        back: &mut c.back,
        front: &mut c.front,
    };
    for i in playing.keyed.values().chain(playing.loose.iter()) {
        i.paint(&mut paint);
    }
}
