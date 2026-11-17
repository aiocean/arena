//! Review shots of the hit geometry, offline: real fights between two house brains, stopped on the moments that show
//! where a fighter can be hit and where its blow lands, each drawn zoomed round the fighter with its frame as the game
//! draws it and the boxes the rules test, labelled. The shipped page draws none of this.
//!
//! ```text
//! cargo run -p arena-game --example hitbox_review -- <out dir>
//! ```
//!
//! For each fighter (Thunder, Frost) and moment (standing, ducking, meditating, in the air, a blade blow on its
//! active frame, a bolt leaving the hand), it writes `<look>-<moment>.png`: the fighters' frames (`pick`, placed as
//! `fighters::placed` places them), each **hurtbox** (`frames::hurtboxes`) filled and outlined in the fighter's colour,
//! the **hitbox** of a blade blow (`unit::strike_box`) in red, and the **launch** point of a bolt
//! (`frames::cast_hand`), and the hand's **wrist** and **hilt** where the frame has them.

#![allow(clippy::disallowed_methods)]

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use arena_duel::data::{Kind, Look};
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::fighter::{Phase, State};
use arena_duel::frames::{cast_hand, hurtboxes};
use arena_duel::pick::{appearance, pick};
use arena_duel::pose::Pose;
use arena_duel::seat::Brain;
use arena_duel::shot::ShotKind;
use arena_game::sheet::{Layout, parse};
use image::{Rgba, RgbaImage};

/// How many image pixels a rules pixel is.
const ZOOM: f64 = 2.0;
/// The crop round a fighter, in rules pixels: this far to each side of its middle, this high above the ground.
const HALF_W: f64 = 220.0;
const HIGH: f64 = 520.0;
const BELOW: f64 = 24.0;

const SHEETS: [(&str, &[&str]); 2] = [
    (
        "assets/fighters/cultivator.json",
        &[
            "assets/fighters/cultivator-thunder.png",
            "assets/fighters/cultivator-thunder-1.png",
        ],
    ),
    (
        "assets/fighters/seven-colored-daoist.json",
        &[
            "assets/fighters/seven-colored-daoist-frost.png",
            "assets/fighters/seven-colored-daoist-frost-1.png",
        ],
    ),
];

struct Sheet {
    layout: Layout,
    pages: Vec<RgbaImage>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Moment {
    Stand,
    Duck,
    Meditate,
    Air,
    Slash,
    Bolt,
}

impl Moment {
    const ALL: [Moment; 6] = [
        Moment::Stand,
        Moment::Duck,
        Moment::Meditate,
        Moment::Air,
        Moment::Slash,
        Moment::Bolt,
    ];
    fn name(self) -> &'static str {
        match self {
            Moment::Stand => "stand",
            Moment::Duck => "duck",
            Moment::Meditate => "meditate",
            Moment::Air => "air",
            Moment::Slash => "slash",
            Moment::Bolt => "bolt",
        }
    }
}

/// Is fighter `f` at moment `m` now?
fn at(state: &State, f: usize, m: Moment) -> bool {
    let fi = &state.fighters[f];
    let b = &fi.body;
    match m {
        Moment::Stand => {
            state.time > 1.0 && b.pose == Pose::Idle && b.air == 0.0 && fi.act.is_none()
        }
        Moment::Duck => b.pose == Pose::Duck && b.pose_t >= 0.3,
        Moment::Meditate => b.pose == Pose::Meditate && b.pose_t >= 0.6,
        Moment::Air => b.air > 90.0 && fi.act.is_none(),
        Moment::Slash => fi.act.is_some_and(|a| {
            a.phase == Phase::Recover
                && a.t == 0.0
                && arena_duel::data::spell(a.spell)
                    .is_some_and(|s| s.kind == Kind::Melee && !s.dive)
        }),
        Moment::Bolt => state
            .shots
            .iter()
            .any(|s| s.owner == f && matches!(s.kind, ShotKind::Bolt { .. }) && s.t <= TICK + 1e-9),
    }
}

