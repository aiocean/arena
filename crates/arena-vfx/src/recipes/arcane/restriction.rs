//! Restriction: a seal of three rune rings round a fighter at knee, waist and head height. The runes write themselves
//! one at a time, ring by ring from the ground up, each flashing as it appears, the ribbon under them drawn behind
//! the pen; the rings spin against each other with a pulse running along them; a blow lands on the waist ring's
//! right side, flaring it from there round, jolting it and throwing rune sparks; then the rings unravel from the
//! top, rune by rune. The far half of each ring (its runes mirrored) and the pillars go behind the fighter, the near
//! half in front. The glow is painted in steps of the ramp (`Halo`), with a faint additive light. One-shot, 3.8 s.
//! The engine's cues and its `hitX`/`hitY` options are dropped (the duel passes neither): the blow lands where their
//! defaults put it.

use core::f64::consts::{PI, TAU};

use crate::kit::halo::Halo;
use crate::kit::{Layer, Opts, Painter, Rgba, Show, clamp, libm, rand};

/// The glow's steps, from its faint dithered edge to the white core.
const RAMP: [Rgba; 5] = [
    Rgba::hex24(0x3a2496),
    Rgba::hex24(0x6c48e0),
    Rgba::hex24(0xa888ff),
    Rgba::hex24(0xdcd0ff),
    Rgba::hex24(0xfdfcfe),
];
/// The specks scattered through the ribbons.
const SPECKS: [Rgba; 2] = [Rgba::hex24(0x8fd8ff), Rgba::hex24(0xff9ef0)];
const WHITE: Rgba = Rgba::hex24(0xffffff);
const GLOW: Rgba = Rgba::hex24(0x8c5cff);

/// The rings' heights (knee, waist, head), their half width and half depth, the ribbon's half height, runes a ring.
const RINGS: [f64; 3] = [-9.0, -29.0, -49.0];
const RX: f64 = 22.0;
const RY: f64 = 9.0;
const RIBBON: f64 = 2.0;
const SLOTS: usize = 16;
/// Turns a second of each ring: the middle one against the others.
const SPIN: [f64; 3] = [1.1, -1.4, 1.1];
const WRITE: f64 = 0.1;
const STROKE: f64 = 0.019;
const HIT: f64 = 1.8;
const BREAK: f64 = 2.8;
const UNDO_RING: f64 = 0.18;
const UNDO_RUNE: f64 = 0.015;
const PEEL: f64 = 0.4;
const SPARKS: usize = 14;
/// Where the blow lands, in local pixels: the right side of the waist ring.
const HIT_X: f64 = RX;
const HIT_Y: f64 = -29.0;

const RUNE: f32 = 2.9;
const EDGE: f32 = 2.3;
const FILL: f32 = 1.9;
const PILLAR: f32 = 2.9;
const FALL: f32 = 1.4;

/// The box the halos cover: its size and where the origin sits in it.
const W: usize = 96;
const H: usize = 96;
const OX: f64 = 48.0;
const OY: f64 = 82.0;

/// The runes, 3 by 5, a row each as bits from the left.
const GLYPHS: [[u8; 5]; 5] = [
    [0b010, 0b111, 0b010, 0b110, 0b010],
    [0b010, 0b101, 0b111, 0b101, 0b010],
    [0b110, 0b011, 0b100, 0b011, 0b101],
    [0b101, 0b010, 0b111, 0b010, 0b101],
    [0b111, 0b100, 0b110, 0b001, 0b110],
];

struct State {
    ramp: Vec<Rgba>,
    seed: f64,
    blow: Blow,
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(State {
        ramp: o.palette_or(&RAMP),
        seed: o.seed,
        blow: Blow::new(HIT_X, HIT_Y),
    })
}

/// Where the blow lands on the seal: the ring nearest it, the angle round that ring, and that point.
#[derive(Clone, Copy)]
struct Blow {
    ring: usize,
    angle: f64,
    x: f64,
    y: f64,
}

