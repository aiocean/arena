//! Where the duel is fought (the page's `duel-scene.js`): a stone dais high in the mountains at night. A moon over
//! ranges of peaks fading into the distance, a pagoda on a far summit with one lamp lit, pines on the near ridge, and
//! bands of mist drifting between the ranges. The still part is painted once per layout into `Canvases.scene`, one
//! pixel per world pixel, from a fixed seed so it is the same on every load; the mist moves with the fight's clock
//! and is painted on `back` each frame, each band only where no nearer range or the floor stands in front of it.
//!
//! The decor plays as effects: when a fight starts, a brazier at each end of the arena and fireflies over the whole
//! scene. They are played in `Frame::Paint`, after the effects' own system has swept the last fight's effects away
//! (it does that in `Frame::React` when `Shown::fight` changes).

use crate::feel::math::{floor, js_max, js_min, js_round, sin};
use crate::layers::{Canvases, Cleared};
use crate::layout::{BARS_TALL, Layout};
use crate::raster::{Blend, Canvas, Rgba, fade, hex};
use crate::rules::Shown;
use crate::vfx::{PlayEffect, Playing, Spawn};
use bevy::prelude::*;

const SKY: [&str; 7] = [
    "#0b0a1c", "#110f27", "#18153a", "#221c4a", "#2e2560", "#3d3174", "#4f3f86",
];
/// Behind everything (and what shows past the edge while the screen shakes).
const BACKDROP: &str = "#0b0a1c";
const MOON_FACE: &str = "#efe2bf";
const MOON_SHADE: &str = "#d3c29a";
const MOON_CRATER: &str = "#c2b08a";
const MOON_HALO: &str = "#cfc6ff";
/// The ranges, far to near: colour, how high their peaks rise (a share of the sky over the horizon), how rough.
const RANGES: [(&str, f64, f64); 3] = [
    ("#3a3170", 0.55, 1.0),
    ("#272157", 0.38, 0.8),
    ("#171434", 0.2, 0.6),
];
const MIST: &str = "#a9a4dc";
const LAMP: &str = "#ffb85a";
const STONE_LIP: &str = "#5d5670";
const STONE_TOP: &str = "#3b3549";
const STONE_SLAB: &str = "#2c2838";
const STONE_JOINT: &str = "#211e2b";
const STONE_SHADOW: &str = "#120f1a";

/// The decor's effects.
const NIGHT: &str = "weather/fireflies";
const BRAZIER: &str = "fire/brazier";

/// A puff of a mist band.
#[derive(Clone, Copy, Debug)]
struct Puff {
    x: f64,
    len: f64,
    thick: i32,
}

/// A mist band: its row, how fast it drifts, its puffs, how thick.
#[derive(Clone, Debug)]
struct Band {
    y: i32,
    speed: f64,
    puffs: Vec<Puff>,
    alpha: f64,
}

/// The backdrop as made for a layout: which layer stands in front at each pixel (0 the sky, 1 to 3 the ranges far to
/// near, 4 the floor) and the mist bands.
#[derive(Resource, Default)]
pub struct Backdrop {
    made_for: Option<(u32, i32, i32)>,
    depth: Vec<u8>,
    w: i32,
    mist: Vec<Band>,
    /// The fight whose decor is playing.
    decor_for: u32,
}

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Backdrop>()
            .add_systems(Update, build.in_set(crate::Frame::Rules).before(Cleared))
            .add_systems(Update, mist.in_set(crate::Frame::React))
            .add_systems(Update, decor.in_set(crate::Frame::Paint).after(Cleared));
    }
}

/// The page's seeded dice (a linear congruence, in doubles as JS does it).
struct Seeded(f64);

impl Seeded {
    fn next(&mut self) -> f64 {
        self.0 = (self.0 * 1103515245.0 + 12345.0) % 2147483648.0;
        self.0 / 2147483648.0
    }
}

