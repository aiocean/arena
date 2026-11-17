//! The fight itself: the rules' duel, made for the world the window shows and the corners as seated, stepped by the
//! frame's pace (the feel's hit-stop and slow motion), its commands and events handed on as `Fx` messages and the
//! duel as it stands this frame as `Shown`. A new layout or new corners start the fight over on the same seed.

use crate::bots::Seating;
use crate::layout::Layout;
use bevy::prelude::*;
use pixfx_duel::data::UNIT;
use pixfx_duel::duel::{Duel, DuelConfig};
use pixfx_duel::out::Out;
use pixfx_duel::view::{DuelView, view};

/// A command or event of this frame's step, in the rules' pixels (`Layout::world` brings a length to the world's).
#[derive(Message, Clone, Debug)]
pub struct Fx(pub Out);

/// How far the fight runs this frame, in seconds: the frame's time, or less in a hit-stop or a knockout's slow
/// motion (the feel sets it before the rules step).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct Pace {
    /// The frame's own time.
    pub dt: f64,
    /// The fight's.
    pub run: f64,
}

/// The duel as it stands after this frame's step, for everything that draws it.
#[derive(Resource, Default, Clone)]
pub struct Shown {
    pub view: Option<DuelView>,
    pub seed: u32,
    pub names: [String; 2],
    /// Bumped each time a new fight starts.
    pub fight: u32,
}

/// The duel (it holds its seats, which hold bot instances: it stays on the main thread).
#[derive(Default)]
pub struct Fight {
    pub duel: Option<Duel>,
    /// The layout and seating it was made for.
    made_for: (u32, u32),
}

pub struct RulesPlugin;

impl Plugin for RulesPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send(Fight::default())
            .init_resource::<Pace>()
            .init_resource::<Shown>()
            .add_message::<Fx>()
            .add_systems(
                Update,
                (begin_fight, step).chain().in_set(crate::Frame::Rules),
            );
    }
}

/// The fight's config in the rules' pixels for `layout`.
pub fn config(layout: &Layout, seed: u32) -> DuelConfig {
    let unit = UNIT;
    DuelConfig {
        seed,
        left: layout.left as f64 * unit,
        right: layout.right as f64 * unit,
        ground: layout.ground as f64 * unit,
        sky: layout.sky as f64 * unit,
        effects: pixfx_duel::effects::all(),
    }
}

fn begin_fight(
    layout: Res<Layout>,
    seating: NonSend<Seating>,
    mut fight: NonSendMut<Fight>,
    mut shown: ResMut<Shown>,
) {
    let wanted = (layout.version, seating.version);
    if fight.made_for == wanted && fight.duel.is_some() {
        return;
    }
    let Some(seated) = &seating.seated else {
        return;
    };
    // a fighter's seat keeps its bot instance: the corners start the new fight with a new round
    let seats = [seated[0].make(), seated[1].make()];
    fight.duel = Some(Duel::new(config(&layout, seating.seed), seats));
    fight.made_for = wanted;
    shown.seed = seating.seed;
    shown.names = seating.names.clone();
    shown.fight += 1;
}

fn step(
    pace: Res<Pace>,
    mut fight: NonSendMut<Fight>,
    mut shown: ResMut<Shown>,
    mut fx: MessageWriter<Fx>,
) {
    let Some(duel) = fight.duel.as_mut() else {
        return;
    };
    for out in duel.step(pace.run) {
        fx.write(Fx(out));
    }
    shown.view = Some(view(duel.state()));
}
