//! Bagua mirror, the shield form: a large octagonal bronze mirror ringed with the eight trigrams round a jade face
//! stands behind the fighter's back. It spins up out of a point of jade light; while it holds it sways, lights its
//! trigrams one after another clockwise and sends ripple rings out on the floor under it; a blow lands on its right
//! edge: the face flashes white, the mirror is knocked away, trembles and squashes, and the blow goes back the way
//! it came as a jade-white beam; at the end it spins faster, shrinks to a point and winks out in shards. One-shot,
//! 3.6 s. The mirror is behind the fighter, what leaves it in front.
//!
//! The engine sizes the mirror from the sprite it wraps and runs a jade line round the sprite's outline; here the
//! body is a fighter's height standing on the origin (so the mirror is sized as on a fighter) and the outline is
//! left out (only its bloom as the blow lands is kept). Its cues and its `hitX`/`hitY` options are dropped (the duel
//! passes neither): the blow lands where their defaults put it.

use core::f64::consts::{FRAC_1_SQRT_2, FRAC_PI_4, PI, TAU};

use crate::kit::halo::Halo;
use crate::kit::{Layer, Opts, Painter, Rgba, Show, clamp, libm, rand, smooth};

const DARK: Rgba = Rgba::hex24(0x1d1e16);
const BAND: Rgba = Rgba::hex24(0x2e3124);
const GOLD_HI: Rgba = Rgba::hex24(0xf2d769);
const GOLD: Rgba = Rgba::hex24(0xe3b440);
const BRONZE: Rgba = Rgba::hex24(0xbd8b2d);
const BRONZE_LO: Rgba = Rgba::hex24(0xa97522);
const SIDE: Rgba = Rgba::hex24(0x6d572a);
const BAR: Rgba = Rgba::hex24(0xf9eeaf);
const BAR_LO: Rgba = Rgba::hex24(0xd4cb99);
const BAR_GAP: Rgba = Rgba::hex24(0x8d6a2a);
const TEAL: Rgba = Rgba::hex24(0x295f57);
const TEAL_HI: Rgba = Rgba::hex24(0x1b7171);
const JADE_LO: Rgba = Rgba::hex24(0x3ba8ad);
const JADE: Rgba = Rgba::hex24(0x71e7dc);
const JADE_HI: Rgba = Rgba::hex24(0xaef8f3);
const WHITE: Rgba = Rgba::hex24(0xf6fdf7);

/// The glow's steps, from its dithered edge to white.
const RAMP: [Rgba; 4] = [Rgba::hex24(0x21878b), JADE, JADE_HI, WHITE];
/// The light the jade adds.
const GLOW: Rgba = Rgba::hex24(0x4fd8cc);
/// Heat lost a pixel out from a stroke.
const FALL: f32 = 1.0;
const SMALL: f64 = 12.0;
const TINY: f64 = 6.0;
const LAYOUT: f64 = 18.0;
/// The eight trigrams round the mirror, clockwise from the top: a bit per line, set for an unbroken line.
const TRIGRAMS: [u8; 8] = [0b111, 0b011, 0b010, 0b001, 0b000, 0b100, 0b101, 0b110];

/// The body it stands behind: a fighter's height on the origin.
const TOP: f64 = -61.0;
const BOTTOM: f64 = 0.0;
const SIZE: f64 = 0.7;
const R_MIN: f64 = 14.0;
const R_MAX: f64 = 34.0;
const HIGH: f64 = 0.45;
const FOOT: f64 = 2.0;
const POINT: f64 = 0.1;
const GROWN: f64 = 0.55;
const HIT: f64 = 1.6;
const KNOCK: f64 = 5.0;
const TREMOR: f64 = 2.0;
const SQUASH: f64 = 0.15;
const KNOCK_T: f64 = 0.6;
const BREAK: f64 = 2.9;
const SHRINK: f64 = 0.45;
const SWAY: f64 = 0.3;
const SWAY_T: f64 = 2.6;
const LIGHT: f64 = 0.12;
const RIPPLE: f64 = 1.1;
const BEAM_SPEED: f64 = 320.0;
const BEAM_LEN: f64 = 30.0;
/// Where the blow lands, local pixels: the mirror's right edge.
const HIT_X: f64 = 30.0;
const HIT_Y: f64 = -28.0;

/// The mirror's halo box (behind), and the beam's (in front: wide, so the beam can leave).
const W: usize = 96;
const H: usize = 96;
const OX: f64 = 48.0;
const OY: f64 = 82.0;
const BEAM_W: usize = 512;
const BEAM_OX: f64 = 256.0;

struct State {
    seed: f64,
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(State { seed: o.seed })
}