/// A canvas `fillRect` in whole pixels (a negative size reaches the other way).
fn fill(c: &mut Canvas, x: i32, y: i32, w: i32, h: i32, color: Rgba) {
    let (x, w) = if w < 0 { (x + w, -w) } else { (x, w) };
    let (y, h) = if h < 0 { (y + h, -h) } else { (y, h) };
    c.rect(x, y, w, h, color, Blend::Over);
}

/// A disc row by row, as the page draws it.
fn disc(c: &mut Canvas, cx: i32, cy: i32, r: i32, color: Rgba) {
    for y in -r..=r {
        let half = js_round(crate::feel::math::sqrt(f64::from(r * r - y * y))) as i32;
        fill(c, cx - half, cy + y, half * 2 + 1, 1, color);
    }
}

fn moon(c: &mut Canvas, cx: i32, cy: i32, r: i32) {
    // a soft halo, then the disc with its shaded limb and a few craters
    for k in (1..=3).rev() {
        disc(
            c,
            cx,
            cy,
            r + 4 * (4 - k),
            fade(hex(MOON_HALO), 0.05 * k as f32),
        );
    }
    disc(c, cx, cy, r, hex(MOON_SHADE));
    disc(c, cx - 2, cy - 1, r - 2, hex(MOON_FACE));
    for (dx, dy, cr) in [
        (-0.3, -0.25, 0.18),
        (0.25, 0.1, 0.13),
        (-0.05, 0.4, 0.1),
        (0.35, -0.4, 0.08),
    ] {
        let rf = f64::from(r);
        disc(
            c,
            js_round(f64::from(cx) + dx * rf) as i32,
            js_round(f64::from(cy) + dy * rf) as i32,
            js_max(1.0, js_round(cr * rf)) as i32,
            hex(MOON_CRATER),
        );
    }
}

/// A three-roofed pagoda standing on (`x`, `y`) in the range's own colour, one window lit.
fn pagoda(c: &mut Canvas, x: i32, y: i32, color: Rgba) {
    let tier = |c: &mut Canvas, base: i32, wide: i32, tall: i32| {
        fill(c, x - wide, base - 1, wide * 2 + 1, 1, color);
        fill(c, x - wide - 1, base - 2, 2, 1, color);
        fill(c, x + wide, base - 2, 2, 1, color);
        fill(
            c,
            x - wide + 2,
            base - tall,
            (wide - 2) * 2 + 1,
            tall - 1,
            color,
        );
    };
    tier(c, y + 2, 7, 6);
    tier(c, y - 4, 6, 5);
    tier(c, y - 9, 4, 5);
    fill(c, x, y - 17, 1, 4, color);
    fill(c, x, y - 2, 1, 2, hex(LAMP));
    fill(c, x - 1, y - 3, 3, 4, fade(hex(LAMP), 0.25));
}

/// A pine `tall` high standing on (`x`, `y`): widest at the foot, tiers stepping in toward the top.
fn pine(c: &mut Canvas, x: i32, y: i32, tall: i32, color: Rgba) {
    fill(c, x, y - tall, 1, tall, color);
    for i in 0..tall - 1 {
        let half = (tall - i) * 3 / tall + i % 2;
        fill(c, x - half, y - i - 1, half * 2 + 1, 1, color);
    }
}

