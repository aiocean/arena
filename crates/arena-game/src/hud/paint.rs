//! The HUD painted in world pixels (from the page's `duel-art.js`, `drawBars` and `drawSkills`): for each fighter a
//! bronze medallion with its face, its life with a draining trail, its qi, its rage in segments and its buffs in
//! tiles, the plaque of rounds won between the two, and each fighter's skill bar on the floor under its side. What
//! lies under the skill icons goes on `base`, what lies over them on `over`; the faces and icons themselves are
//! sprites (finer than the world), placed from what is painted here.

use super::glyphs::{DIGIT, buff, icon_cell};
use crate::layout::{BARS_TALL, Layout};
use crate::raster::{Blend, Canvas, Rgba, fade, hex};
use bevy::math::ops;
use pixfx_duel::data::{Look, RAGE};
use pixfx_duel::view::{DuelView, FighterView};

const BRONZE_LIGHT: &str = "#f0cf7a";
const BRONZE_MID: &str = "#b8893c";
const BRONZE_DARK: &str = "#6a4a1c";
const BRONZE_DEEP: &str = "#2a1c0e";
const BAR_BACK: &str = "#160f1c";
const BAR_LOST: &str = "#5a1a14";
const BAR_TRAIL: &str = "#fff0d0";
const BAR_QI: &str = "#7fb4ff";
const BAR_QI_LIGHT: &str = "#c8e0ff";
const BAR_RAGE: &str = "#ff5a32";
const BAR_RAGE_LIGHT: &str = "#ffb070";
const WHITE: &str = "#ffffff";
const TILE: &str = "#120d18";
/// A buff tile's rim: green for a good one, red for a bad one.
const RIM_GOOD: &str = "#3fae5a";
const RIM_BAD: &str = "#c8322a";
const SLOT_FIELD: &str = "#1a1424";
const SLOT_SHADE: &str = "#06050c";
/// How stale a bar skill is (0 to 1) when its icon starts to flicker before it leaves the bar.
const STALE_FLICKER: f64 = 2.0 / 3.0;
/// How long a slot flashes when a spell in it is cast, and glints when its cooldown ends, in seconds.
const GLINT: f32 = 0.25;

/// A look's colours the HUD draws in (the engine's looks): its life bar's ribbon, light then dark, and its glow.
pub struct Colors {
    pub ribbon: [Rgba; 2],
    pub glow: Rgba,
}

pub fn colors(s: Look) -> Colors {
    match s {
        Look::Thunder => Colors {
            ribbon: [hex("#e0443a"), hex("#a8241c")],
            glow: hex("#ffe066"),
        },
        Look::Frost => Colors {
            ribbon: [hex("#bff2ff"), hex("#7ab8e0")],
            glow: hex("#a6ecff"),
        },
    }
}

/// JS `Math.round`.
fn round(x: f32) -> i32 {
    ops::floor(x + 0.5) as i32
}

/// Where the HUD's parts stand, in world pixels: the arena's walls, the bars' top, the floor the skill bars stand
/// on and the icons' size.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub left: i32,
    pub right: i32,
    pub top: i32,
    pub bottom: i32,
    pub icon: i32,
}

impl Geo {
    pub fn of(l: &Layout) -> Geo {
        Geo {
            left: l.left as i32,
            right: l.right as i32,
            top: l.bar_top as i32,
            bottom: l.h as i32,
            icon: l.icon as i32,
        }
    }

    /// Slot `i` of fighter `f`'s skill bar: x, y, w, h.
    pub fn slot(&self, f: usize, i: usize) -> [i32; 4] {
        let size = self.icon + 4;
        let i = i as i32;
        let x = if f == 0 {
            self.left + i * (size + 2)
        } else {
            self.right - size - i * (size + 2)
        };
        [x, self.bottom - size, size, size]
    }