/// How the mirror looks at one moment.
#[derive(Clone, Copy)]
struct Look {
    x: f64,
    y: f64,
    r: f64,
    turn: f64,
    flash: f64,
    lit: usize,
    lit_k: f64,
    swirl: f64,
}

impl Show for State {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if t > BREAK + SHRINK + 0.75 {
            return;
        }
        let m = self.look(t);
        let dir = dir();
        let since = t - HIT;
        let (bx, by) = (m.x + dir * m.r * libm::fabs(libm::cos(m.turn)), hit_row());
        match layer {
            Layer::Back => {
                let mut halo = Halo::new(W, H, OX, OY);
                let px = mirror(&mut halo, &m);
                halo.spread(FALL);
                paint_halo(p, &halo);
                ripples(p, t);
                put(p, &px);
                light(p, &m);
            }
            Layer::Front => {
                if (0.0..0.6).contains(&since) {
                    let mut halo = Halo::new(BEAM_W, H, BEAM_OX, OY);
                    let px = beam(&mut halo, bx, by, dir, since - 0.04);
                    halo.spread(FALL);
                    paint_halo(p, &halo);
                    put(p, &px);
                }
                if (0.0..0.3).contains(&since) {
                    p.glow(bx, by, 14.0, WHITE, 0.9 * (1.0 - since / 0.3));
                }
                // the bloom over the body as the blow lands (the engine's outline flare)
                let flare = if since >= 0.0 {
                    clamp(1.0 - (since - 0.06).max(0.0) / 0.3, 0.0, 1.0)
                } else {
                    0.0
                };
                if flare > 0.0 {
                    let h = BOTTOM - TOP;
                    p.glow(0.0, (TOP + BOTTOM) * 0.5, h * 0.7, GLOW, 0.35 * flare);
                }
                self.point(p, t);
                shards(p, bx, by, since, 10, self.seed, 0.45, dir);
                let (cx, cy) = centre();
                shards(
                    p,
                    cx,
                    cy,
                    t - BREAK - SHRINK * 0.6,
                    16,
                    self.seed + 7.0,
                    0.7,
                    0.0,
                );
            }
        }
    }
}

/// The mirror's radius on a fighter.
fn radius() -> f64 {
    clamp(libm::floor((BOTTOM - TOP) * SIZE + 0.5), R_MIN, R_MAX)
}

/// The mirror's centre at rest: behind the body's middle, its foot near the ground.
fn centre() -> (f64, f64) {
    let y = (TOP + (BOTTOM - TOP) * HIGH).min(BOTTOM + FOOT - radius());
    (0.0, libm::floor(y))
}

/// The row the blow lands on, kept on the mirror's face.
fn hit_row() -> f64 {
    let (_, cy) = centre();
    let r = radius();
    clamp(HIT_Y, cy - r + 3.0, cy + r - 3.0)
}

/// The side the blow comes from: 1 right, -1 left.
fn dir() -> f64 {
    if HIT_X >= centre().0 { 1.0 } else { -1.0 }
}

/// How the mirror takes a blow `since` seconds ago: knocked away, springing back, trembling, squashed.
fn knock(since: f64, dir: f64) -> (f64, f64, f64) {
    if !(0.0..KNOCK_T).contains(&since) {
        return (0.0, 0.0, 1.0);
    }
    let fade = |rate: f64| libm::exp(-rate * since);
    let x = libm::floor(-dir * KNOCK * fade(8.0) * libm::cos(22.0 * since) + 0.5);
    let frame = libm::floor(since * 30.0);
    let shiver = libm::floor(TREMOR * fade(9.0) + 0.5);
    let y = if frame.rem_euclid(2.0) == 0.0 {
        shiver
    } else {
        -shiver
    };
    let scale = 1.0 - SQUASH * fade(14.0) * libm::cos(25.0 * since);
    (x, y, scale)
}