impl Blow {
    fn new(hx: f64, hy: f64) -> Self {
        let mut ring = 0;
        for (r, &cy) in RINGS.iter().enumerate() {
            if libm::fabs(cy - hy) < libm::fabs(RINGS[ring] - hy) {
                ring = r;
            }
        }
        let cy = RINGS.get(ring).copied().unwrap_or(RINGS[1]);
        let (u, v) = (hx / RX, (hy - cy) / RY);
        let angle = if libm::fabs(u) + libm::fabs(v) < 1e-6 {
            PI / 2.0
        } else {
            libm::atan2(v, u)
        };
        Blow {
            ring,
            angle,
            x: RX * libm::cos(angle),
            y: cy + RY * libm::sin(angle),
        }
    }
}

fn ring_y(r: usize) -> f64 {
    RINGS.get(r).copied().unwrap_or(0.0)
}

/// When rune `k` of ring `r` is written.
fn written(r: usize, k: usize) -> f64 {
    WRITE + (r * SLOTS + k) as f64 * STROKE
}

/// When rune `k` of ring `r` peels off: the top ring first.
fn peeled(r: usize, k: usize) -> f64 {
    BREAK + (RINGS.len() - 1 - r) as f64 * UNDO_RING + k as f64 * UNDO_RUNE
}

/// The jolt of the blow: the struck ring tilts, the others a little, dying out.
fn jolt(b: &Blow, r: usize, t: f64) -> f64 {
    let since = t - HIT;
    if since < 0.0 {
        return 0.0;
    }
    let k = if r == b.ring { 2.5 } else { 0.8 };
    -k * libm::sin(30.0 * since) * libm::exp(-6.0 * since)
}

/// A point of ring `r` at angle `a` (`PI / 2` nearest the viewer), and how near it is (-1 far, 1 near).
fn at(b: &Blow, r: usize, a: f64, t: f64) -> (f64, f64, f64) {
    let x = RX * libm::cos(a);
    let tilt = jolt(b, r, t) * libm::cos(a - b.angle);
    let y = ring_y(r) + RY * libm::sin(a) + libm::floor(tilt + 0.5);
    (x, y, libm::sin(a))
}

fn apart(a: f64, b: f64) -> f64 {
    let d = libm::fmod(libm::fabs(a - b), TAU);
    d.min(TAU - d)
}

fn turn(r: usize, t: f64) -> f64 {
    PI / 2.0 + SPIN.get(r).copied().unwrap_or(0.0) * t
}

impl Show for State {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if t > peeled(0, SLOTS - 1) + PEEL + 0.05 {
            return;
        }
        let back_layer = layer == Layer::Back;
        let mut back = if back_layer {
            Halo::new(W, H, OX, OY)
        } else {
            Halo::none()
        };
        let mut front = if back_layer {
            Halo::none()
        } else {
            Halo::new(W, H, OX, OY)
        };
        if back_layer {
            self.pillars(&mut back, t);
        }
        for r in 0..RINGS.len() {
            self.ribbon(&mut back, &mut front, r, t);
            for k in 0..SLOTS {
                self.rune(&mut back, &mut front, r, k, t);
            }
        }
        if back_layer {
            back.spread(FALL);
            self.paint(p, &back, t);
        } else {
            front.spread(FALL);
            self.paint(p, &front, t);
            self.light(p, t);
            self.sparks(p, t);
        }
    }
}

impl State {
    fn c(&self, i: usize) -> Rgba {
        self.ramp
            .get(i.min(self.ramp.len().saturating_sub(1)))
            .copied()
            .unwrap_or(Rgba::NONE)
    }