    /// Buff tile `i` of fighter `f`, under the bars: x, y, w, h.
    pub fn buff(&self, f: usize, i: usize) -> [i32; 4] {
        let i = i as i32;
        let x = if f == 1 {
            self.right - 9 - i * 11
        } else {
            self.left + i * 11
        };
        [x, self.top + BARS_TALL as i32 + 2, 9, 9]
    }
}

/// What of the HUD lies under a world pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// Fighter, slot of its skill bar.
    Skill(usize, usize),
    /// Fighter, buff of its `buffs`.
    Buff(usize, usize),
}

pub fn hud_at(view: &DuelView, g: &Geo, px: f32, py: f32) -> Option<Hit> {
    let inside = |[x, y, w, h]: [i32; 4]| {
        px >= x as f32 && px < (x + w) as f32 && py >= y as f32 && py < (y + h) as f32
    };
    for f in &view.fighters {
        for i in 0..f.skill_bar.len() {
            if inside(g.slot(f.index, i)) {
                return Some(Hit::Skill(f.index, i));
            }
        }
        for i in 0..f.buffs.len() {
            if inside(g.buff(f.index, i)) {
                return Some(Hit::Buff(f.index, i));
            }
        }
    }
    None
}

/// What the HUD remembers of a fighter from frame to frame.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    /// The life its bar last showed, so a blow's loss drains away behind the bar.
    pub trail: Option<f64>,
    /// Each slot's cooldown last frame, and when it last came off cooldown (fight time).
    pub left: Vec<f64>,
    pub at: Vec<f64>,
}

impl Seen {
    pub fn slots(&mut self, n: usize) {
        if self.left.len() < n {
            self.left.resize(n, 0.0);
            self.at.resize(n, f64::NEG_INFINITY);
        }
    }
}

/// A fighter's face in its medallion: the `size` square at (`x`, `y`) its portrait fits.
#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub school: usize,
    pub x: i32,
    pub y: i32,
    pub size: i32,
    pub alpha: f32,
    pub flip: bool,
}

/// A skill's icon: its cell in the atlas, drawn `n` square at (`x`, `y`).
#[derive(Clone, Copy, Debug)]
pub struct Icon {
    pub cell: u32,
    pub x: i32,
    pub y: i32,
    pub n: i32,
    pub alpha: f32,
}

/// The sprites this frame's HUD wants.
#[derive(Clone, Debug, Default)]
pub struct Painted {
    pub faces: Vec<Face>,
    pub icons: Vec<Icon>,
}

/// Rectangles and bitmaps in world pixels.
pub struct Pen<'a>(pub &'a mut Canvas);

impl Pen<'_> {
    fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba, a: f32) {
        if w > 0 && h > 0 && a > 0.0 {
            self.0.rect(x, y, w, h, fade(c, a), Blend::Over);
        }
    }

    fn hex(&mut self, x: i32, y: i32, w: i32, h: i32, c: &str) {
        self.rect(x, y, w, h, hex(c), 1.0);
    }

    fn glyph(&mut self, rows: &[&str], x: i32, y: i32, c: Rgba, z: i32, a: f32) {
        for (r, row) in rows.iter().enumerate() {
            for (k, ch) in row.chars().enumerate() {
                if ch == '#' {
                    self.rect(x + k as i32 * z, y + r as i32 * z, z, z, c, a);
                }
            }
        }
    }

    /// A one-pixel frame round the edge of an `n`-square at (`x`, `y`).
    fn frame(&mut self, x: i32, y: i32, n: i32, c: Rgba, a: f32) {
        self.rect(x, y, n, 1, c, a);
        self.rect(x, y + n - 1, n, 1, c, a);
        self.rect(x, y + 1, 1, n - 2, c, a);
        self.rect(x + n - 1, y + 1, 1, n - 2, c, a);
    }
}