/// The still backdrop for a `w` x `h` world whose floor is at `ground`, the moon below the band `clear` the bars sit
/// in; and its depths and mist.
fn paint_scene(w: i32, h: i32, ground: i32, clear: i32) -> (Canvas, Vec<u8>, Vec<Band>) {
    let mut rand = Seeded(11.0);
    let horizon = ground - 4;
    let hz = f64::from(horizon);
    let layer = || Canvas::new(w, h);

    let mut sky = layer();
    for y in 0..horizon {
        // banded, with a dithered edge between bands
        let f = f64::from(y) / hz * (SKY.len() - 1) as f64;
        let i = floor(f) as usize;
        let dither = usize::from(f - i as f64 > f64::from((y * 7 + 3) % 4) / 4.0);
        let mut x = 0;
        while x < w {
            let d = if ((x >> 1) + y) % 2 == 0 { dither } else { 0 };
            fill(&mut sky, x, y, 2, 1, hex(SKY[(i + d).min(SKY.len() - 1)]));
            x += 2;
        }
    }
    let stars = floor(f64::from(w) * hz / 230.0) as i32;
    for _ in 0..stars {
        let x = floor(rand.next() * f64::from(w)) as i32;
        let y = floor(rand.next() * hz * 0.75) as i32;
        let a = 0.25 + rand.next() * 0.6 * (1.0 - f64::from(y) / hz);
        let color = if rand.next() < 0.15 {
            "#ffe9c0"
        } else {
            "#e6e2ff"
        };
        fill(&mut sky, x, y, 1, 1, fade(hex(color), a as f32));
    }
    let r = js_round(js_max(14.0, js_min(24.0, f64::from(h) * 0.1))) as i32;
    let cy = (js_round(js_min(46.0, hz * 0.3)) as i32).max(clear + r + 4);
    moon(&mut sky, js_round(f64::from(w) * 0.74) as i32, cy, r);

    let mut layers = vec![sky];
    for (k, &(color, top, rough)) in RANGES.iter().enumerate() {
        let mut l = layer();
        let color = hex(color);
        let peak = hz * (1.0 - top);
        let phase = [rand.next() * 6.0, rand.next() * 6.0, rand.next() * 6.0];
        // the ridge sits between its peaks and the horizon
        let ridge = |x: i32| {
            let x = f64::from(x);
            js_round(
                peak + (hz - peak) * 0.55 * (1.0 - rough * sin(x * 0.011 + phase[0]).abs())
                    + 8.0 * sin(x * 0.043 + phase[1]) * rough
                    + 3.0 * sin(x * 0.17 + phase[2]),
            ) as i32
        };
        for x in 0..w {
            fill(&mut l, x, ridge(x), 1, ground - ridge(x), color);
        }
        // a lit pagoda on the far range's highest point
        if k == 0 {
            let mut best = 0;
            let mut x = floor(f64::from(w) * 0.1) as i32;
            while f64::from(x) < f64::from(w) * 0.6 {
                if ridge(x) < ridge(best) || best == 0 {
                    best = x;
                }
                x += 1;
            }
            pagoda(&mut l, best, ridge(best), color);
        }
        // pines along the nearest ridge
        if k == RANGES.len() - 1 {
            let mut x = 2;
            while x < w {
                let tall = 5 + floor(rand.next() * 8.0) as i32;
                pine(&mut l, x, ridge(x) + 1, tall, color);
                x += 3 + floor(rand.next() * 9.0) as i32;
            }
        }
        layers.push(l);
    }

    let mut floor_l = layer();
    let f = &mut floor_l;
    fill(f, 0, ground, w, h - ground, hex(STONE_SLAB));
    fill(f, 0, ground, w, 1, hex(STONE_LIP));
    fill(f, 0, ground + 1, w, 2, hex(STONE_TOP));
    // joints between the slabs, staggered from course to course
    let (mut y, mut row) = (ground + 3, 0);
    while y < h {
        fill(f, 0, y, w, 1, hex(STONE_JOINT));
        let mut x = (row % 2) * 9;
        while x < w {
            fill(f, x, y, 1, 6, hex(STONE_JOINT));
            x += 18;
        }
        y += 6;
        row += 1;
    }
    fill(f, 0, h - 3, w, 3, fade(hex(STONE_SHADOW), 0.5));
    layers.push(floor_l);

    // mist bands: one between each pair of ranges and one along the foot of the near ridge
    let puffs = ((w + 39) / 40) as usize + 2;
    let mist = [0.7, 0.84, 0.96]
        .iter()
        .enumerate()
        .map(|(i, at)| Band {
            y: js_round(hz * at) as i32,
            speed: 1.5 + i as f64 * 1.2,
            puffs: (0..puffs)
                .map(|_| Puff {
                    x: rand.next() * f64::from(w + 80),
                    len: 30.0 + rand.next() * 60.0,
                    thick: 2 + floor(rand.next() * 3.0) as i32,
                })
                .collect(),
            alpha: 0.07 + i as f64 * 0.03,
        })
        .collect();

    let mut scene = layer();
    let (sw, sh) = (scene.w, scene.h);
    scene.rect(0, 0, sw, sh, hex(BACKDROP), Blend::Set);
    let mut depth = vec![0u8; (w * h) as usize];
    for (d, l) in layers.iter().enumerate() {
        for (i, p) in l.px.iter().enumerate() {
            if p[3] > 0 {
                scene.put(i as i32 % w, i as i32 / w, *p, Blend::Over);
                if d > 0 {
                    depth[i] = d as u8;
                }
            }
        }
    }
    (scene, depth, mist)
}