/// Blend `c` over the pixel at (`x`, `y`).
fn blend(img: &mut RgbaImage, x: i64, y: i64, c: [u8; 4]) {
    if x < 0 || y < 0 || x >= img.width() as i64 || y >= img.height() as i64 {
        return;
    }
    let p = img.get_pixel_mut(x as u32, y as u32);
    let a = c[3] as f32 / 255.0;
    for i in 0..3 {
        p[i] = (p[i] as f32 * (1.0 - a) + c[i] as f32 * a).round() as u8;
    }
    p[3] = 255;
}

struct View {
    /// The crop's left edge and its ground row, in rules pixels.
    left: f64,
    ground_px: f64,
}

impl View {
    fn px(&self, x: f64, y: f64) -> (i64, i64) {
        (
            ((x - self.left) * ZOOM).round() as i64,
            (self.ground_px - y * ZOOM).round() as i64,
        )
    }
}

fn rect(img: &mut RgbaImage, v: &View, r: [f64; 4], fill: [u8; 4], line: [u8; 4], width: i64) {
    let (x0, y1) = v.px(r[0], r[1]);
    let (x1, y0) = v.px(r[2], r[3]);
    for y in y0..y1 {
        for x in x0..x1 {
            let edge = x - x0 < width || x1 - 1 - x < width || y - y0 < width || y1 - 1 - y < width;
            blend(img, x, y, if edge { line } else { fill });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn text(
    img: &mut RgbaImage,
    font: &FontRef,
    x: i64,
    y: i64,
    size: f32,
    s: &str,
    color: [u8; 4],
    back: bool,
) {
    let scale = PxScale::from(size);
    let sf = font.as_scaled(scale);
    let width: f32 = s.chars().map(|c| sf.h_advance(font.glyph_id(c))).sum();
    if back {
        for yy in y - 2..y + size as i64 + 2 {
            for xx in x - 3..x + width as i64 + 3 {
                blend(img, xx, yy, [10, 8, 20, 200]);
            }
        }
    }
    let mut caret = x as f32;
    for c in s.chars() {
        let g = font
            .glyph_id(c)
            .with_scale_and_position(scale, ab_glyph::point(caret, y as f32 + sf.ascent()));
        caret += sf.h_advance(font.glyph_id(c));
        if let Some(o) = font.outline_glyph(g) {
            let bb = o.px_bounds();
            o.draw(|gx, gy, cov| {
                let a = (cov * color[3] as f32) as u8;
                blend(
                    img,
                    bb.min.x as i64 + gx as i64,
                    bb.min.y as i64 + gy as i64,
                    [color[0], color[1], color[2], a],
                );
            });
        }
    }
}

/// Draw fighter `f`'s frame as the game does.
fn draw_fighter(img: &mut RgbaImage, v: &View, state: &State, f: usize, sheet: &Sheet) {
    let fi = &state.fighters[f];
    let b = &fi.body;
    let a = appearance(
        b.pose,
        b.pose_t,
        !fi.alive(),
        fi.act
            .map(|a| (a.spell, a.phase, a.t, arena_duel::act::phase_span(&a))),
    );
    let look = fi.look;
    let shot = pick(
        &a,
        sheet.layout.fps,
        |c| sheet.layout.clips.get(c).map_or(0, Vec::len),
        |c| arena_duel::frames::active(look, c),
    );
    let Some(cell) = sheet
        .layout
        .clips
        .get(shot.clip)
        .and_then(|c| c.get(shot.frame))
    else {
        return;
    };
    let Some(page) = sheet.pages.get(cell.page as usize) else {
        return;
    };
    let face_left = b.face < 0.0;
    // as fighters::placed: the box's left from the feet, mirrored round them facing left
    let left = if face_left {
        b.x - cell.dx as f64 - cell.w as f64
    } else {
        b.x + cell.dx as f64
    };
    let top_up = b.air - cell.dy as f64;
    for py in 0..cell.h {
        for qx in 0..cell.w {
            let src = page.get_pixel(cell.x + qx, cell.y + py);
            if src[3] == 0 {
                continue;
            }
            let sx = if face_left { cell.w - 1 - qx } else { qx } as f64;
            let (x0, y0) = v.px(left + sx, top_up - py as f64);
            let (x1, y1) = v.px(left + sx + 1.0, top_up - py as f64 - 1.0);
            for y in y0..y1 {
                for x in x0..x1 {
                    blend(img, x, y, [src[0], src[1], src[2], src[3]]);
                }
            }
        }
    }
}

fn shoot(
    state: &State,
    f: usize,
    m: Moment,
    sheets: &[Sheet],
    font: &FontRef,
    out: &str,
) -> Result<(), String> {
    let fi = &state.fighters[f];
    let b = &fi.body;
    let (w, h) = ((2.0 * HALF_W * ZOOM) as u32, ((HIGH + BELOW) * ZOOM) as u32);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([52, 48, 72, 255]));
    let v = View {
        left: b.x - HALF_W,
        ground_px: HIGH * ZOOM,
    };
    // the ground
    for x in 0..w as i64 {
        for y in 0..3 {
            blend(&mut img, x, (HIGH * ZOOM) as i64 + y, [200, 200, 220, 255]);
        }
    }
    for g in 0..2 {
        let sheet = &sheets[match state.fighters[g].look {
            Look::Thunder => 0,
            Look::Frost => 1,
        }];
        draw_fighter(&mut img, &v, state, g, sheet);
    }
    let colour = |g: usize| {
        if state.fighters[g].look == Look::Thunder {
            [255, 200, 40]
        } else {
            [60, 220, 255]
        }
    };
    for g in 0..2 {
        let [r, gg, bb] = colour(g);
        for hb in hurtboxes(state, g) {
            rect(&mut img, &v, hb, [r, gg, bb, 70], [r, gg, bb, 255], 3);
            let (x, y) = v.px(hb[0], hb[3]);
            text(
                &mut img,
                font,
                x + 5,
                y + 4,
                15.0,
                "HURTBOX",
                [r, gg, bb, 255],
                true,
            );
        }
    }
    let mut note = String::new();
    if m == Moment::Slash
        && let Some(act) = fi.act
        && let Some(k) = fi.kit.index(act.spell)
    {
        let sp = fi.kit.skills[k].motion;
        let face = if b.face < 0.0 { -1.0 } else { 1.0 };
        let hb = arena_duel::unit::strike_box(
            state,
            f,
            b.x,
            b.air,
            face,
            sp.range.unwrap_or(0.0),
            b.spec.chest + 16.0,
            true,
        );
        rect(&mut img, &v, hb, [255, 40, 40, 80], [255, 40, 40, 255], 3);
        let (x, y) = v.px(hb[0], hb[1]);
        text(
            &mut img,
            font,
            x + 5,
            y - 24,
            15.0,
            "HITBOX",
            [255, 90, 90, 255],
            true,
        );
        note = format!("{} (reach {:.0})", act.spell, sp.range.unwrap_or(0.0));
    }
    // the hand held out ahead: its wrist (where the arm's hurtbox ends) and its hilt (where a blade's hitbox starts)
    if let Some(fr) = arena_duel::frames::frame_of(state, f) {
        let face = if b.face < 0.0 { -1.0 } else { 1.0 };
        for (p, colour, label) in [
            (fr.wrist, [80, 255, 120, 255], "wrist"),
            (fr.hilt, [80, 200, 255, 255], "hilt"),
        ] {
            let Some(p) = p else { continue };
            let (x, y) = v.px(b.x + face * p[0], b.air + p[1]);
            for dy in -5..=5i64 {
                for dx in -5..=5i64 {
                    if dx * dx + dy * dy <= 25 {
                        blend(
                            &mut img,
                            x + dx,
                            y + dy,
                            if dx * dx + dy * dy <= 9 {
                                colour
                            } else {
                                [20, 20, 20, 255]
                            },
                        );
                    }
                }
            }
            text(
                &mut img,
                font,
                x + 8,
                y + if label == "wrist" { 6 } else { -24 },
                13.0,
                label,
                colour,
                true,
            );
        }
    }
    if m == Moment::Bolt {
        let launch = cast_hand(state, f);
        if let Some((lx, ly)) = launch {
            let (x, y) = v.px(lx, ly);
            for dy in -7..=7i64 {
                for dx in -7..=7i64 {
                    if dx * dx + dy * dy <= 49 {
                        blend(
                            &mut img,
                            x + dx,
                            y + dy,
                            if dx * dx + dy * dy <= 16 {
                                [255, 255, 80, 255]
                            } else {
                                [20, 20, 20, 255]
                            },
                        );
                    }
                }
            }
            text(
                &mut img,
                font,
                x + 10,
                y - 26,
                15.0,
                "launch",
                [255, 255, 120, 255],
                true,
            );
        }
        for s in state.shots.iter().filter(|s| s.owner == f) {
            if let Some((bx, by, _)) = s.bolt_at(state) {
                let (x, y) = v.px(bx, by);
                for d in -5..=5i64 {
                    blend(&mut img, x + d, y, [255, 80, 220, 255]);
                    blend(&mut img, x, y + d, [255, 80, 220, 255]);
                }
                note = format!(
                    "{} leaves {}",
                    s.name,
                    if launch.is_some() {
                        "the hand"
                    } else {
                        "the shoulders (no hand drawn ahead)"
                    }
                );
            }
        }
    }
    let who = if fi.look == Look::Thunder {
        "Thunder"
    } else {
        "Frost"
    };
    let a = appearance(
        b.pose,
        b.pose_t,
        !fi.alive(),
        fi.act
            .map(|a| (a.spell, a.phase, a.t, arena_duel::act::phase_span(&a))),
    );
    let sheet = &sheets[if fi.look == Look::Thunder { 0 } else { 1 }];
    let look = fi.look;
    let shot = pick(
        &a,
        sheet.layout.fps,
        |c| sheet.layout.clips.get(c).map_or(0, Vec::len),
        |c| arena_duel::frames::active(look, c),
    );
    let title = format!(
        "{who} · {} · clip {} frame {} {}",
        m.name(),
        shot.clip,
        shot.frame,
        note
    );
    text(
        &mut img,
        font,
        10,
        8,
        20.0,
        &title,
        [255, 255, 255, 255],
        true,
    );
    let path = format!("{out}/{}-{}.png", who.to_lowercase(), m.name());
    img.save(&path).map_err(|e| format!("{path}: {e}"))?;
    println!("{path}");
    Ok(())
}

fn main() -> Result<(), String> {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/hitbox-review".into());
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let root = env!("CARGO_MANIFEST_DIR");
    let font_bytes = std::fs::read(format!("{root}/assets/fonts/BeVietnamPro-Bold-vi.ttf"))
        .map_err(|e| e.to_string())?;
    let font = FontRef::try_from_slice(&font_bytes).map_err(|e| e.to_string())?;
    let mut sheets = Vec::new();
    for (json, pages) in SHEETS {
        let layout =
            parse(&std::fs::read_to_string(format!("{root}/{json}")).map_err(|e| e.to_string())?)
                .map_err(|e| format!("{json}: {e:?}"))?;
        let pages = pages
            .iter()
            .map(|p| {
                image::open(format!("{root}/{p}"))
                    .map(|i| i.to_rgba8())
                    .map_err(|e| format!("{p}: {e}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        sheets.push(Sheet { layout, pages });
    }
    let mut todo: Vec<(usize, Moment)> = (0..2).flat_map(|f| Moment::ALL.map(|m| (f, m))).collect();
    for seed in 1..=60 {
        if todo.is_empty() {
            break;
        }
        let config = DuelConfig {
            seed,
            left: 88.0,
            right: 1192.0,
            ground: 632.0,
            sky: 196.0,
            effects: arena_duel::effects::all(),
        };
        let mut duel = Duel::new(config, [Brain::make(), Brain::make()]);
        while duel.state().time < 120.0 && !todo.is_empty() {
            duel.step(TICK);
            let state = duel.state();
            let mut done = Vec::new();
            for (i, &(f, m)) in todo.iter().enumerate() {
                if at(state, f, m) {
                    shoot(state, f, m, &sheets, &font, &out)?;
                    done.push(i);
                }
            }
            for i in done.into_iter().rev() {
                todo.remove(i);
            }
        }
    }
    for (f, m) in todo {
        println!("not caught: fighter {f} {}", m.name());
    }
    Ok(())
}
