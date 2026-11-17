//! The world's size and where things stand in it, from the window: how many screen pixels a world pixel is, the
//! floor, the arena on the dais between its two braziers, and where the bars go. The camera shows the world's pixels
//! with (0, 0) at the top left and y down, as the page measured them.

use bevy::camera::ScalingMode;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use pixfx_duel::data::UNIT;

/// How tall the bars at the top are, in world pixels.
pub const BARS_TALL: f32 = 26.0;
/// How far inside each brazier the arena's wall stands.
pub const LAMP_INSET: f32 = 14.0;
/// The widest the arena gets.
pub const SPAN_MAX: f32 = 420.0;

/// The world in the window.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    /// Screen (physical) pixels to a world pixel: a whole number, a multiple of `UNIT`.
    pub scale: f32,
    /// The window's scale factor (physical pixels to a logical one).
    pub dpr: f32,
    /// The world, in world pixels.
    pub w: f32,
    pub h: f32,
    /// The floor's row.
    pub ground: f32,
    /// The top of the bars.
    pub bar_top: f32,
    /// The skill icons' size.
    pub icon: f32,
    /// The braziers at the arena's ends.
    pub lamps: [f32; 2],
    /// The arena's walls (inside the braziers).
    pub left: f32,
    pub right: f32,
    /// How high a fighter may fly: its feet no higher than this row.
    pub sky: f32,
    /// Bumped each time the layout changes (the fight starts over in the new world).
    pub version: u32,
}

impl Layout {
    /// The layout of a window `pw` x `ph` physical pixels at scale factor `dpr`.
    pub fn of(pw: f32, ph: f32, dpr: f32, version: u32) -> Layout {
        let logical_w = pw / dpr.max(0.5);
        let narrow = logical_w < 760.0;
        let dpr_whole = dpr.round().clamp(1.0, 2.0);
        let mut scale = (if narrow {
            2.0
        } else if logical_w < 1200.0 {
            3.0
        } else {
            4.0
        }) * dpr_whole;
        let unit = UNIT as f32;
        scale = (scale - scale % unit).max(unit);
        let w = (pw / scale).ceil().max(64.0);
        let h = (ph / scale).ceil().max(64.0);
        let ground = h - if narrow { 14.0 } else { 22.0 };
        let span = (w - 16.0).min(SPAN_MAX);
        let lamps = [((w - span) / 2.0).round(), ((w + span) / 2.0).round()];
        let bar_top = 6.0;
        let sky = ground - (bar_top + BARS_TALL + 13.0) - 64.0;
        let icon = if h - ground >= 20.0 { 16.0 } else { 12.0 };
        Layout {
            scale,
            dpr,
            w,
            h,
            ground,
            bar_top,
            icon,
            lamps,
            left: lamps[0] + LAMP_INSET,
            right: lamps[1] - LAMP_INSET,
            sky,
            version,
        }
    }

    /// World pixel (`x`, `y`) (from the top left, y down) where the camera sees it.
    pub fn at(&self, x: f32, y: f32) -> Vec2 {
        Vec2::new(x - self.w / 2.0, self.h / 2.0 - y)
    }

    /// A length in the rules' pixels, in the world's.
    pub fn world(rules: f64) -> f32 {
        (rules / UNIT) as f32
    }
}

impl Default for Layout {
    fn default() -> Layout {
        Layout::of(1280.0, 720.0, 1.0, 0)
    }
}

/// The camera, with the screen shake added to where it looks.
#[derive(Component)]
pub struct MainCamera;

pub struct LayoutPlugin;

impl Plugin for LayoutPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Layout>()
            .add_systems(Startup, camera)
            .add_systems(Update, follow_window.in_set(crate::Frame::Pace));
    }
}

fn camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        MainCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::WindowSize,
            ..OrthographicProjection::default_2d()
        }),
    ));
}

/// The layout follows the window; the camera shows one world pixel as `scale` screen pixels.
fn follow_window(
    windows: Query<&Window, With<PrimaryWindow>>,
    mut layout: ResMut<Layout>,
    mut cameras: Query<&mut Projection, With<MainCamera>>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let (pw, ph) = (
        window.physical_width() as f32,
        window.physical_height() as f32,
    );
    if pw < 16.0 || ph < 16.0 {
        return;
    }
    let dpr = window.scale_factor();
    let next = Layout::of(pw, ph, dpr, layout.version);
    let same = next.w == layout.w
        && next.h == layout.h
        && next.scale == layout.scale
        && layout.version > 0;
    if !same {
        *layout = Layout {
            version: layout.version + 1,
            ..next
        };
    }
    for mut projection in &mut cameras {
        if let Projection::Orthographic(o) = projection.as_mut() {
            o.scale = dpr / layout.scale;
        }
    }
}
