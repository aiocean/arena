//! What the feel draws over the fight (the page's `duel-art.js` and `duel-hero.js`): the floating numbers in a tiny
//! pixel font with a dark outline, the qi streaks (Thunder's a crooked bolt, Frost's a straight shimmer), the dust a
//! body raises.

use super::bubble::Place;
use super::fight::{Dust, Number, NumberKind, Streak};
use super::math::{hypot, js_round, sin, sqrt};
use crate::raster::{Blend, Canvas, Rgba, fade, hex};
use arena_duel::data::Look;

/// Each look's glow (the engine's looks): a hit's number and a streak are drawn in the striker's.
pub fn glow(look: Look) -> Rgba {
    match look {
        Look::Thunder => hex("#ffe066"),
        Look::Frost => hex("#a6ecff"),
    }
}

/// 5 tall glyphs, 3 wide (the HUD's digits) or 1 (stops), and what the numbers and their words need.
fn glyph(c: char) -> Option<[&'static str; 5]> {
    Some(match c {
        '0' => ["###", "#.#", "#.#", "#.#", "###"],
        '1' => [".#.", "##.", ".#.", ".#.", "###"],
        '2' => ["##.", "..#", ".#.", "#..", "###"],
        '3' => ["##.", "..#", ".#.", "..#", "##."],
        '4' => ["#.#", "#.#", "###", "..#", "..#"],
        '5' => ["###", "#..", "##.", "..#", "##."],
        '6' => [".##", "#..", "###", "#.#", "###"],
        '7' => ["###", "..#", ".#.", ".#.", ".#."],
        '8' => ["###", "#.#", "###", "#.#", "###"],
        '9' => ["###", "#.#", "###", "..#", "##."],
        '+' => ["...", ".#.", "###", ".#.", "..."],
        '-' => ["...", "...", "###", "...", "..."],
        '!' => ["#", "#", "#", ".", "#"],
        '.' => [".", ".", ".", ".", "#"],
        'C' => [".##", "#..", "#..", "#..", ".##"],
        'O' => [".#.", "#.#", "#.#", "#.#", ".#."],
        'U' => ["#.#", "#.#", "#.#", "#.#", "###"],
        'N' => ["##.", "#.#", "#.#", "#.#", "#.#"],
        'T' => ["###", ".#.", ".#.", ".#.", ".#."],
        'E' => ["###", "#..", "##.", "#..", "###"],
        'R' => ["##.", "#.#", "##.", "#.#", "#.#"],
        'P' => ["##.", "#.#", "##.", "#..", "#.."],
        'I' => ["###", ".#.", ".#.", ".#.", "###"],
        'S' => [".##", "#..", ".#.", "..#", "##."],
        'H' => ["#.#", "#.#", "###", "#.#", "#.#"],
        _ => return None,
    })
}

/// How each kind of number looks: its glyphs' size (1 or 2 pixels a dot) and colour (none: the striker's glow). The
/// page's font sizes 9 to 13 for a blow, 7 and 8 for the small ones.
fn look(kind: NumberKind) -> (i32, Option<&'static str>) {
    match kind {
        NumberKind::Hit => (1, None),
        NumberKind::Crit => (2, Some("#ff6a3d")),
        NumberKind::Tick => (1, Some("#ff9a3a")),
        NumberKind::Heal => (1, Some("#8dffa8")),
        NumberKind::Ward => (1, Some("#a6ecff")),
        NumberKind::Dodge => (1, Some("#b8b4c8")),
        NumberKind::Counter => (2, Some("#ffd84a")),
        NumberKind::Punish => (2, Some("#ff3d6a")),
    }
}

const OUTLINE: &str = "#13101b";

/// The numbers over the fighters, centred on their `x`, standing on row `ground - y`, each pixel drawn once (the
/// outline under the face) so a fading number fades evenly.
/// A word's box (`left`, `top`, `w` x `h`, its outline a pixel round it) moved off every bubble it meets: under
/// the bubble, so the shout over a head stays readable and the word still sits by the one it was dealt to.
fn clear_of(left: i32, top: i32, w: i32, h: i32, bubbles: &[Place]) -> (i32, i32) {
    let mut top = top;
    // a word pushed under one bubble may meet the other: a few passes settle it
    for _ in 0..=bubbles.len() {
        let met = bubbles.iter().map(Place::area).find(|&(x0, y0, x1, y1)| {
            left - 1 < x1 && x0 < left + w + 1 && top - 1 < y1 && y0 < top + h + 1
        });
        match met {
            Some((_, _, _, bottom)) => top = bottom + 1,
            None => break,
        }
    }
    (left, top)
}