/// Where in a clockwise turn from twelve o'clock pixel (`px`, `py`) of an `n`-square lies, 0 to 1.
fn turn(px: i32, py: i32, n: i32) -> f32 {
    let mid = (n - 1) as f32 / 2.0;
    (ops::atan2(px as f32 - mid, mid - py as f32) / std::f32::consts::TAU + 1.0).rem_euclid(1.0)
}

/// Shade a share of an `n`-square from twelve o'clock: `share` > 0 the first part of the turn clockwise, < 0 the
/// last part (what a cooldown still has to run).
fn sweep(pen: &mut Pen, x: i32, y: i32, n: i32, share: f32, c: Rgba, a: f32) {
    for py in 0..n {
        for px in 0..n {
            let t = turn(px, py, n);
            if if share >= 0.0 {
                t < share
            } else {
                t >= 1.0 + share
            } {
                pen.rect(x + px, y + py, 1, 1, c, a);
            }
        }
    }
}

/// Light the first `share` of an `n`-square's outer ring, clockwise from twelve o'clock.
fn ring2(pen: &mut Pen, x: i32, y: i32, n: i32, share: f32, c: Rgba) {
    for py in 0..n {
        for px in 0..n {
            if px > 1 && px < n - 2 && py > 1 && py < n - 2 {
                continue;
            }
            if turn(px, py, n) < share {
                pen.rect(x + px, y + py, 1, 1, c, 1.0);
            }
        }
    }
}

