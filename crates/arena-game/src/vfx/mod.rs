//! The spells and the arena's lamps as pixel particles: each effect id the rules play (`Out::Play`) is an `Effect`
//! (`recipes`) that starts an `Instance` at its place; the rules move it (`Out::Move`) and stop it (`Out::Stop`), and
//! one that does not loop ends on its own. Every instance paints itself each frame on the world's pictures, behind the
//! fighters or over them, its colour and its light (added).

pub mod kit;
pub mod recipes;

use crate::layers::{Canvases, Cleared};
use crate::layout::Layout;
use crate::raster::{Canvas, Rgba, hex};
use crate::rules::{Fx, Pace};
use bevy::prelude::*;
use pixfx_duel::out::{Out, PlayOpts};
use std::collections::BTreeMap;

/// How an effect is played, in world pixels: where, its school's colours (from the rules), its size, which way it
/// faces, its seed.
#[derive(Clone, Debug, Default)]
pub struct Spawn {
    pub x: f32,
    pub y: f32,
    /// The ground's row (an effect that falls or splashes stops there).
    pub ground: f32,
    pub palette: Option<Vec<Rgba>>,
    pub accent: Option<Vec<Rgba>>,
    pub stars: Option<Vec<Rgba>>,
    pub fire: Option<Vec<Rgba>>,
    pub glow: Option<Rgba>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Faces left.
    pub flip: bool,
    pub seed: u32,
}

impl Spawn {
    /// The effect's palette, or `default` when the rules gave none.
    pub fn palette_or(&self, default: &[&str]) -> Vec<Rgba> {
        self.palette
            .clone()
            .unwrap_or_else(|| default.iter().map(|c| hex(c)).collect())
    }

    /// Which way it faces: 1 right, -1 left.
    pub fn dir(&self) -> f32 {
        if self.flip { -1.0 } else { 1.0 }
    }
}

/// The two pictures an instance paints: behind the fighters (`back`) and over them (`front`). Light is added (`Blend::Add`)
/// on the same pictures.
pub struct Paint<'a> {
    pub back: &'a mut Canvas,
    pub front: &'a mut Canvas,
}

/// An effect: what starts when the rules play its id.
pub trait Effect: Send + Sync {
    /// Its id, as the rules name it (`"fire/brazier"`).
    fn id(&self) -> &'static str;
    /// Whether it plays until stopped (the rules stop it) rather than ending by itself.
    fn looping(&self) -> bool;
    /// One playing of it.
    fn spawn(&self, at: &Spawn) -> Box<dyn Instance>;
}

/// One playing effect.
pub trait Instance: Send + Sync {
    /// Moves on by `dt` seconds of the fight; false once it is over (a stopped effect fades out first).
    fn update(&mut self, dt: f32) -> bool;
    /// Where it is now (the rules move bolts and orbs each tick).
    fn set_position(&mut self, x: f32, y: f32);
    /// Asked to end: a looping effect fades out and then is over.
    fn stop(&mut self);
    /// Paints this frame.
    fn paint(&self, paint: &mut Paint);
}

/// The effects playing, by the rules' key, and those the rules left (stopped or keyless, ending by themselves).
#[derive(Resource, Default)]
pub struct Playing {
    pub keyed: BTreeMap<u32, Box<dyn Instance>>,
    pub loose: Vec<Box<dyn Instance>>,
    /// The fight these belong to (a new fight sweeps them away).
    fight: u32,
}

impl Playing {
    /// Start effect `fx` at `at`; an id with no recipe plays nothing.
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

pub struct VfxPlugin;

impl Plugin for VfxPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Playing>()
            .add_systems(Update, (play, advance).chain().in_set(crate::Frame::React))
            .add_systems(Update, paint.in_set(crate::Frame::Paint).after(Cleared));
    }
}

fn colors(c: Option<&'static [&'static str]>) -> Option<Vec<Rgba>> {
    c.map(|cs| cs.iter().map(|s| hex(s)).collect())
}

/// How an effect's play options read in world pixels.
pub fn spawn_of(x: f64, y: f64, opts: &PlayOpts, ground: f32) -> Spawn {
    Spawn {
        x: Layout::world(x),
        y: Layout::world(y),
        ground,
        palette: colors(opts.palette),
        accent: colors(opts.accent),
        stars: colors(opts.stars),
        fire: colors(opts.fire),
        glow: opts.glow.map(hex),
        width: opts.width.map(Layout::world),
        height: opts.height.map(Layout::world),
        flip: opts.flip.unwrap_or(false),
        seed: opts.seed.unwrap_or(1),
    }
}

/// The rules' effect commands: play, move, stop.
fn play(
    mut fx: MessageReader<Fx>,
    mut playing: ResMut<Playing>,
    layout: Res<Layout>,
    shown: Res<crate::rules::Shown>,
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
            } => {
                let at = spawn_of(*x, *y, opts, layout.ground);
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

/// Every effect moves on by the fight's time; those over are dropped.
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