/// The backdrop painted again when the layout changes (before the frame's pictures start from it).
fn build(layout: Res<Layout>, mut backdrop: ResMut<Backdrop>, mut canvases: ResMut<Canvases>) {
    let (w, h) = (layout.w as i32, layout.h as i32);
    let key = Some((layout.version, w, h));
    if backdrop.made_for == key && canvases.scene.w == w && canvases.scene.h == h {
        return;
    }
    let clear = (layout.bar_top + BARS_TALL) as i32;
    let (scene, depth, mist) = paint_scene(w, h, layout.ground as i32, clear);
    canvases.scene = scene;
    backdrop.made_for = key;
    backdrop.depth = depth;
    backdrop.w = w;
    backdrop.mist = mist;
}

/// The mist drifting at the fight's time, each band behind the ranges nearer than it (on `back`, before anything else
/// is painted on it).
fn mist(backdrop: Res<Backdrop>, shown: Res<Shown>, mut canvases: ResMut<Canvases>) {
    let t = shown.view.as_ref().map_or(0.0, |v| v.time);
    let back = &mut canvases.back;
    if backdrop.depth.len() != back.px.len() {
        return;
    }
    let span = f64::from(backdrop.w + 80);
    let color = hex(MIST);
    for (i, band) in backdrop.mist.iter().enumerate() {
        let c = fade(color, band.alpha as f32);
        // band i lies over range i and under range i + 1
        let shows = (i + 1) as u8;
        let mut puff = |x: i32, y: i32, len: i32, thick: i32| {
            for yy in y..y + thick {
                for xx in x..x + len {
                    if xx < 0 || yy < 0 || xx >= back.w || yy >= back.h {
                        continue;
                    }
                    if backdrop.depth[(yy * back.w + xx) as usize] <= shows {
                        back.put(xx, yy, c, Blend::Over);
                    }
                }
            }
        };
        for p in &band.puffs {
            let x = js_round((((p.x + t * band.speed) % span) + span) % span - 40.0) as i32;
            puff(x, band.y, js_round(p.len) as i32, p.thick);
            puff(x + 6, band.y - 1, js_round(p.len * 0.6) as i32, 1);
        }
    }
}

/// A new fight lights its braziers and lets its fireflies out.
fn decor(
    layout: Res<Layout>,
    shown: Res<Shown>,
    mut backdrop: ResMut<Backdrop>,
    mut playing: ResMut<Playing>,
    mut lamps: MessageWriter<PlayEffect>,
) {
    if shown.view.is_none() || backdrop.decor_for == shown.fight {
        return;
    }
    backdrop.decor_for = shown.fight;
    let have = pixfx_duel::effects::all();
    let has = |id: &str| have.iter().any(|e| e == id);
    if has(NIGHT) {
        // one fireflies canvas the scene's size: the meadow on the dais (its ground mid-width, 10 pixels above its
        // canvas's foot)
        let night = Spawn {
            x: floor(f64::from(layout.w) / 2.0) as f32,
            y: layout.ground,
            ground: layout.ground,
            width: Some(layout.w),
            height: Some(layout.ground + 10.0),
            seed: 1,
            ..default()
        };
        playing.play(None, NIGHT, &night);
    }
    if has(BRAZIER) {
        for x in layout.lamps {
            lamps.write(PlayEffect {
                fx: BRAZIER,
                x,
                y: layout.ground,
                opts: default(),
            });
        }
    }
}
