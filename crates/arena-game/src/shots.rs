//! Screenshots for checking the look by eye: `ARENA_SHOTS=/tmp/arena:3,6,9` saves the window at 3, 6 and 9 seconds
//! (`/tmp/arena-3.png`, ...) and quits after the last.

use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};

#[derive(Resource)]
struct Shots {
    prefix: String,
    at: Vec<f32>,
    quit_at: Option<f32>,
}

pub struct ShotsPlugin;

impl Plugin for ShotsPlugin {
    fn build(&self, app: &mut App) {
        let Ok(spec) = std::env::var("ARENA_SHOTS") else {
            return;
        };
        let (prefix, times) = spec.split_once(':').unwrap_or((spec.as_str(), "5"));
        let mut at: Vec<f32> = times.split(',').filter_map(|t| t.parse().ok()).collect();
        at.sort_by(f32::total_cmp);
        let quit_at = at.last().map(|t| t + 1.5);
        app.insert_resource(Shots {
            prefix: prefix.to_string(),
            at,
            quit_at,
        })
        .add_systems(Update, shoot);
    }
}

fn shoot(
    mut commands: Commands,
    time: Res<Time>,
    mut shots: ResMut<Shots>,
    mut exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed_secs();
    while shots.at.first().is_some_and(|t| *t <= now) {
        let t = shots.at.remove(0);
        let path = format!("{}-{t}.png", shots.prefix);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
    if shots.quit_at.is_some_and(|q| now >= q) {
        exit.write(AppExit::Success);
    }
}
