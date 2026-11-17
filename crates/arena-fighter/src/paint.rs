//! A fighter's frame drawn on an engine `Frame` the way the engine draws its own fighters: feet on the floor at
//! the fighter's place, turned to its facing, jolted by a blow, white in a hit's flash, blinking while nothing can
//! touch it, a fading white copy behind a dodge; then the marks that hang on it, which the engine draws. The sheets
//! are finer than the engine's pixels: a fighter is drawn on a frame `unit` times finer than the world it stands in,
//! the marks on one in the engine's own pixels, and [`coarse`] gives the fighter in those for the effects that wrap
//! it.

use crate::pick::pick;
use crate::sheet::{Cell, Sheet};
use pixfx_core::squash::{SquashSpec, blink};
use pixfx_draw::math::js_round;
use pixfx_draw::{Frame, Rgba};
use pixfx_fighter::paint::Appearance;
use pixfx_fighter::status::{self, DomeStyle, Figure, Mark, Silhouette};
use serde::Deserialize;

/// What hangs on a fighter (the engine's marks): a ward's shell, a barrier (`dome` its pattern, `rang` when a
/// blow has just rung on it), a sign for each effect on it, `fury` pips, and fire (`burn`), lightning (`shock`)
/// and frost (`ice`) over the body at time `t`, drawn from `seed`.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Marks {
    pub shield: bool,
    pub barrier: bool,
    pub rang: bool,
    pub burning: bool,
    pub def_down: bool,
    pub silence: bool,
    pub chilled: bool,
    pub fury: u32,
    /// Seconds since the pose began.
    pub pose_t: f64,
    pub dome: DomeStyle,
    pub burn: bool,
    pub shock: bool,
    pub ice: bool,
    pub t: f64,
    pub seed: u32,
}

/// Fighter `a` from `sheet`, its feet on the floor at row `ground`, on a frame `unit` times finer than the world `a`
/// and `ground` are in.
pub fn draw(frame: &mut Frame, sheet: &Sheet, a: &Appearance, ground: f64, unit: u32) {
    let shot = pick(a, sheet.layout.fps, |clip| sheet.len(clip));
    let Some(cell) = sheet.cell(shot.clip, shot.frame) else {
        return;
    };
    let k = f64::from(unit);
    let jolt = [a.jolt[0] * unit as i32, a.jolt[1] * unit as i32];
    let feet = (
        js_round(a.x * k) as i32 + jolt[0],
        js_round((ground - a.air) * k) as i32 + jolt[1],
    );
    if a.dodged > 0.0 && !a.out {
        let ghost = (
            feet.0 - jolt[0] + js_round(a.face * 7.0 * k) as i32,
            feet.1 - jolt[1],
        );
        stamp(
            frame,
            sheet,
            cell,
            ghost,
            a.face,
            true,
            (a.dodged * 1.5).min(1.0),
        );
    }
    let alpha = if a.untouchable && !a.out {
        blink(
            &SquashSpec {
                blink_rate: 10.0,
                ..SquashSpec::default()
            },
            a.pose_t,
        )
    } else {
        1.0
    };
    stamp(frame, sheet, cell, feet, a.face, a.flash, alpha);
}

/// The marks on fighter `a` (feet on row `ground`), a barrier in `glow`, in the order the engine's own fighters
/// have them.
pub fn draw_marks(frame: &mut Frame, marks: &Marks, a: &Appearance, ground: f64, glow: Rgba) {
    let mut list = Vec::new();
    if marks.shield {
        list.push(Mark::Shield);
    }
    if marks.barrier {
        list.push(Mark::Barrier {
            style: marks.dome,
            glow,
            rang: marks.rang,
        });
    }
    for (on, mark) in [
        (marks.burning, Mark::Burning),
        (marks.def_down, Mark::DefDown),
        (marks.silence, Mark::Silence),
        (marks.chilled, Mark::Chilled),
    ] {
        if on {
            list.push(mark);
        }
    }
    list.push(Mark::Fury(marks.fury));
    let figure = Figure {
        feet_x: a.x,
        feet_y: ground - a.air,
        face: a.face,
        silhouette: Silhouette::of(a.pose),
    };
    if marks.burn {
        status::draw_burning(frame, marks.t, marks.seed, &figure, 1.0);
    }
    if marks.ice {
        status::draw_frozen(frame, marks.t, marks.seed, &figure, 1.0);
    }
    if marks.shock {
        status::draw_shocked(frame, marks.t, marks.seed, &figure, 1.0);
    }
    status::draw_marks(frame, &list, marks.pose_t, figure.feet_x, figure.feet_y);
}