impl State {
    /// The mirror at `t`: grown, turned, lit, flashing, jolted.
    fn look(&self, t: f64) -> Look {
        let grow = smooth(POINT, GROWN, t);
        let shrink = 1.0 - smooth(0.0, SHRINK, t - BREAK);
        let s = grow * shrink;
        let held = smooth(GROWN, GROWN + 0.3, t);
        let sway = SWAY * libm::sin(TAU * (t - GROWN) / SWAY_T) * held;
        let spin_in = (1.0 - grow) * 3.0 * PI;
        let b = (t - BREAK).max(0.0);
        let spin_out = b * b * 40.0;
        let since = t - HIT;
        let (mut jolt, mut flash) = (0.0, 0.0);
        let hit = knock(since, dir());
        if since >= 0.0 {
            jolt = -dir() * 0.6 * libm::sin(25.0 * since) * libm::exp(-7.0 * since);
            flash = clamp(1.0 - since / 0.25, 0.0, 1.0);
        }
        let bob = libm::floor(libm::sin(TAU * t / 1.6) + 0.5) * held;
        let (cx, cy) = centre();
        let (lit, lit_k) = if t > GROWN && t < BREAK {
            (
                (libm::floor((t - GROWN) / LIGHT) as i64).rem_euclid(8) as usize,
                1.0,
            )
        } else {
            (0, 0.0)
        };
        Look {
            x: cx + hit.0,
            y: cy + bob + hit.1,
            r: radius() * s * hit.2,
            turn: sway + spin_in + spin_out + jolt,
            flash,
            lit,
            lit_k,
            swirl: t * 2.0,
        }
    }

    /// The point of light it grows from and winks out to.
    fn point(&self, p: &mut Painter, t: f64) {
        let start = clamp(1.0 - (t - POINT) / 0.15, 0.0, 1.0) * clamp(t / 0.04, 0.0, 1.0);
        let end = t - BREAK - SHRINK;
        let wink = if (-0.1..0.12).contains(&end) {
            1.0 - libm::fabs(end) / 0.12
        } else {
            0.0
        };
        let k = start.max(wink);
        if k > 0.0 {
            let (cx, cy) = centre();
            star(p, cx, cy, 2, k);
            p.glow(cx, cy, 8.0, GLOW, 0.8 * k);
        }
    }
}

/// The flat ripple rings on the ground under it: one as it forms, then one each `RIPPLE` seconds while it holds.
fn ripples(p: &mut Painter, t: f64) {
    let (cx, _) = centre();
    let (y, max) = (BOTTOM + 1.0, radius() * 1.3);
    if t < BREAK {
        ripple(p, cx, y, (t - 0.2) / 0.8, max);
    }
    if t > GROWN && t < BREAK + 0.3 {
        ripple(p, cx, y, ((t - GROWN) % RIPPLE) / RIPPLE, max * 0.9);
    }
}

/// The faint additive bloom over the jade face, brighter as it flashes.
fn light(p: &mut Painter, m: &Look) {
    let k = clamp(m.r / radius(), 0.0, 1.0) * libm::fabs(libm::cos(m.turn));
    if k > 0.0 {
        p.glow(m.x, m.y, m.r * 1.4, GLOW, (0.25 + 0.8 * m.flash) * k);
    }
}

/// The mirror's pixels, and its edge stamped into `halo` so its glow can be painted under it.
fn mirror(halo: &mut Halo, m: &Look) -> Vec<(f64, f64, Rgba)> {
    let mut out = Vec::new();
    if m.r < 0.6 {
        return out;
    }
    let (cx, cy) = (libm::floor(m.x), libm::floor(m.y));
    let u = libm::cos(m.turn);
    let side = libm::sin(m.turn);
    let heat = (2.2 + 1.4 * m.flash) as f32;
    let reach = libm::ceil(m.r) + 3.0;
    let mut y = -reach;
    while y <= reach {
        let mut x = -reach;
        while x <= reach {
            if let Some(c) = pixel(m, x, y, u, side) {
                out.push((cx + x, cy + y, c));
                if edge(m, x, y, u) {
                    halo.stamp(cx + x, cy + y, heat, 1);
                }
            }
            x += 1.0;
        }
        y += 1.0;
    }
    out
}

/// The octagon's distance of a point, flat sides up, down and to the sides.
fn d8(x: f64, y: f64) -> f64 {
    libm::fabs(x)
        .max(libm::fabs(y))
        .max((libm::fabs(x) + libm::fabs(y)) * FRAC_1_SQRT_2)
}

fn edge(m: &Look, x: f64, y: f64, u: f64) -> bool {
    if libm::fabs(u) < 0.22 {
        return true;
    }
    let (lx, ly) = ((x + 0.5) / libm::fabs(u), y + 0.5);
    m.r - d8(lx, ly) < 1.0
}

