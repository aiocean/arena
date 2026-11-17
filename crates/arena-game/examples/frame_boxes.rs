//! Frame data from the fighters' sprites: for every look, every clip the rules pick from (`arena_duel::pick::CLIPS`)
//! and every frame of it, the boxes the rules test blows against, written as a generated table into arena-duel
//! (`crates/arena-duel/src/frames_data.rs`). The rules never read a picture: this runs offline, after a sheet is
//! (re)made, and the table it writes is part of the rules.
//!
//! ```text
//! cargo run -p arena-game --example frame_boxes -- crates/arena-duel/src/frames_data.rs
//! ```
//!
//! From a frame's opaque pixels, measured from the feet (a sheet's pixel is the rules' pixel), facing right:
//! - **hurtboxes**: the head and torso, and the legs, each the middle run of pixels round the body's line, its edges
//!   taken over the band's rows at the 15th and 85th percentile (a sleeve or a blade on a few rows does not widen
//!   it), the top short of the hair; and, on a clip that strikes or casts, the arm held out ahead of the torso (no
//!   farther than `ARM` from it).
//! - **hitbox**, on the active frame of a blade clip: what the body holds out ahead of the torso from the waist up,
//!   the hand and the blade.
//! - **hand**, on the active frame of a casting clip: the farthest point the body holds out in the torso's band,
//!   where a bolt leaves.
//! - **active window**: from the frame that reaches farthest ahead (at least `REACH_MIN` beyond the clip's least
//!   reach; none when no frame does, as on a clip drawn striking behind) on while a blade stays out ahead. `pick` ends
//!   the wind-up on its first frame: the rules strike or cast on the frame the picture strikes on, and a blade that
//!   missed tries again on each frame of the window, one hit a swing.

// offline tooling over pictures: it writes the table the rules use, its own rounding is not the rules'
#![allow(clippy::disallowed_methods)]

use arena_duel::pick::CLIPS;
use arena_game::sheet::{Cell, parse};
use image::RgbaImage;
use std::fmt::Write as _;

/// A pixel counts when it is this opaque.
const SOLID: u8 = 64;
/// The waist: this share of the figure's height from its lowest pixel up splits legs from torso.
const WAIST: f64 = 0.45;
/// The share of a band's rows a box's edges leave out on each side: a hurtbox sits inside the drawn body, so a robe's
/// flare, a sleeve or a blade on a few rows does not widen it (cloth is not body).
const LOW: f64 = 0.3;
/// The least thickness (pixels, up and down) a column held out ahead must have to be an arm or a hand: a blade is
/// thinner, and a blade is never hurt (it is the hitbox, disjointed from the body, as fighting games have it).
const LIMB: usize = 7;
/// The hair or crown's share of the height left off the top of the head's box.
const HAIR: f64 = 0.04;
/// How far an arm held out reaches from the torso, at most (a blade beyond it is not the body).
const ARM: f64 = 72.0;
/// Clips that strike or cast: an arm held out is part of the body there.
const REACHING: &[&str] = &["slash", "overhead", "cast", "dash"];
/// Blade clips: their active frame has a hitbox.
const BLADES: &[&str] = &["slash", "overhead"];
/// Casting clips: their active frame has a hand a bolt leaves from.
const CASTS: &[&str] = &["cast", "dash"];

/// Each look's sheet, in `arena_duel::data::Look` order.
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

/// A frame's opaque pixels, measured from the feet: x right, y up, each pixel the unit square at its corner.
struct Figure {
    /// (x, y) of every opaque pixel's lower left corner.
    px: Vec<(i32, i32)>,
}

fn figure(page: &RgbaImage, c: &Cell) -> Figure {
    let mut px = Vec::new();
    for py in 0..c.h {
        for qx in 0..c.w {
            if page.get_pixel(c.x + qx, c.y + py)[3] >= SOLID {
                let x = qx as i32 + c.dx;
                // row py's top is (py + dy) below the feet; its lower edge one pixel lower
                let y = -(py as i32 + c.dy) - 1;
                px.push((x, y));
            }
        }
    }
    Figure { px }
}

/// The pixels of a row in runs, a gap of up to `GAP` pixels (an outline's seam) kept inside one run.
fn runs(row: &[i32]) -> Vec<(i32, i32)> {
    let mut runs: Vec<(i32, i32)> = Vec::new();
    for &x in row {
        match runs.last_mut() {
            Some(r) if x - r.1 <= GAP + 1 => r.1 = x,
            _ => runs.push((x, x)),
        }
    }
    runs
}

/// The widest seam inside a run.
const GAP: i32 = 2;

/// The run of pixels round the body's line on a row: the one holding x = 0, else the nearest.
fn core_run(row: &[i32]) -> Option<(i32, i32)> {
    runs(row).into_iter().min_by_key(|(l, r)| {
        if *l <= 0 && 0 <= *r {
            0
        } else {
            l.abs().min(r.abs())
        }
    })
}