/// The fighter drawn on `fine` (`unit` times finer than `out`) in `out`'s pixels, for an effect that wraps it: each
/// pixel the most opaque of its `unit` x `unit` block, the first of them on a tie.
pub fn coarse(fine: &Frame, unit: u32, out: &mut Frame) {
    let u = unit.max(1) as usize;
    let (fw, fh) = (fine.w as usize, fine.h as usize);
    for y in 0..out.h as usize {
        for x in 0..out.w as usize {
            let mut best = 0_u32;
            for v in y * u..((y + 1) * u).min(fh) {
                for c in x * u..((x + 1) * u).min(fw) {
                    let word = fine.color[v * fw + c];
                    if word >> 24 > best >> 24 {
                        best = word;
                    }
                }
            }
            out.color[y * out.w as usize + x] = best;
        }
    }
}

/// The sheet's face for the HUD: its portrait box scaled by whole pixels to fit a `size` square at (`x`, `y`),
/// centred, at `alpha`, looking right (`face` 1) or mirrored to look left (-1).
pub fn draw_portrait(
    frame: &mut Frame,
    sheet: &Sheet,
    x: i32,
    y: i32,
    size: u32,
    alpha: f64,
    face: f64,
) {
    let p = sheet.layout.portrait;
    let k = (size / p.w.max(p.h).max(1)).max(1);
    let (w, h) = (p.w * k, p.h * k);
    let (x0, y0) = (
        x + (size.saturating_sub(w) / 2) as i32,
        y + (size.saturating_sub(h) / 2) as i32,
    );
    for v in 0..h.min(size) {
        for u in 0..w.min(size) {
            let col = if face < 0.0 { w - 1 - u } else { u };
            let px = sheet.at(p.page, p.x + col / k, p.y + v / k);
            over(frame, x0 + u as i32, y0 + v as i32, px, alpha);
        }
    }
}

/// `cell` drawn with its feet at `feet`, mirrored when facing left, all white when `white`.
fn stamp(
    frame: &mut Frame,
    sheet: &Sheet,
    cell: Cell,
    feet: (i32, i32),
    face: f64,
    white: bool,
    alpha: f64,
) {
    if alpha <= 0.0 {
        return;
    }
    for v in 0..cell.h {
        for u in 0..cell.w {
            let mut px = sheet.at(cell.page, cell.x + u, cell.y + v);
            if px[3] == 0 {
                continue;
            }
            if white {
                px = [255, 255, 255, px[3]];
            }
            let col = cell.dx + u as i32;
            // a mirror about the feet column: pixel column c (covering c..c+1) lands on -c-1
            let x = if face < 0.0 {
                feet.0 - col - 1
            } else {
                feet.0 + col
            };
            over(frame, x, feet.1 + cell.dy + v as i32, px, alpha);
        }
    }
}