/// The colour of pixel (`x`, `y`) from the mirror's centre, if the mirror covers it.
fn pixel(m: &Look, x: f64, y: f64, u: f64, side: f64) -> Option<Rgba> {
    let r = m.r;
    let au = libm::fabs(u);
    if au < 0.22 {
        // edge-on: a bronze bar three pixels thick, lit on its left
        let h = libm::floor(r);
        if y < -h || y >= h || !(-1.0..=1.0).contains(&x) {
            return None;
        }
        return Some(match x as i32 {
            -1 => GOLD_HI,
            0 => GOLD,
            _ => BRONZE_LO,
        });
    }
    let (lx, ly) = ((x + 0.5) / au, y + 0.5);
    let d = d8(lx, ly);
    if d > r {
        // the rim's thickness shows on the side turned away
        let thick = libm::floor(libm::fabs(side) * r * 0.16 + 0.5);
        let dir = if side * u > 0.0 { 1.0 } else { -1.0 };
        for k in 1..=thick as i32 {
            let sx = (x - dir * f64::from(k) + 0.5) / au;
            if d8(sx, ly) <= r {
                return Some(if ly < 0.0 { BRONZE } else { SIDE });
            }
        }
        return None;
    }
    let lit = lx + ly < 0.0;
    let e = libm::hypot(lx, ly);
    if r < TINY {
        return Some(match libm::floor(r - d) as i32 {
            0 => DARK,
            1 => {
                if lit {
                    GOLD_HI
                } else {
                    GOLD
                }
            }
            _ => face_px(m, lx, ly, e / (r - 1.5).max(1.0), x, y),
        });
    }
    let small = r < SMALL;
    let unit = if small { 1.0 } else { (r / LAYOUT).max(1.0) };
    let depth = libm::floor((r - d) / unit);
    let face = if small { r - 5.0 } else { r - 11.5 * unit };
    if e < face + 2.0 * unit {
        if e >= face + unit {
            return Some(if lit { GOLD_HI } else { GOLD });
        }
        if e >= face && !small {
            return Some(if lit { TEAL_HI } else { TEAL });
        }
        return Some(face_px(m, lx, ly, e / face.max(1.0), x, y));
    }
    let c = match depth as i32 {
        0 => DARK,
        1 => {
            if lit {
                GOLD_HI
            } else {
                BRONZE
            }
        }
        2 if !small => {
            if lit {
                GOLD
            } else {
                BRONZE_LO
            }
        }
        _ => {
            let band = if small { 2.0 } else { 3.0 };
            bar(m, lx, ly, depth - band, small).unwrap_or(BAND)
        }
    };
    Some(c)
}

/// A trigram bar's pixel at `step` pixels in from the band's outer edge, if one lies there.
fn bar(m: &Look, lx: f64, ly: f64, step: f64, small: bool) -> Option<Rgba> {
    let a = libm::atan2(ly, lx) + PI / 2.0;
    let k = (libm::floor(a / FRAC_PI_4 + 0.5) as i32).rem_euclid(8) as usize;
    let na = k as f64 * FRAC_PI_4 - PI / 2.0;
    let along = -lx * libm::sin(na) + ly * libm::cos(na);
    let len = if small { 1.6 } else { m.r * 0.19 };
    if libm::fabs(along) > len {
        return None;
    }
    let line = if small {
        if step as i32 != 0 {
            return None;
        }
        0
    } else {
        match step as i32 {
            1 => 2,
            3 => 1,
            5 => 0,
            2 | 4 => return Some(BAR_GAP),
            _ => return None,
        }
    };
    let whole = (TRIGRAMS.get(k).copied().unwrap_or(0) >> line) & 1 == 1;
    if !small && !whole && libm::fabs(along) < 1.0 {
        return None;
    }
    if k == m.lit && m.lit_k > 0.5 {
        return Some(WHITE);
    }
    Some(if lx + ly < m.r * 0.6 { BAR } else { BAR_LO })
}

/// The jade face: a pale core, a three-armed swirl, a darker rim to the bottom right, white under a flash.
fn face_px(m: &Look, lx: f64, ly: f64, f: f64, x: f64, y: f64) -> Rgba {
    if m.flash > 0.6 || (m.flash > 0.3 && (x + y).rem_euclid(2.0) == 0.0) {
        return WHITE;
    }
    let lit = lx + ly < 0.0;
    if f > 0.82 {
        return if lit { JADE } else { JADE_LO };
    }
    if f < 0.3 {
        return WHITE;
    }
    let s = libm::sin(3.0 * libm::atan2(ly, lx) + 5.0 * f + m.swirl);
    if s > 0.45 {
        JADE_HI
    } else if lit {
        WHITE
    } else {
        JADE
    }
}

/// Paint a halo's heats as steps of the ramp: kind 0 only (the strokes paint themselves), the faint edge dithered.
fn paint_halo(p: &mut Painter, halo: &Halo) {
    for (x, y, h, kind) in halo.each() {
        if kind != 0 {
            continue;
        }
        let step = libm::floor(f64::from(h)) as usize;
        if step == 0 {
            if (x + y).rem_euclid(2.0) == 0.0 {
                p.dot(x, y, RAMP[0], 0.8);
            }
            continue;
        }
        p.dot(x, y, RAMP.get(step).copied().unwrap_or(WHITE), 1.0);
        p.light(x, y, GLOW, 0.12 * step as f64);
    }
}