/// The pixels of the figure itself: those joined to the body's line (x = 0) through opaque pixels, a seam of up to
/// `GAP` kept joined. A blade in the hand is joined to the body through the hand; sparks floating free are not.
fn body_part(px: &[(i32, i32)]) -> Vec<(i32, i32)> {
    use std::collections::{HashSet, VecDeque};
    let set: HashSet<(i32, i32)> = px.iter().copied().collect();
    let mut seen: HashSet<(i32, i32)> = HashSet::new();
    let mut queue: VecDeque<(i32, i32)> = VecDeque::new();
    // start from every pixel on the body's line
    for p in px.iter().filter(|p| p.0.abs() <= 2) {
        if seen.insert(*p) {
            queue.push_back(*p);
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for dy in -GAP..=GAP {
            for dx in -GAP..=GAP {
                let q = (x + dx, y + dy);
                if set.contains(&q) && seen.insert(q) {
                    queue.push_back(q);
                }
            }
        }
    }
    let mut out: Vec<_> = seen.into_iter().collect();
    out.sort_unstable();
    out
}

fn percentile(v: &mut [i32], q: f64) -> i32 {
    v.sort_unstable();
    let i = ((v.len() as f64 - 1.0) * q).round() as usize;
    v.get(i).copied().unwrap_or(0)
}

/// A frame's boxes: [x0, y0, x1, y1] from the feet, facing right. `hit` and `hand` are each frame's candidates; only
/// the clip's active frame keeps them. `reach` is how far ahead of the body the frame strikes or casts.
#[derive(Default)]
struct Boxes {
    hurt: Vec<[i32; 4]>,
    hit: Option<[i32; 4]>,
    hand: Option<[i32; 2]>,
    reach: i32,
}

/// The least reach ahead of the torso a frame needs to be a clip's active frame.
const REACH_MIN: i32 = 24;

fn boxes(f: &Figure, clip: &str) -> Boxes {
    let mut out = Boxes::default();
    if f.px.is_empty() {
        return out;
    }
    let ymin = f.px.iter().map(|p| p.1).min().unwrap_or(0);
    let ymax = f.px.iter().map(|p| p.1).max().unwrap_or(0) + 1;
    let height = (ymax - ymin) as f64;
    let waist = ymin + (height * WAIST).round() as i32;
    let top = ymax - (height * HAIR).round() as i32;

    // rows, each its pixels' x in order
    let mut rows: std::collections::BTreeMap<i32, Vec<i32>> = Default::default();
    for &(x, y) in &f.px {
        rows.entry(y).or_default().push(x);
    }
    for r in rows.values_mut() {
        r.sort_unstable();
    }
    let band = |lo: i32, hi: i32| -> Option<[i32; 4]> {
        let (mut ls, mut rs) = (Vec::new(), Vec::new());
        for (_, row) in rows.range(lo..hi) {
            if let Some((l, r)) = core_run(row) {
                ls.push(l);
                rs.push(r + 1);
            }
        }
        if ls.len() < 4 {
            return None;
        }
        let (x0, x1) = (percentile(&mut ls, LOW), percentile(&mut rs, 1.0 - LOW));
        (x1 > x0).then_some([x0, lo, x1, hi])
    };
    let legs = band(ymin, waist);
    let torso = band(waist, top);
    out.hurt.extend(legs);
    out.hurt.extend(torso);

    // what the body holds out ahead of the torso: an arm, a hand, a blade (all joined to the body)
    let edge = torso.map_or(0, |t| t[2]);
    let body = body_part(&f.px);
    let ahead = |lo: i32, hi: i32| -> Vec<(i32, i32)> {
        body.iter()
            .copied()
            .filter(|&(x, y)| y >= lo && y < hi && x > edge + 2)
            .collect()
    };
    if REACHING.contains(&clip) {
        // the arm and hand: columns ahead of the torso as thick as a limb, from the torso's edge out to the first
        // thin one (where a blade starts)
        let band = ahead(waist, top);
        // `ahead` starts two pixels out from the edge
        let mut x1 = edge + 2;
        loop {
            let col: Vec<_> = band.iter().filter(|p| p.0 == x1 + 1).collect();
            if col.len() < LIMB || x1 - edge >= ARM as i32 {
                break;
            }
            x1 += 1;
        }
        let arm: Vec<_> = band.iter().filter(|p| p.0 <= x1).collect();
        if x1 - edge >= 6 && !arm.is_empty() {
            let y0 = arm.iter().map(|a| a.1).min().unwrap_or(waist);
            let y1 = arm.iter().map(|a| a.1).max().unwrap_or(waist) + 1;
            out.hurt.push([edge, y0, x1 + 1, y1]);
        }
    }
    let farthest = |lo: i32| {
        body.iter()
            .filter(|p| p.1 >= lo && p.1 < top)
            .map(|p| p.0 + 1)
            .max()
            .unwrap_or(0)
    };
    if BLADES.contains(&clip) {
        // a cut at the body's height: what is held out from the waist up (a blade hanging down at rest is not)
        let cut = ahead(waist - 10, ymax);
        if cut.len() > 20 {
            let y0 = cut.iter().map(|a| a.1).min().unwrap_or(waist);
            let y1 = cut.iter().map(|a| a.1).max().unwrap_or(waist) + 1;
            let x1 = cut.iter().map(|a| a.0).max().unwrap_or(edge) + 1;
            out.hit = Some([edge, y0, x1, y1]);
        }
        out.reach = farthest(waist - 10);
    }
    if CASTS.contains(&clip) {
        let hand = body
            .iter()
            .filter(|p| p.1 >= waist && p.1 < top)
            .max_by_key(|p| (p.0, p.1));
        out.hand = hand.map(|p| [p.0 + 1, p.1]);
        out.reach = farthest(waist);
    }
    out
}

fn num(v: i32) -> String {
    format!("{v}.0")
}

fn rect(r: &[i32; 4]) -> String {
    format!(
        "[{}, {}, {}, {}]",
        num(r[0]),
        num(r[1]),
        num(r[2]),
        num(r[3])
    )
}

fn main() -> Result<(), String> {
    let out_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "crates/arena-duel/src/frames_data.rs".into());
    let root = env!("CARGO_MANIFEST_DIR");
    let mut src = String::new();
    let _ = writeln!(
        src,
        "// @generated by `cargo run -p arena-game --example frame_boxes`: do not edit; run it again after a sheet changes.\n\
         // Each look's frames, in `Look` order: per clip of `pick::CLIPS`, per frame, the hurtboxes, the hitbox (a blade's\n\
         // active frame) and the hand (a cast's active frame), in the rules' pixels from the feet, facing right.\n\n\
         use crate::frames::{{Frame, LookFrames}};\n\n\
         pub static LOOKS: [LookFrames; 2] = ["
    );
    for (json_path, pages) in SHEETS {
        let json = std::fs::read_to_string(format!("{root}/{json_path}"))
            .map_err(|e| format!("{json_path}: {e}"))?;
        let layout = parse(&json).map_err(|e| format!("{json_path}: {e:?}"))?;
        let images: Vec<RgbaImage> = pages
            .iter()
            .map(|p| {
                image::open(format!("{root}/{p}"))
                    .map(|i| i.to_rgba8())
                    .map_err(|e| format!("{p}: {e}"))
            })
            .collect::<Result<_, _>>()?;
        let _ = writeln!(
            src,
            "    LookFrames {{\n        fps: {:?},\n        clips: &[",
            layout.fps
        );
        for clip in CLIPS {
            let cells = layout
                .clips
                .get(*clip)
                .ok_or(format!("{json_path}: no clip {clip}"))?;
            let mut all = Vec::with_capacity(cells.len());
            for c in cells {
                let page = images
                    .get(c.page as usize)
                    .ok_or(format!("{json_path}: no page {}", c.page))?;
                all.push(boxes(&figure(page, c), clip));
            }
            // the active frame: where the blade or the casting hand reaches farthest ahead (the rules strike there:
            // `pick` splits the clip's wind-up from its follow-through at it)
            // and it must reach out beyond where the clip starts (its guard), or the clip has no active frame
            let rest = all.iter().map(|b| b.reach).min().unwrap_or(0);
            let active = all
                .iter()
                .enumerate()
                .filter(|(_, b)| b.reach - rest >= REACH_MIN)
                .max_by_key(|(i, b)| (b.reach, std::cmp::Reverse(*i)))
                .map(|(i, _)| i);
            // a blade's active window: from that frame on while its blade stays out ahead (a cut swinging round
            // behind the body can no longer strike the foe ahead); a cast's is its one frame
            let window = active.map(|a| {
                let mut end = a;
                if BLADES.contains(clip) {
                    while all
                        .get(end + 1)
                        .is_some_and(|b| b.reach - rest >= REACH_MIN && b.hit.is_some())
                    {
                        end += 1;
                    }
                }
                (a, end)
            });
            let active_s = window.map_or("None".into(), |(a, e)| format!("Some(({a}, {e}))"));
            let _ = writeln!(src, "            (\"{clip}\", {active_s}, &[");
            for (i, mut b) in all.into_iter().enumerate() {
                let inside = window.is_some_and(|(a, e)| i >= a && i <= e);
                if !inside {
                    b.hit = None;
                }
                if Some(i) != active {
                    b.hand = None;
                }
                let hurt: Vec<String> = b.hurt.iter().map(rect).collect();
                let hit = b
                    .hit
                    .as_ref()
                    .map_or("None".into(), |h| format!("Some({})", rect(h)));
                let hand = b.hand.map_or("None".into(), |h| {
                    format!("Some([{}, {}])", num(h[0]), num(h[1]))
                });
                let _ = writeln!(
                    src,
                    "                Frame {{ hurt: &[{}], hit: {hit}, hand: {hand} }},",
                    hurt.join(", ")
                );
            }
            let _ = writeln!(src, "            ]),");
        }
        let _ = writeln!(src, "        ],\n    }},");
    }
    let _ = writeln!(src, "];");
    std::fs::write(&out_path, src).map_err(|e| format!("{out_path}: {e}"))?;
    println!("frame boxes written to {out_path}");
    Ok(())
}
