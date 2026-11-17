//! The fight's feel: hit-stop and the knockout's slow motion (the pace the rules run at), the screen's shake, the
//! jolt of the one struck, damage numbers, qi streaks and the dust a fall raises, as the page's hero plays the
//! engine's `FightFeel` (`duel-hero.js`, `duel-art.js`).
//!
//! A frame: in `Frame::Pace` the feel lets the frame's time pass and says how much of it the fight runs (`Pace`); in
//! `Frame::React`, set `Felt`, it answers the step's events, jolts the one struck (`Jolts`) and moves the camera for
//! the shake (whole world pixels; the HUD's picture follows the camera, so it stays still). Then it paints, after
//! everything else in `Frame::Paint` (the sets `Overlay`, before the pictures go up): the light goes on, and the
//! dust, streaks and numbers go on `front`.

pub mod bubble;
pub mod draw;
pub mod fight;
pub mod math;

use crate::layers::Canvases;
use crate::layout::{Layout, MainCamera};
use crate::rules::{Fx, Pace, Shown};
use bevy::prelude::*;
use fight::FightFeel;
use arena_duel::data::Look;

/// Whole world pixels the screen's answer to a blow jolts each fighter off its place.
#[derive(Resource, Default, Clone, Copy)]
pub struct Jolts(pub [[i32; 2]; 2]);

/// After the feel has answered this frame's commands and events.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Felt;

/// What is painted over the frame's pictures once `Frame::Paint` is done, in this order, before `Frame::Upload`:
/// the fight's light, then the marks of the fight (dust, streaks, numbers) on `front`.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Overlay {
    Light,
    Marks,
}

/// The feel of the fight now playing.
#[derive(Resource, Default)]
pub struct Feel {
    pub fight: FightFeel,
    /// The fight it answers (a new one starts calm).
    of: u32,
}

pub struct FeelPlugin;

impl Plugin for FeelPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(bubble::BubblePlugin);
        app.init_resource::<Jolts>()
            .init_resource::<Feel>()
            .configure_sets(
                Update,
                (Overlay::Light, Overlay::Marks)
                    .chain()
                    .after(crate::Frame::Paint)
                    .before(crate::Frame::Upload),
            )
            .add_systems(Update, pace.in_set(crate::Frame::Pace))
            .add_systems(Update, react.in_set(Felt).in_set(crate::Frame::React))
            .add_systems(Update, marks.in_set(Overlay::Marks));
    }
}

/// The frame's time passes; the fight runs what the feel gives it.
fn pace(time: Res<Time>, mut pace: ResMut<Pace>, mut feel: ResMut<Feel>) {
    let dt = time.delta_secs_f64();
    let run = feel.fight.advance(dt);
    *pace = Pace { dt, run };
}

/// The step's events: the feel answers, the one struck is jolted, the camera shakes.
fn react(
    shown: Res<Shown>,
    mut fx: MessageReader<Fx>,
    mut feel: ResMut<Feel>,
    mut jolts: ResMut<Jolts>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
) {
    if feel.of != shown.fight {
        *feel = Feel {
            of: shown.fight,
            ..default()
        };
    }
    for Fx(out) in fx.read() {
        feel.fight.react(out);
    }
    jolts.0 = feel.fight.jolts();
    // the page moves its world right and down by the shake: the camera goes the other way (y up)
    let (sx, sy) = feel.fight.shake();
    for mut t in &mut cameras {
        t.translation.x = -sx as f32;
        t.translation.y = sy as f32;
    }
}

/// Each fighter's look, for the colours of its streaks and numbers.
pub fn looks(shown: &Shown) -> [Look; 2] {
    shown
        .view
        .as_ref()
        .map(|v| [v.fighters[0].look, v.fighters[1].look])
        .unwrap_or([Look::Thunder, Look::Frost])
}

/// The dust, the streaks and the numbers over the fighters.
fn marks(feel: Res<Feel>, shown: Res<Shown>, layout: Res<Layout>, mut canvases: ResMut<Canvases>) {
    let ground = f64::from(layout.ground);
    let looks = looks(&shown);
    let front = &mut canvases.front;
    draw::dust(front, &feel.fight.dust(), ground);
    draw::streaks(front, &feel.fight.streaks(), ground, looks);
    draw::numbers(front, &feel.fight.numbers(), ground, looks);
}