fn put(p: &mut Painter, px: &[(f64, f64, Rgba)]) {
    for &(x, y, c) in px {
        p.px(x, y, c);
    }
}

/// The blow thrown back: a beam three pixels thick leaving (`x0`, `y0`) toward side `dir`, its core white.
fn beam(halo: &mut Halo, x0: f64, y0: f64, dir: f64, since: f64) -> Vec<(f64, f64, Rgba)> {
    let mut out = Vec::new();
    if !(0.0..0.6).contains(&since) {
        return out;
    }
    let head = since * BEAM_SPEED;
    let tail = (head - BEAM_LEN).max(0.0);
    let thick = since < 0.12;
    let (x0, y0) = (libm::floor(x0), libm::floor(y0));
    let mut d = libm::floor(tail);
    while d <= head {
        let x = x0 + dir * d;
        out.push((x, y0, WHITE));
        halo.stamp(x, y0, 2.6, 1);
        for e in [-1.0, 1.0] {
            if thick || d > tail + BEAM_LEN * 0.4 {
                out.push((x, y0 + e, JADE_HI));
                halo.stamp(x, y0 + e, 1.9, 1);
            }
        }
        d += 1.0;
    }
    out
}

/// A four-pointed spark at (`x`, `y`), `size` pixels from its middle to each point.
fn star(p: &mut Painter, x: f64, y: f64, size: i32, k: f64) {
    if k <= 0.0 {
        return;
    }
    let (x, y) = (libm::floor(x), libm::floor(y));
    p.dot(x, y, WHITE, k);
    for i in 1..=size {
        let c = if i == size { JADE } else { JADE_HI };
        let f = f64::from(i);
        for (dx, dy) in [(f, 0.0), (-f, 0.0), (0.0, f), (0.0, -f)] {
            p.dot(x + dx, y + dy, c, k);
        }
    }
    p.light(x, y, GLOW, 0.5 * k);
}

/// Bronze shards and jade sparks flung out from (`x0`, `y0`), falling; `dir` biases them to a side (0 all round).
#[allow(clippy::too_many_arguments)]
fn shards(p: &mut Painter, x0: f64, y0: f64, since: f64, n: usize, seed: f64, life: f64, dir: f64) {
    if since < 0.0 || since > life {
        return;
    }
    for i in 0..n {
        let fi = i as f64;
        let own = life * (0.6 + 0.4 * rand(&[fi, seed, 41.0]));
        if since > own {
            continue;
        }
        let ang = if dir == 0.0 {
            rand(&[fi, seed, 43.0]) * TAU
        } else {
            let base = if dir > 0.0 { 0.0 } else { PI };
            base + (rand(&[fi, seed, 43.0]) - 0.5) * 2.4
        };
        let speed = 30.0 + 45.0 * rand(&[fi, seed, 45.0]);
        let x = x0 + libm::cos(ang) * speed * since;
        let y = y0 + libm::sin(ang) * speed * since + 60.0 * since * since;
        let k = 1.0 - since / own;
        if i % 3 == 0 {
            star(p, x, y, 1, k);
        } else {
            let (x, y) = (libm::floor(x), libm::floor(y));
            let a = clamp(k * 1.5, 0.0, 1.0);
            p.dot(x, y, if i % 2 == 0 { GOLD_HI } else { GOLD }, a);
            p.dot(x + 1.0, y, BRONZE_LO, a);
            p.dot(x, y + 1.0, SIDE, a);
        }
    }
}

/// A flat ripple ring on the floor, `k` 0 to 1 through its life: it widens and fades.
fn ripple(p: &mut Painter, x: f64, y: f64, k: f64, max: f64) {
    if !(0.0..1.0).contains(&k) {
        return;
    }
    let rx = 4.0 + (max - 4.0) * k;
    let ry = (rx * 0.22).max(1.0);
    let n = (rx * 4.0) as i32;
    let a = 1.0 - k;
    for i in 0..n {
        let t = f64::from(i) / f64::from(n) * TAU;
        let (px, py) = (
            libm::floor(x + rx * libm::cos(t)),
            libm::floor(y + ry * libm::sin(t)),
        );
        let far = libm::sin(t) < 0.0;
        p.dot(
            px,
            py,
            if far { JADE } else { JADE_HI },
            a * if far { 0.5 } else { 1.0 },
        );
        p.light(px, py, GLOW, 0.2 * a);
    }
}