    /// Each pixel's heat as a step of the ramp: under 1 the dithered edge, then a colour a step.
    fn paint(&self, p: &mut Painter, halo: &Halo, t: f64) {
        let flicker = libm::floor(t * 10.0);
        for (x, y, h, kind) in halo.each() {
            let (gx, gy) = (x + OX, y + OY);
            let step = libm::floor(f64::from(h)) as usize;
            if step == 0 {
                if (gx + gy) % 2.0 == 0.0 {
                    p.dot(x, y, self.c(0), 0.85);
                }
                continue;
            }
            let mut c = match kind {
                2 => WHITE,
                3 => self.c(4),
                _ => self.c(step),
            };
            if kind == 1 && (2..4).contains(&step) {
                let r = rand(&[gx, gy, flicker, self.seed]);
                if r < 0.06 {
                    c = SPECKS[usize::from(r < 0.03)];
                }
            }
            p.dot(x, y, c, 1.0);
            if step >= 2 || kind >= 2 {
                p.light(x, y, GLOW, 0.14);
            }
        }
    }

    /// Ring `r`'s ribbon: two edges with a fill between, drawn behind the pen, taken away behind the peeling.
    fn ribbon(&self, back: &mut Halo, front: &mut Halo, r: usize, t: f64) {
        let from = clamp((t - peeled(r, 0)) / (SLOTS as f64 * UNDO_RUNE), 0.0, 1.0);
        let to = clamp((t - written(r, 0)) / (SLOTS as f64 * STROKE), 0.0, 1.0);
        if to <= from {
            return;
        }
        let steps = 200;
        for i in 0..steps {
            let u = f64::from(i) / f64::from(steps);
            if u < from || u > to {
                continue;
            }
            let a = turn(r, t) + (u - 0.5 / SLOTS as f64) * TAU;
            let (x, y, near) = at(&self.blow, r, a, t);
            let g = if near >= 0.0 { &mut *front } else { &mut *back };
            let dim = if near >= 0.0 { 0.0 } else { 0.5 };
            g.stamp(x, y - RIBBON, EDGE - dim, 1);
            g.stamp(x, y + RIBBON, EDGE - dim, 1);
            for dy in 1..RIBBON as i32 * 2 {
                g.stamp(x, y - RIBBON + f64::from(dy), FILL - dim, 1);
            }
        }
    }

    /// Rune `k` of ring `r`: written with a flash, pulsing while it holds, flaring under the blow, peeling off.
    fn rune(&self, back: &mut Halo, front: &mut Halo, r: usize, k: usize, t: f64) {
        let born = t - written(r, k);
        if born < 0.0 {
            return;
        }
        let gone = t - peeled(r, k);
        if gone > PEEL {
            return;
        }
        let a = turn(r, t) + k as f64 / SLOTS as f64 * TAU;
        let (mut x, mut y, near) = at(&self.blow, r, a, t);
        let mut fade = 1.0;
        if gone > 0.0 {
            let g = gone / PEEL;
            x += libm::cos(a) * 10.0 * g;
            y -= 14.0 * g + 4.0 * rand(&[r as f64, k as f64, self.seed]) * g;
            fade = 1.0 - g;
        }
        let flash = clamp(1.0 - born / 0.15, 0.0, 1.0);
        let head = (t * 9.0 + r as f64 * 4.0) % SLOTS as f64;
        let d = libm::fabs(k as f64 - head);
        let pulse = clamp(1.0 - d.min(SLOTS as f64 - d) / 1.5, 0.0, 1.0)
            * clamp((t - written(2, SLOTS - 1)) / 0.2, 0.0, 1.0);
        let since = t - HIT;
        let flare = if r == self.blow.ring && since >= 0.0 {
            let delay = apart(a, self.blow.angle) / PI * 0.12;
            if since >= delay {
                clamp(1.0 - (since - delay) / 0.35, 0.0, 1.0)
            } else {
                0.0
            }
        } else {
            0.0
        };
        let hot = flash.max(flare);
        let far = near < 0.0;
        let heat =
            ((f64::from(RUNE) + pulse + 2.0 * hot) * fade - if far { 0.5 } else { 0.0 }) as f32;
        let kind = if hot > 0.3 { 2 } else { 3 };
        let pick = (rand(&[r as f64, k as f64, self.seed, 7.0]) * GLYPHS.len() as f64) as usize
            % GLYPHS.len();
        let Some(glyph) = GLYPHS.get(pick) else {
            return;
        };
        let side = libm::fabs(libm::cos(a)) > 0.85;
        let (gx, gy) = (libm::floor(x), libm::floor(y) - 2.0);
        let g = if far { back } else { front };
        for (row, bits) in glyph.iter().enumerate() {
            for col in 0..3u8 {
                // seen from behind the rune is mirrored; seen edge-on only its middle stroke shows
                let bit = if far { col } else { 2 - col };
                if side && col != 1 {
                    continue;
                }
                if (bits >> bit) & 1 == 0 {
                    continue;
                }
                g.stamp(gx + f64::from(col) - 1.0, gy + row as f64, heat, kind);
            }
        }
    }