/// The fight's bars: each fighter's medallion, life, qi, rage and buffs, and the plaque of rounds won between them.
/// `frames` is this frame's length in sixtieths of a second (the trail drains by the frame, as the page's did).
pub fn bars(
    pen: &mut Pen,
    view: &DuelView,
    g: &Geo,
    seen: &mut [Seen; 2],
    frames: f64,
    faces: &mut Vec<Face>,
) {
    let span = g.right - g.left;
    let medal = BARS_TALL as i32;
    // a narrow arena hangs the plaque under the bars, between the rage beads, so the bars get its room
    let narrow = span < 260;
    let width = (span.div_euclid(2) - medal - if narrow { 5 } else { 26 }).clamp(30, 120);
    let top = g.top;
    let time = view.time as f32;
    let (light, mid, deep, back) = (
        hex(BRONZE_LIGHT),
        hex(BRONZE_MID),
        hex(BRONZE_DEEP),
        hex(BAR_BACK),
    );
    for f in &view.fighters {
        let right = f.index == 1;
        let c = colors(f.look);
        let mx = if right { g.right - medal } else { g.left };
        medallion(pen, f, mx, top, medal, &c);
        faces.push(Face {
            school: f.index.min(1),
            x: mx + 3,
            y: top + 3,
            size: medal - 6,
            alpha: if f.hp <= 0.0 { 0.35 } else { 1.0 },
            flip: f.index != 0,
        });
        let left = if right {
            mx - 3 - width
        } else {
            mx + medal + 3
        };
        // each bar grows from its medallion outward
        let from = |w: i32| if right { left + width - w } else { left };
        // life: a bronze frame with end caps
        pen.rect(left - 2, top + 2, width + 4, 9, deep, 1.0);
        pen.rect(left - 1, top + 3, width + 2, 7, mid, 1.0);
        pen.rect(left - 1, top + 3, width + 2, 1, light, 1.0);
        pen.rect(left, top + 4, width, 5, back, 1.0);
        let hp = round(width as f32 * f.hp as f32 / 100.0).max(0);
        let s = &mut seen[f.index.min(1)];
        let last = f.hp.max(s.trail.unwrap_or(f.hp).min(100.0));
        s.trail = Some(last - ((last - f.hp) * 0.06).max(0.4) * frames);
        let trail = round(width as f32 * last as f32 / 100.0).max(0);
        pen.hex(from(trail), top + 4, trail, 5, BAR_LOST);
        pen.rect(from(trail), top + 4, trail, 5, hex(BAR_TRAIL), 0.25);
        pen.rect(from(hp), top + 4, hp, 5, c.ribbon[1], 1.0);
        pen.rect(from(hp), top + 4, hp, 2, c.ribbon[0], 1.0);
        pen.rect(from(hp), top + 4, hp, 1, hex(WHITE), 0.35);
        // a notch each tenth
        for i in 1..10 {
            pen.rect(
                left + round(width as f32 * i as f32 / 10.0),
                top + 8,
                1,
                1,
                back,
                0.6,
            );
        }
        let cap = if right { left - 2 } else { left + width + 1 };
        pen.rect(cap, top + 4, 1, 5, light, 1.0);
        pen.rect(cap + if right { -1 } else { 1 }, top + 5, 1, 3, mid, 1.0);
        // qi: a blue bar in its own frame under the life bar
        pen.rect(left - 1, top + 12, width + 2, 4, deep, 1.0);
        pen.rect(left, top + 13, width, 2, back, 1.0);
        let qi = round(width as f32 * f.qi as f32 / 100.0).max(0);
        pen.hex(from(qi), top + 13, qi, 2, BAR_QI);
        pen.hex(from(qi), top + 13, qi, 1, BAR_QI_LIGHT);
        // rage: a red bar of its own under the qi, a segment per point, blazing once full
        let full = f.rage >= RAGE;
        let blaze = full && ops::sin(time * 12.0) > 0.0;
        let points = RAGE as i32;
        let seg = ((width - if narrow { 20 } else { 0 } + 1).div_euclid(points)).clamp(3, 9);
        let rage_w = points * seg - 1;
        let rx = if right { left + width - rage_w } else { left };
        if full {
            pen.rect(
                rx - 2,
                top + 16,
                rage_w + 4,
                8,
                hex(BAR_RAGE),
                if blaze { 0.35 } else { 0.15 },
            );
        }
        pen.rect(rx - 1, top + 17, rage_w + 2, 6, deep, 1.0);
        for i in 0..points {
            let x = if right {
                rx + rage_w - seg + 1 - i * seg
            } else {
                rx + i * seg
            };
            let on = (i as f64) < f.rage;
            let fill = match (on, blaze) {
                (false, _) => back,
                (true, true) => hex(WHITE),
                (true, false) => hex(BAR_RAGE),
            };
            pen.rect(x, top + 18, seg - 1, 4, fill, 1.0);
            if on {
                pen.hex(
                    x,
                    top + 18,
                    seg - 1,
                    1,
                    if blaze { WHITE } else { BAR_RAGE_LIGHT },
                );
            }
        }
        buffs(pen, g, f);
    }
    // the plaque in the middle: the rounds each fighter has won, as a number in its own colour on its own half
    let mid_x = round((g.left + g.right) as f32 / 2.0);
    let top = top + if narrow { 13 } else { 0 };
    pen.rect(mid_x - 18, top + 2, 36, 9, deep, 1.0);
    pen.hex(mid_x - 17, top + 3, 34, 7, BRONZE_DARK);
    pen.rect(mid_x - 17, top + 3, 34, 1, mid, 1.0);
    pen.rect(mid_x, top + 4, 1, 5, mid, 1.0);
    for f in &view.fighters {
        let wins = view.wins.get(f.index).copied().unwrap_or(0).min(999);
        let digits: Vec<usize> = wins
            .to_string()
            .chars()
            .filter_map(|d| d.to_digit(10))
            .map(|d| d as usize)
            .collect();
        let w = digits.len() as i32 * 4 - 1;
        // centred in its half, which is 17 pixels wide
        let x0 = if f.index == 0 {
            mid_x - 17 + (17 - w).div_euclid(2)
        } else {
            mid_x + 1 + (17 - w + 1).div_euclid(2)
        };
        let glow = colors(f.look).glow;
        for (i, d) in digits.iter().enumerate() {
            if let Some(rows) = DIGIT.get(*d) {
                pen.glyph(rows, x0 + i as i32 * 4, top + 4, glow, 1, 1.0);
            }
        }
    }
}