/// Each floating number and word, kept on the canvas and off the bubbles (`bubbles`, this frame's shouts).
pub fn numbers(
    canvas: &mut Canvas,
    list: &[Number],
    ground: f64,
    looks: [Look; 2],
    bubbles: &[Place],
) {
    for n in list {
        let (s, color) = look(n.kind);
        let color = color.map(hex).unwrap_or_else(|| {
            glow(
                n.by.and_then(|b| looks.get(b).copied())
                    .unwrap_or(Look::Thunder),
            )
        });
        let glyphs: Vec<[&str; 5]> = n.text.chars().filter_map(glyph).collect();
        let w = glyphs
            .iter()
            .map(|g| (g[0].len() as i32 + 1) * s)
            .sum::<i32>()
            - s;
        if w <= 0 {
            continue;
        }
        let h = 5 * s;
        // kept on the canvas, outline and all: a word over a fighter at the wall is not cut by the edge
        let left = (js_round(n.x) as i32 - w / 2).clamp(1, (canvas.w - w - 1).max(1));
        let top = js_round(ground - n.y) as i32 - h;
        let (left, top) = clear_of(left, top, w, h, bubbles);
        // the face's dots in a grid with a pixel of room all round for the outline
        let (gw, gh) = (w + 2, h + 2);
        let mut face = vec![false; (gw * gh) as usize];
        let mut pen = 0;
        for rows in &glyphs {
            for (r, row) in rows.iter().enumerate() {
                for (k, b) in row.bytes().enumerate() {
                    if b != b'#' {
                        continue;
                    }
                    for dy in 0..s {
                        for dx in 0..s {
                            let x = 1 + pen + k as i32 * s + dx;
                            let y = 1 + r as i32 * s + dy;
                            face[(y * gw + x) as usize] = true;
                        }
                    }
                }
            }
            pen += (rows[0].len() as i32 + 1) * s;
        }
        let at =
            |x: i32, y: i32| x >= 0 && y >= 0 && x < gw && y < gh && face[(y * gw + x) as usize];
        let a = n.alpha as f32;
        let (dark, bright) = (fade(hex(OUTLINE), a), fade(color, a));
        for y in 0..gh {
            for x in 0..gw {
                let c = if at(x, y) {
                    bright
                } else if [(1, 0), (-1, 0), (0, 1), (0, -1), (-1, -1)]
                    .iter()
                    .any(|(dx, dy)| at(x + dx, y + dy))
                {
                    dark
                } else {
                    continue;
                };
                canvas.put(left - 1 + x, top - 1 + y, c, Blend::Over);
            }
        }
    }
}

/// A pixel line between two points; `thin` puts every other pixel, a bright core over the glow.
fn plot(canvas: &mut Canvas, (x0, y0): (f64, f64), (x1, y1): (f64, f64), c: Rgba, thin: bool) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1.0);
    let mut i = 0.0;
    while i <= n {
        let x = js_round(x0 + (x1 - x0) * i / n) as i32;
        let y = js_round(y0 + (y1 - y0) * i / n) as i32;
        canvas.put(x, y, c, Blend::Over);
        i += if thin { 2.0 } else { 1.0 };
    }
}

/// Each qi streak from the caster's hand to where its spell lands.
pub fn streaks(canvas: &mut Canvas, list: &[Streak], ground: f64, looks: [Look; 2]) {
    for k in list {
        let look = looks.get(k.by).copied().unwrap_or(Look::Thunder);
        let (x0, y0, x1, y1) = (k.x0, ground - k.y0, k.x1, ground - k.y1);
        let n = js_round(hypot(x1 - x0, y1 - y0) / 6.0).max(2.0) as usize;
        let pts: Vec<(f64, f64)> = (0..=n)
            .map(|i| {
                let u = i as f64 / n as f64;
                // lightning jags, ice runs true
                let jag = if look == Look::Thunder && i > 0 && i < n {
                    js_round(sin(i as f64 * 12.9898 + k.x0 * 78.233) * 3.0)
                } else {
                    0.0
                };
                (
                    js_round(x0 + (x1 - x0) * u),
                    js_round(y0 + (y1 - y0) * u) + jag,
                )
            })
            .collect();
        let a = (k.fade * 1.6).min(1.0) as f32;
        let white = hex("#ffffff");
        for i in 0..n {
            plot(canvas, pts[i], pts[i + 1], fade(glow(look), a), false);
            if look == Look::Frost && i % 2 == 0 {
                canvas.put(
                    pts[i].0 as i32,
                    pts[i].1 as i32 - 1,
                    fade(white, a),
                    Blend::Over,
                );
            }
        }
        let a = (k.fade * 1.2).min(1.0) as f32;
        for i in 0..n {
            plot(canvas, pts[i], pts[i + 1], fade(white, a), true);
        }
    }
}

