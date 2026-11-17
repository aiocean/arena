//! The arena's duel as a Bevy game. The rules (`arena-duel`, themselves a bevy_ecs world stepped in whole ticks) run
//! inside the game as a non-send resource; what a step hands back (effect commands and events, `Out`) becomes
//! messages the rest of the game answers: the spells play as particles, the fight's feel shakes and stops the
//! screen, the HUD shows the bars and skills. Natively each corner's bot runs in wasmtime (the house AI from its
//! ONNX model); in the browser the page hands the game the modules and the game instantiates them.
//!
//! The world is measured in the page's pixels (a `UNIT`th of the rules'): one world pixel is a whole number of
//! screen pixels, and a fighter's sprite, `UNIT` times finer, a whole number too.

#![forbid(unsafe_code)]
// The workspace bans std's float rounding and transcendentals so the rules give the same bits everywhere; nothing
// here decides the fight (it only draws it), so the game uses them freely.
#![allow(clippy::disallowed_methods)]

pub mod bots;
pub mod feel;
pub mod fighters;
pub mod hud;
pub mod layers;
pub mod layout;
pub mod light;
pub mod pick;
pub use arena_vfx::raster;
pub mod rules;
pub mod scene;
pub mod sheet;
#[cfg(not(target_arch = "wasm32"))]
pub mod shots;
pub mod units;
pub mod vfx;
#[cfg(target_arch = "wasm32")]
pub mod web;

use bevy::prelude::*;

/// The order of a frame: the feel decides how fast the fight runs, the rules step, then everything is drawn and the
/// pictures go to the screen.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum Frame {
    /// The fight's pace this frame (hit-stop, slow motion) and the layout when the window changed.
    Pace,
    /// The rules step and their commands and events go out as messages.
    Rules,
    /// The feel, the effects, the fighters, the light and the HUD answer what the rules did.
    React,
    /// The world's pictures are painted (scene, effects, light, HUD).
    Paint,
    /// The painted pictures go to their textures.
    Upload,
}

/// How a game starts: its seed (a random one when `None`) and what sits in each corner.
#[derive(Resource, Clone, Debug)]
pub struct Start {
    pub seed: Option<u32>,
    pub corners: [bots::Corner; 2],
}

impl Default for Start {
    fn default() -> Start {
        Start {
            seed: None,
            corners: [bots::Corner::House, bots::Corner::House],
        }
    }
}

/// Where the game's assets are: beside the game's wasm on the page (`site/build.sh` copies them to `game/assets`),
/// and natively in the crate's `assets` (or `assets` beside the executable, when the crate is not where it was built).
fn assets_path() -> String {
    if cfg!(target_arch = "wasm32") {
        return "game/assets".into();
    }
    let built = concat!(env!("CARGO_MANIFEST_DIR"), "/assets");
    if std::path::Path::new(built).is_dir() {
        built.into()
    } else {
        "assets".into()
    }
}

/// The game, ready to run: a window over the canvas `#scene` in the browser, its own window natively.
pub fn app(start: Start) -> App {
    let mut app = App::new();
    app.add_plugins(
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(AssetPlugin {
                file_path: assets_path(),
                // the page serves no .meta files: asking for each would be a 404 a texture
                meta_check: bevy::asset::AssetMetaCheck::Never,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Arena".into(),
                    canvas: Some("#scene".into()),
                    fit_canvas_to_parent: true,
                    prevent_default_event_handling: false,
                    resolution: bevy::window::WindowResolution::new(1280, 720),
                    ..default()
                }),
                ..default()
            }),
    )
    .insert_resource(ClearColor(Color::srgb_u8(8, 6, 18)))
    .insert_resource(start)
    .configure_sets(
        Update,
        (
            Frame::Pace,
            Frame::Rules,
            Frame::React,
            Frame::Paint,
            Frame::Upload,
        )
            .chain(),
    )
    .add_plugins((
        layout::LayoutPlugin,
        layers::LayersPlugin,
        bots::BotsPlugin,
        rules::RulesPlugin,
        feel::FeelPlugin,
        scene::ScenePlugin,
        light::LightPlugin,
        vfx::VfxPlugin,
        fighters::FightersPlugin,
        units::UnitsPlugin,
        hud::HudPlugin,
    ));
    #[cfg(not(target_arch = "wasm32"))]
    app.add_plugins(shots::ShotsPlugin);
    #[cfg(target_arch = "wasm32")]
    app.add_plugins(web::WebPlugin);
    app
}