    /// The vertical strokes joining the rings once they are written, behind the fighter.
    fn pillars(&self, back: &mut Halo, t: f64) {
        let up = clamp((t - written(2, SLOTS - 1)) / 0.3, 0.0, 1.0);
        let down = clamp(1.0 - (t - BREAK) / 0.3, 0.0, 1.0);
        if up * down <= 0.0 {
            return;
        }
        let top = RINGS[2] + (RINGS[0] - RINGS[2]) * (1.0 - up);
        let bottom = RINGS[0] - (RINGS[0] - RINGS[2]) * (1.0 - down);
        for x in [-RX + 1.0, 0.0, RX - 1.0] {
            let mut y = top;
            while y <= bottom {
                back.stamp(x, y, PILLAR, 3);
                back.stamp(x + 1.0, y, PILLAR - 0.5, 1);
                y += 1.0;
            }
        }
    }

    /// The faint additive bloom over the seal while it stands.
    fn light(&self, p: &mut Painter, t: f64) {
        let up = clamp((t - written(2, SLOTS - 1)) / 0.3, 0.0, 1.0);
        let down = clamp(1.0 - (t - BREAK) / 0.4, 0.0, 1.0);
        let k = up * down;
        if k > 0.0 {
            p.glow(0.0, RINGS[1], 40.0, GLOW, 0.4 * k);
            for cy in RINGS {
                p.glow(0.0, cy, 28.0, GLOW, 0.1 * k);
            }
        }
    }

    /// Rune sparks thrown out where the blow lands.
    fn sparks(&self, p: &mut Painter, t: f64) {
        let since = t - HIT;
        if !(0.0..0.5).contains(&since) {
            return;
        }
        let (x0, y0) = (self.blow.x, self.blow.y);
        let side = libm::cos(self.blow.angle);
        let base = if side >= 0.0 { 0.0 } else { PI };
        let spread = 2.4 + 3.9 * (1.0 - libm::fabs(side));
        p.glow(x0, y0, 10.0, WHITE, 1.2 * (1.0 - since * 2.0));
        p.glow(x0, y0, 22.0, GLOW, 0.6 * (1.0 - since * 2.0));
        for i in 0..SPARKS {
            let fi = i as f64;
            let life = 0.3 + 0.2 * rand(&[fi, self.seed, 31.0]);
            if since > life {
                continue;
            }
            let ang = base + (rand(&[fi, self.seed, 33.0]) - 0.5) * spread;
            let speed = 40.0 + 50.0 * rand(&[fi, self.seed, 35.0]);
            let x = x0 + libm::cos(ang) * speed * since;
            let y = y0 + libm::sin(ang) * speed * since + 40.0 * since * since;
            let k = 1.0 - since / life;
            let c = if i % 3 == 0 { WHITE } else { self.c(3) };
            p.dot(libm::floor(x), libm::floor(y), c, k);
            p.light(x, y, GLOW, 0.9 * k);
        }
    }
}