/// The dust a body raises: specks thrown out to both sides along the floor, rising a little and settling as they
/// fade (the engine's `draw_dust`).
pub fn dust(canvas: &mut Canvas, list: &[Dust], ground: f64) {
    let (light, dark) = (hex("#a89a88"), hex("#6e6458"));
    for d in list {
        let n = 2.0 + js_round(3.0 * d.strength);
        let alpha = (0.8 * (1.0 - d.k)) as f32;
        let mut i = 0.0;
        while i < n {
            let out = js_round((3.0 + i * 3.0) * sqrt(d.k) * (0.6 + d.strength));
            let rise = js_round(
                sin(std::f64::consts::PI * d.k) * (1.0 + (i * 7.0) % 3.0) * d.strength * 2.0,
            );
            let c = if i % 2.0 != 0.0 { light } else { dark };
            let w = if i < 2.0 { 2 } else { 1 };
            for side in [-1.0, 1.0] {
                let x = js_round(d.x + side * out) as i32;
                let y = (ground - 1.0 - rise) as i32;
                canvas.rect(x, y, w, 1, fade(c, alpha), Blend::Over);
            }
            i += 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feel::fight::NumberKind;

    #[test]
    fn a_word_by_the_wall_stays_on_the_canvas() {
        let lit_at = |x: f64| {
            let mut canvas = Canvas::new(200, 100);
            let word = Number {
                x,
                y: 40.0,
                alpha: 1.0,
                text: "COUNTER!".to_owned(),
                by: Some(0),
                kind: NumberKind::Counter,
            };
            numbers(
                &mut canvas,
                &[word],
                100.0,
                [Look::Thunder, Look::Frost],
                &[],
            );
            canvas.px.iter().filter(|p| p[3] > 0).count()
        };
        let whole = lit_at(100.0);
        assert!(whole > 0);
        assert_eq!(lit_at(199.0), whole, "cut by the right edge");
        assert_eq!(lit_at(0.0), whole, "cut by the left edge");
    }

    /// The seed 9 case (`review/visual-qa`, 3.5 s): "PUNISH!" dealt at the head of a fighter whose shout is up was
    /// painted over the bubble. It goes under it, and nothing of the two shares a pixel.
    #[test]
    fn a_word_keeps_off_a_bubble() {
        let (w, h, ground) = (320, 180, 158);
        let (x, feet) = (95, 120);
        let p = crate::feel::bubble::place(x, feet, 48, 11, w, 30);
        let paint = |bubbles: &[Place]| {
            let mut canvas = Canvas::new(w, h);
            let word = Number {
                x: f64::from(x),
                // its foot on the bubble's middle
                y: f64::from(ground - (p.y + p.h / 2)),
                alpha: 1.0,
                text: "PUNISH!".to_owned(),
                by: Some(1),
                kind: NumberKind::Punish,
            };
            numbers(
                &mut canvas,
                &[word],
                f64::from(ground),
                [Look::Thunder, Look::Frost],
                bubbles,
            );
            canvas
        };
        let mut shout = Canvas::new(w, h);
        crate::feel::bubble::bubble(&mut shout, &p, [255, 200, 80, 255], 1.0);
        let shared = |c: &Canvas| {
            c.px.iter()
                .zip(&shout.px)
                .filter(|(a, b)| a[3] > 0 && b[3] > 0)
                .count()
        };
        assert!(
            shared(&paint(&[])) > 0,
            "the case does not overlap to begin with"
        );
        assert_eq!(
            shared(&paint(&[p])),
            0,
            "the word is painted over the bubble"
        );
    }
}