/// Straight-alpha `px` at `alpha` over the frame's pixel at world (`x`, `y`); nothing off the frame.
fn over(frame: &mut Frame, x: i32, y: i32, px: [u8; 4], alpha: f64) {
    let (fx, fy) = (x - frame.ox, y - frame.oy);
    if fx < 0 || fy < 0 || fx >= frame.w as i32 || fy >= frame.h as i32 {
        return;
    }
    let i = fy as usize * frame.w as usize + fx as usize;
    let Some(word) = frame.color.get_mut(i) else {
        return;
    };
    let sa = f64::from(px[3]) / 255.0 * alpha.clamp(0.0, 1.0);
    if sa <= 0.0 {
        return;
    }
    let d = word.to_le_bytes();
    let da = f64::from(d[3]) / 255.0;
    let oa = sa + da * (1.0 - sa);
    let ch =
        |s: u8, t: u8| js_round((f64::from(s) * sa + f64::from(t) * da * (1.0 - sa)) / oa) as u8;
    *word = u32::from_le_bytes([
        ch(px[0], d[0]),
        ch(px[1], d[1]),
        ch(px[2], d[2]),
        js_round(oa * 255.0) as u8,
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use pixfx_fighter::pose::Pose;

    /// A sheet whose every animation is one 2x3 frame: a red column left of a blue one, feet under its
    /// bottom-left pixel; its portrait the same box.
    type Res = Result<(), crate::LayoutError>;

    fn sheet() -> Result<Sheet, crate::LayoutError> {
        let cell = r#"{"x":0,"y":0,"w":2,"h":3,"dx":0,"dy":-3}"#;
        let clips: Vec<String> = crate::pick::CLIPS
            .iter()
            .map(|c| format!("\"{c}\":[{cell}]"))
            .collect();
        let json = format!(
            r#"{{"size":[2,3],"fps":10,"clips":{{{}}},"portrait":{cell}}}"#,
            clips.join(",")
        );
        let rgba: Vec<u8> = (0..6)
            .flat_map(|i| {
                if i % 2 == 0 {
                    [255, 0, 0, 255]
                } else {
                    [0, 0, 255, 255]
                }
            })
            .collect();
        Sheet::new(&json, &rgba, 2, 3)
    }

    fn standing(face: f64) -> Appearance {
        Appearance {
            pose: Pose::Idle,
            pose_t: 0.0,
            face,
            x: 10.0,
            air: 0.0,
            vx: 0.0,
            vy: 0.0,
            out: false,
            flash: false,
            untouchable: false,
            frozen: false,
            shocked: false,
            hit_t: 9.0,
            hit_power: 0.0,
            dodged: 0.0,
            act: None,
            jolt: [0, 0],
        }
    }

    fn at(f: &Frame, x: i32, y: i32) -> [u8; 4] {
        f.color[(y * f.w as i32 + x) as usize].to_le_bytes()
    }

    #[test]
    fn the_feet_stand_on_the_floor_at_the_fighters_place() -> Res {
        let mut f = Frame::new(20, 20, 0, 0);
        draw(&mut f, &sheet()?, &standing(1.0), 15.0, 1);
        assert_eq!(
            at(&f, 10, 14),
            [255, 0, 0, 255],
            "bottom-left pixel just above the floor at x"
        );
        assert_eq!(at(&f, 11, 12), [0, 0, 255, 255]);
        assert_eq!(at(&f, 10, 11), [0; 4], "nothing above the frame's top");
        assert_eq!(at(&f, 10, 15), [0; 4], "nothing on the floor row");
        Ok(())
    }

    #[test]
    fn facing_left_mirrors_about_the_feet() -> Res {
        let mut f = Frame::new(20, 20, 0, 0);
        draw(&mut f, &sheet()?, &standing(-1.0), 15.0, 1);
        assert_eq!(
            at(&f, 9, 14),
            [255, 0, 0, 255],
            "red stays next to the feet column"
        );
        assert_eq!(
            at(&f, 8, 14),
            [0, 0, 255, 255],
            "blue goes to the other side"
        );
        assert_eq!(at(&f, 10, 14), [0; 4]);
        Ok(())
    }

    #[test]
    fn a_hits_flash_draws_it_white_and_a_jolt_moves_it() -> Res {
        let mut f = Frame::new(20, 20, 0, 0);
        let mut a = standing(1.0);
        a.flash = true;
        a.jolt = [2, -1];
        draw(&mut f, &sheet()?, &a, 15.0, 1);
        assert_eq!(at(&f, 12, 13), [255, 255, 255, 255]);
        Ok(())
    }

    #[test]
    fn a_finer_frame_puts_the_feet_unit_times_further_and_jolts_by_whole_world_pixels() -> Res {
        let mut f = Frame::new(40, 40, 0, 0);
        let mut a = standing(1.0);
        a.jolt = [1, 0];
        draw(&mut f, &sheet()?, &a, 15.0, 2);
        // feet at (10 * 2 + 2, 15 * 2): the sheet's own pixels, not doubled
        assert_eq!(at(&f, 22, 29), [255, 0, 0, 255]);
        assert_eq!(at(&f, 23, 27), [0, 0, 255, 255]);
        assert_eq!(at(&f, 22, 26), [0; 4]);
        Ok(())
    }

    #[test]
    fn coarse_keeps_the_most_opaque_pixel_of_each_block() {
        let mut fine = Frame::new(4, 2, 0, 0);
        fine.color[1] = u32::from_le_bytes([9, 9, 9, 100]);
        fine.color[4] = u32::from_le_bytes([1, 2, 3, 255]);
        fine.color[3] = u32::from_le_bytes([7, 7, 7, 40]);
        let mut out = Frame::new(2, 1, 0, 0);
        coarse(&fine, 2, &mut out);
        assert_eq!(out.color[0].to_le_bytes(), [1, 2, 3, 255]);
        assert_eq!(out.color[1].to_le_bytes(), [7, 7, 7, 40]);
    }

    #[test]
    fn the_portrait_scales_by_whole_pixels_and_centres() -> Res {
        let mut f = Frame::new(20, 20, 0, 0);
        draw_portrait(&mut f, &sheet()?, 0, 0, 7, 1.0, 1.0);
        // a 2x3 box scaled 2 to 4x6, centred in 7: from (1, 0)
        assert_eq!(at(&f, 1, 0), [255, 0, 0, 255]);
        assert_eq!(at(&f, 3, 5), [0, 0, 255, 255]);
        assert_eq!(at(&f, 0, 0), [0; 4]);
        let mut g = Frame::new(20, 20, 0, 0);
        draw_portrait(&mut g, &sheet()?, 0, 0, 7, 1.0, -1.0);
        assert_eq!(
            at(&g, 1, 0),
            [0, 0, 255, 255],
            "looking left, blue comes first"
        );
        Ok(())
    }
}