/// A bronze medallion at (`x`, `top`), `size` across, lit in the fighter's colour once its rage is full.
fn medallion(pen: &mut Pen, f: &FighterView, x: i32, top: i32, size: i32, c: &Colors) {
    // an octagon: a dark rim, bronze lit from above, a dark field
    let inset = 3;
    for y in 0..size {
        let cut = 0.max(inset - y).max(y - (size - 1 - inset));
        pen.hex(x + cut, top + y, size - 2 * cut, 1, BRONZE_DEEP);
        if y > 0 && y < size - 1 {
            let lit = if (y as f32) < size as f32 / 2.0 {
                BRONZE_LIGHT
            } else {
                BRONZE_MID
            };
            pen.hex(x + cut + 1, top + y, size - 2 * cut - 2, 1, lit);
        }
        if y > 1 && y < size - 2 {
            pen.hex(x + cut + 2, top + y, size - 2 * cut - 4, 1, "#1a1424");
        }
    }
    let full = f.rage >= RAGE;
    pen.rect(
        x + 4,
        top + 4,
        size - 8,
        size - 8,
        if full { c.glow } else { hex("#241a30") },
        if full { 0.35 } else { 1.0 },
    );
}

/// Fighter `f`'s buffs as 9 by 9 tiles in a row under its bars: an icon on a dark field, a rim green for a good one
/// and red for a bad one, and its stacks as a digit in its lower right corner.
fn buffs(pen: &mut Pen, g: &Geo, f: &FighterView) {
    for (i, b) in f.buffs.iter().enumerate() {
        let Some((color, rows)) = buff(b.id) else {
            continue;
        };
        let [x, top, ..] = g.buff(f.index, i);
        pen.hex(x, top, 9, 9, if b.good { RIM_GOOD } else { RIM_BAD });
        pen.hex(x + 1, top + 1, 7, 7, TILE);
        pen.glyph(&rows, x + 1, top + 1, hex(color), 1, 1.0);
        if b.stacks > 0.0 {
            // inside the tile's lower right corner, so a tile at the screen's edge keeps its digit
            let d = (b.stacks as usize).clamp(1, 9);
            pen.hex(x + 4, top + 2, 5, 7, TILE);
            pen.glyph(&DIGIT[d], x + 5, top + 3, hex(WHITE), 1, 1.0);
        }
    }
}

