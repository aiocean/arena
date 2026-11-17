//! The fight's feel: hit-stop and the knockout's slow motion (the pace the rules run at), the screen's shake, the
//! jolt of the one struck, damage numbers, qi streaks and the dusk of an ultimate.

use crate::rules::Pace;
use bevy::prelude::*;

/// Whole world pixels the screen's answer to a blow jolts each fighter off its place.
#[derive(Resource, Default, Clone, Copy)]
pub struct Jolts(pub [[i32; 2]; 2]);

/// After the feel has answered this frame's commands and events.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct Felt;

pub struct FeelPlugin;

impl Plugin for FeelPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Jolts>()
            .add_systems(Update, pace.in_set(crate::Frame::Pace));
    }
}

fn pace(time: Res<Time>, mut pace: ResMut<Pace>) {
    let dt = time.delta_secs_f64();
    *pace = Pace { dt, run: dt };
}