/// Each fighter's skill bar on the floor under its own side: slots 1, 2 and 3 and the ultimate, the ultimate nearest
/// the middle. A slot's frame is bronze (the bar's tiers are off); the icon (a sprite, listed in `icons`) shows full while it can be
/// cast and dimmer while it cannot; a cooldown sweeps back clockwise over it; an empty slot counts down to its next
/// skill; the ultimate's frame fills with rage; a skill with uses stored shows how many.
pub fn skills(
    base: &mut Pen,
    over: &mut Pen,
    view: &DuelView,
    g: &Geo,
    seen: &mut [Seen; 2],
    icons: &mut Vec<Icon>,
) {
    let n = g.icon;
    let size = n + 4;
    let top = g.bottom - size;
    let time = view.time as f32;
    let (deep, field, shade, white) = (
        hex(BRONZE_DEEP),
        hex(SLOT_FIELD),
        hex(SLOT_SHADE),
        hex(WHITE),
    );
    for f in &view.fighters {
        let c = colors(f.look);
        let s = &mut seen[f.index.min(1)];
        s.slots(f.skill_bar.len());
        for (i, slot) in f.skill_bar.iter().enumerate() {
            let [x, ..] = g.slot(f.index, i);
            if s.left[i] > 0.0 && slot.left == 0.0 {
                s.at[i] = view.time;
            }
            s.left[i] = slot.left;
            let pulse = ops::sin(time * 6.0) > 0.0;
            let _ = pulse;
            // the frame: bronze
            base.rect(x, top, size, size, deep, 1.0);
            base.frame(x + 1, top + 1, size - 2, hex(BRONZE_MID), 1.0);
            base.rect(x + 1, top + 1, size - 2, 1, hex(BRONZE_LIGHT), 1.0);
            base.rect(x + 2, top + 2, n, n, field, 1.0);
            let cell = slot.spell.and_then(icon_cell).filter(|_| !slot.empty);
            let Some(cell) = cell else {
                // the wait for the next skill: its frame lights clockwise, and the seconds left stand in the middle
                if slot.full > 0.0 {
                    ring2(
                        base,
                        x,
                        top,
                        size,
                        (1.0 - slot.left / slot.full) as f32,
                        c.glow,
                    );
                    let z = if n >= 16 { 2 } else { 1 };
                    let d = (ops::ceil(slot.left.max(0.0) as f32) as usize).min(9);
                    base.glyph(
                        &DIGIT[d],
                        x + 2 + (n - 3 * z).div_euclid(2),
                        top + 2 + (n - 5 * z).div_euclid(2),
                        hex(BRONZE_LIGHT),
                        z,
                        1.0,
                    );
                }
                continue;
            };
            if slot.ultimate {
                // rage fills the frame from the foot up, and blazes once full
                let blaze = slot.ready && ops::sin(time * 12.0) > 0.0;
                let col = if blaze { white } else { c.glow };
                let lit = round(size as f32 * slot.charge as f32).clamp(0, size);
                for py in size - lit..size {
                    base.rect(x, top + py, 2, 1, col, 1.0);
                    base.rect(x + size - 2, top + py, 2, 1, col, 1.0);
                }
                if lit >= size {
                    base.rect(x, top, size, 2, col, 1.0);
                }
                if lit > size - 2 {
                    base.rect(x, top + size - 2, size, 2, col, 1.0);
                }
                base.rect(x + 2, top + 2, n, n, field, 1.0);
            }
            // an icon that cannot be cast now (short of qi, silenced) is dimmer; a skill about to leave the bar
            // unused flickers through its last stretch
            let leaving = slot.stale > STALE_FLICKER && (ops::floor(time * 6.0) as i64) % 2 == 1;
            let alpha = if slot.ready || slot.left > 0.0 {
                1.0
            } else {
                0.6
            } * if leaving { 0.35 } else { 1.0 };
            icons.push(Icon {
                cell,
                x: x + 2,
                y: top + 2,
                n,
                alpha,
            });
            if slot.ultimate && !slot.ready && slot.left == 0.0 {
                over.rect(x + 2, top + 2, n, n, shade, 0.35);
            }
            // uses stored beyond one: their count in the top corner
            if slot.max_uses > 1 {
                let d = usize::from(slot.uses).min(9);
                let gx = if f.index == 0 { x + 2 } else { x + 2 + n - 4 };
                over.rect(gx - 1, top + 1, 5, 7, shade, 0.75);
                over.glyph(&DIGIT[d], gx, top + 2, white, 1, 1.0);
            }
            // the cooldown: the share still to run stays dark, sweeping clockwise from twelve o'clock
            if slot.left > 0.0 && slot.full > 0.0 {
                sweep(
                    over,
                    x + 2,
                    top + 2,
                    n,
                    -(slot.left / slot.full) as f32,
                    shade,
                    0.6,
                );
                let since = (slot.full - slot.left) as f32;
                if since < GLINT {
                    over.rect(x + 2, top + 2, n, n, white, 0.6 * (1.0 - since / GLINT));
                }
            }
            let ago = (view.time - s.at[i]) as f32;
            if (0.0..GLINT).contains(&ago) && slot.ready {
                over.frame(x, top, size, white, 0.8 * (1.0 - ago / GLINT));
            }
        }
    }
}
