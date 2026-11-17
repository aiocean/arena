//! What the blasts share: the engine's explosions `lib.rs` (stones, dirt in flight, the crater, the white-hot dome,
//! culling) and its tone painting (`tone.rs`: strokes painted as tones through a ramp, keyed poses).

use crate::vfx::kit::sim::{Bit, Kind, Particles};
use crate::vfx::kit::{
    Layer, Painter, Rgba, TAU, WHITE, clamp, js_max, js_min, js_round, libm, rand, scale, step_at,
};
use core::f64::consts::PI;

pub const OUTLINE: Rgba = Rgba::hex24(0x1a1422);

/// Stone colours: a lit face, the body, a shaded face, and its outline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stone {
    pub light: Rgba,
    pub body: Rgba,
    pub shade: Rgba,
    pub edge: Rgba,
}

/// A seed for the dice from the effect's seed.
pub fn seed_of(seed: f64) -> u32 {
    if seed.is_finite() && seed >= 0.0 {
        seed as u32
    } else {
        1
    }
}

/// Every particle, soft and hard (the engine's `Layer::Both`).
pub fn draw_both(p: &mut Painter, ps: &Particles) {
    ps.draw(p, Layer::Back);
    ps.draw(p, Layer::Front);
}

/// Drop particles that leave the box (local px), so nothing is drawn past it.
pub fn cull(ps: &mut Particles, x0: f64, y0: f64, x1: f64, y1: f64) {
    ps.list_mut()
        .retain(|q| !(q.x < x0 || q.x > x1 || q.y < y0 || q.y > y1));
}

/// Soft particles fade out over w px as they near the sides x0, x1, and go before they reach them.
pub fn fade_edges(ps: &mut Particles, x0: f64, x1: f64, w: f64, y1: f64) {
    ps.list_mut().retain_mut(|q| {
        if !matches!(q.kind, Kind::Smoke | Kind::Dust | Kind::Vapour) {
            return true;
        }
        let r = q.size * 1.65;
        let e = js_min((js_min(q.x - x0, x1 - q.x) - r) / w, (y1 - q.y - r) / 6.0);
        if e <= 0.0 {
            return false;
        }
        if e < 1.0 {
            q.life = js_min(q.life, q.max * 0.42 * e);
        }
        true
    });
}

/// A rock: a lit side, a shaded side and an outline, turning as it spins. `heat` makes it molten through `fr`.
pub fn rock(p: &mut Painter, b: &Bit, st: &Stone, fr: &[Rgba], heat: f64) {
    let r = 0.6 + b.s * 0.75;
    let fade = js_min(1.0, b.life / 0.5);
    let lx = libm::cos(b.a) * r * 0.4;
    let ly = libm::sin(b.a) * r * 0.4;
    let f = |i: usize| fr.get(i).copied().unwrap_or(Rgba::NONE);
    p.shade(
        b.x - r - 1.0,
        b.y - r - 1.0,
        b.x + r + 1.0,
        b.y + r + 1.0,
        |x, y, out| {
            let (dx, dy) = (x - b.x, y - b.y);
            let d = libm::hypot(dx * 1.15, dy);
            if d > r + 0.9 {
                return None;
            }
            out.a = fade;
            if d > r {
                return Some(st.edge);
            }
            let lit = dx * lx + dy * ly;
            if heat > 0.08 {
                let g = heat * (1.1 - d / (r + 0.5)) + if lit > 0.3 { heat * 0.45 } else { 0.0 };
                if g > 0.5 {
                    return Some(f(if g > 1.0 {
                        1
                    } else if g > 0.8 {
                        2
                    } else if g > 0.62 {
                        3
                    } else {
                        4
                    }));
                }
                if g > 0.32 {
                    return Some(f(5));
                }
            }
            Some(if lit < -0.4 {
                st.light
            } else if lit > 0.6 {
                st.shade
            } else {
                st.body
            })
        },
    );
    if heat > 0.05 {
        p.glow(b.x, b.y, r * 2.0 + 3.0, f(3), 0.32 * heat * fade);
    }
}

/// A particle system with its GRIT drawn as dirt in flight: fast bits streak, slow ones are 1 or 2 px chips.
pub fn draw_dirt(p: &mut Painter, ps: &Particles, layer: Layer) {
    ps.draw_where(p, layer, |q| q.kind != Kind::Grit);
    if layer == Layer::Back {
        return;
    }
    for q in ps.list() {
        if q.kind != Kind::Grit || q.delay > 0.0 {
            continue;
        }
        let k = q.life / q.max;
        let sp = libm::hypot(q.vx, q.vy);
        if k < 0.25 && k * 4.0 < 0.5 {
            continue;
        }
        if sp > 140.0 {
            let l = js_min(5.0, sp * 0.012);
            let (ux, uy) = (q.vx / sp, q.vy / sp);
            let tail = scale(q.col, 0.7);
            let col = q.col;
            p.line(
                q.x - ux * l,
                q.y - uy * l,
                q.x,
                q.y,
                move |u: f64| if u < 0.5 { tail } else { col },
                1.0,
            );
        } else {
            p.px(q.x, q.y, q.col);
            if q.size >= 2.0 {
                p.px(q.x + 1.0, q.y, q.col);
                p.px(q.x, q.y + 1.0, scale(q.col, 0.8));
                p.px(q.x + 1.0, q.y + 1.0, scale(q.col, 0.7));
            }
        }
    }
}

/// A crater in the floor at x: a dark pit with its far wall lit, a lip of heaped dirt, loose earth round it.
/// `e`: an earth ramp (dark to bright); r: half-width; k: 0..1 how far it has opened; a: alpha.
pub fn crater(p: &mut Painter, e: &[Rgba], x: f64, r: f64, k: f64, a: f64) {
    if k <= 0.0 || a <= 0.0 {
        return;
    }
    let n = e.len();
    let c = |i: usize| e.get(i).copied().unwrap_or(Rgba::NONE);
    let rx = r * k;
    let ry = js_max(1.5, r * 0.17 * k);
    let mut py = -1.0;
    while py <= libm::ceil(ry) + 1.0 {
        let mut px = libm::floor(-rx - 1.0);
        while px <= libm::ceil(rx + 1.0) {
            let dx = (px + 0.5) / rx;
            let dy = (py + 0.5 - ry * 0.35) / ry;
            let d = libm::sqrt(dx * dx + dy * dy);
            if d <= 1.08 {
                let col = if d > 0.92 {
                    if dy < 0.0 { c(n.wrapping_sub(2)) } else { c(2) }
                } else if dy < -0.2 {
                    c(0)
                } else if dy < 0.35 {
                    c(1)
                } else {
                    c(2)
                };
                p.dot(x + px, py, col, a);
            }
            px += 1.0;
        }
        py += 1.0;
    }
    for s in [-1.0, 1.0] {
        for i in 0..9 {
            let fi = f64::from(i);
            let u = fi / 8.0;
            let bump = if rand(&[fi, if s > 0.0 { 3.0 } else { 4.0 }]) < 0.4 {
                1.0
            } else {
                0.0
            };
            let h = js_round(libm::sin(PI * u) * 2.2 * k + bump * k);
            let hx = x + s * (rx * 0.82 + u * r * 0.55);
            let mut j = 0.0;
            while j <= h {
                let col = if j == h {
                    c(n.wrapping_sub(2))
                } else if j == h - 1.0 {
                    c(3)
                } else {
                    c(2)
                };
                p.dot(hx, -j, col, a);
                j += 1.0;
            }
        }
    }
    for i in 0..26 {
        let fi = f64::from(i);
        let s = if rand(&[fi, 7.0]) < 0.5 { -1.0 } else { 1.0 };
        let d = rx * (1.05 + rand(&[fi, 8.0]) * 1.4);
        let y = js_round(rand(&[fi, 9.0]) * 3.0) - 1.0;
        if rand(&[fi, 10.0]) < k {
            let col = if rand(&[fi, 11.0]) < 0.5 { c(3) } else { c(2) };
            p.dot(x + s * d, y, col, a);
        }
    }
}

/// A hemisphere of white-hot air on the floor at x, radius r: an orange rim, a hot body, a white core; k 1 to 0 as
/// it dies. `f` an 8-step fire ramp.
pub fn dome(p: &mut Painter, f: &[Rgba], x: f64, r: f64, k: f64) {
    if r <= 0.0 {
        return;
    }
    let c = |i: usize| f.get(i).copied().unwrap_or(Rgba::NONE);
    let e = 1.4 / r;
    p.shade(x - r - 1.5, -r - 1.5, x + r + 1.5, 0.5, |px, py, _| {
        let d = libm::hypot(px - x, py) / r;
        if d > 1.0 + e {
            return None;
        }
        Some(if d > 1.0 {
            c(2)
        } else if d > 0.8 {
            if k > 0.45 { c(6) } else { c(5) }
        } else if d > 0.5 {
            if k > 0.45 { WHITE } else { c(6) }
        } else if k > 0.2 {
            WHITE
        } else {
            c(6)
        })
    });
}

/// A soft pool of light on the floor, an ellipse rx by ry at (x, y).
pub fn pool(p: &mut Painter, x: f64, y: f64, rx: f64, ry: f64, c: Rgba, k: f64) {
    p.floor_light(x, y, rx, ry, c, k);
}

// ---------------------------------------------------------------------------------------------------------------
// tone painting (the engine's `tone.rs`): each pixel a tone, 0 its darkest edge to 15 its white core, through a ramp

/// The brightest step, as a tone.
pub const LAST: f64 = 15.0;

/// The colourway the engine's tone effects default to (its variant 0, ember: red to yellow).
pub const EMBER: [Rgba; 16] = [
    Rgba::hex24(0xae2d48),
    Rgba::hex24(0xb7354c),
    Rgba::hex24(0xc64253),
    Rgba::hex24(0xd54e5a),
    Rgba::hex24(0xe55c61),
    Rgba::hex24(0xeb6b65),
    Rgba::hex24(0xef7a69),
    Rgba::hex24(0xf2886d),
    Rgba::hex24(0xf69871),
    Rgba::hex24(0xfaa674),
    Rgba::hex24(0xfeb478),
    Rgba::hex24(0xfec27c),
    Rgba::hex24(0xfecd81),
    Rgba::hex24(0xfdd884),
    Rgba::hex24(0xfde388),
    Rgba::hex24(0xfcf08d),
];

/// The colour of tone `tone` (0..1) on `ramp`.
pub fn color(ramp: &[Rgba], tone: f64) -> Rgba {
    if tone.is_nan() {
        return Rgba::NONE;
    }
    let last = ramp.len().saturating_sub(1) as f64;
    step_at(ramp, js_round(clamp(tone, 0.0, 1.0) * last))
}

/// A point of a stroke: `(x, y, half-width, tone)`.
pub type Knot = (f64, f64, f64, f64);

struct Seg {
    a: (f64, f64),
    b: (f64, f64),
    r: (f64, f64),
    tone: (f64, f64),
    lo: (f64, f64),
    hi: (f64, f64),
}

/// Strokes in tone, gathered for a frame then painted pixel by pixel; where they overlap the brighter tone wins.
pub struct Ink {
    segs: Vec<Seg>,
    rim: f64,
    grain: (f64, f64, f64),
}

impl Ink {
    pub fn new() -> Self {
        Ink {
            segs: Vec::new(),
            rim: 0.0,
            grain: (0.0, 1.0, 0.0),
        }
    }

    /// Darken each stroke toward its edge.
    pub fn rim(mut self, rim: f64) -> Self {
        self.rim = rim;
        self
    }

    /// Speckle bright strokes: a share of the pixels at or past step 11 keep `keep` of their tone.
    pub fn grain(mut self, share: f64, keep: f64, seed: f64) -> Self {
        self.grain = (share, keep, seed);
        self
    }

    pub fn seg(&mut self, a: (f64, f64), b: (f64, f64), r: (f64, f64), tone: (f64, f64)) {
        if !(r.0 > 0.0 || r.1 > 0.0) {
            return;
        }
        let m = r.0.max(r.1);
        self.segs.push(Seg {
            a,
            b,
            r,
            tone,
            lo: (a.0.min(b.0) - m, a.1.min(b.1) - m),
            hi: (a.0.max(b.0) + m, a.1.max(b.1) + m),
        });
    }

    pub fn dot(&mut self, x: f64, y: f64, r: f64, tone: f64) {
        self.seg((x, y), (x, y), (r, r), (tone, tone));
    }

    /// An arc round `centre` from `a0` to `a1` (radians, clockwise on screen from the right), in ~3 px segments.
    pub fn arc(
        &mut self,
        centre: (f64, f64),
        radius: f64,
        a0: f64,
        a1: f64,
        r: (f64, f64),
        tone: (f64, f64),
    ) {
        let n = (libm::fabs(a1 - a0) * radius / 3.0).clamp(1.0, 64.0) as usize;
        let at = |k: usize| {
            let f = k as f64 / n as f64;
            let a = a0 + (a1 - a0) * f;
            (
                centre.0 + radius * libm::cos(a),
                centre.1 + radius * libm::sin(a),
                r.0 + (r.1 - r.0) * f,
                tone.0 + (tone.1 - tone.0) * f,
            )
        };
        for k in 0..n {
            let (p, q) = (at(k), at(k + 1));
            self.seg((p.0, p.1), (q.0, q.1), (p.2, q.2), (p.3, q.3));
        }
    }

    /// The brightest tone any stroke gives the point, grained.
    pub fn tone(&self, x: f64, y: f64) -> Option<f64> {
        let mut best: Option<(f64, f64, f64, usize)> = None;
        for (i, s) in self.segs.iter().enumerate() {
            if x < s.lo.0 || x > s.hi.0 || y < s.lo.1 || y > s.hi.1 {
                continue;
            }
            if let Some((v, along, across)) = s.tone_at(x, y, self.rim)
                && best.is_none_or(|b| v > b.0)
            {
                best = Some((v, along, across, i));
            }
        }
        let (v, along, across, i) = best?;
        let (share, keep, seed) = self.grain;
        let speck = v >= 11.0
            && share > 0.0
            && rand(&[i as f64, libm::floor(along), libm::floor(across), seed]) < share;
        Some(if speck { v * keep } else { v })
    }

    /// Paint every covered pixel of the square `[-half, half)` in its tone's colour, with light past step 10.
    pub fn paint(&self, p: &mut Painter, ramp: &[Rgba], half: f64, light: f64) {
        if self.segs.is_empty() {
            return;
        }
        p.shade(-half, -half, half - 1.0, half - 1.0, |x, y, o| {
            let v = self.tone(x, y)?;
            o.k = light * (v - 10.0).max(0.0);
            Some(color(ramp, v / LAST))
        });
    }
}

impl Seg {
    fn tone_at(&self, x: f64, y: f64, rim: f64) -> Option<(f64, f64, f64)> {
        let (dx, dy) = (self.b.0 - self.a.0, self.b.1 - self.a.1);
        let len2 = dx * dx + dy * dy;
        let h = if len2 > 0.0 {
            clamp(((x - self.a.0) * dx + (y - self.a.1) * dy) / len2, 0.0, 1.0)
        } else {
            0.0
        };
        let (cx, cy) = (self.a.0 + dx * h, self.a.1 + dy * h);
        let d = libm::sqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
        let r = self.r.0 + (self.r.1 - self.r.0) * h;
        if d > r {
            return None;
        }
        let v = self.tone.0 + (self.tone.1 - self.tone.0) * h;
        let (ux, uy) = if len2 > 0.0 {
            let len = libm::sqrt(len2);
            (dx / len, dy / len)
        } else {
            (1.0, 0.0)
        };
        let (ox, oy) = (x - self.a.0, y - self.a.1);
        let v = if r > 0.0 { v * (1.0 - rim * d / r) } else { v };
        Some((v, ox * ux + oy * uy, oy * ux - ox * uy))
    }
}

/// The pose at moment `s` of strokes keyed at whole moments, on smooth curves through the keys, held after the last.
pub fn pose<const N: usize>(keys: &[[Knot; N]], s: f64) -> [Knot; N] {
    let last = keys.len().saturating_sub(1);
    let s = clamp(s, 0.0, last as f64);
    let i = (libm::floor(s) as usize).min(last);
    let f = s - i as f64;
    let at = |k: usize| keys.get(k.min(last));
    let (Some(p), Some(a), Some(b), Some(q)) =
        (at(i.saturating_sub(1)), at(i), at(i + 1), at(i + 2))
    else {
        return [(0.0, 0.0, 0.0, 0.0); N];
    };
    let (first, end) = (i == 0, i + 1 >= last);
    let one = |p: f64, a: f64, b: f64, q: f64| {
        let m0 = if first { b - a } else { slope(a - p, b - a) };
        let m1 = if end { 0.0 } else { slope(b - a, q - b) };
        hermite(a, b, m0, m1, f)
    };
    core::array::from_fn(|n| {
        let (p, a, b, q) = (p[n], a[n], b[n], q[n]);
        (
            one(p.0, a.0, b.0, q.0),
            one(p.1, a.1, b.1, q.1),
            one(p.2, a.2, b.2, q.2),
            one(p.3, a.3, b.3, q.3),
        )
    })
}

fn slope(before: f64, after: f64) -> f64 {
    if before * after <= 0.0 {
        0.0
    } else {
        2.0 * before * after / (before + after)
    }
}

fn hermite(a: f64, b: f64, m0: f64, m1: f64, f: f64) -> f64 {
    let (f2, f3) = (f * f, f * f * f);
    (2.0 * f3 - 3.0 * f2 + 1.0) * a
        + (f3 - 2.0 * f2 + f) * m0
        + (3.0 * f2 - 2.0 * f3) * b
        + (f3 - f2) * m1
}

/// A value keyed at moments `(s, value)`, on a smooth curve through the keys, held past either end.
pub fn curve(keys: &[(f64, f64)], s: f64) -> f64 {
    let (Some(first), Some(last)) = (keys.first(), keys.last()) else {
        return 0.0;
    };
    if s <= first.0 {
        return first.1;
    }
    let span = |k: usize| match (keys.get(k), keys.get(k + 1)) {
        (Some(a), Some(b)) if b.0 > a.0 => (b.1 - a.1) / (b.0 - a.0),
        _ => 0.0,
    };
    for (k, w) in keys.windows(2).enumerate() {
        if let [a, b] = w
            && s <= b.0
        {
            let width = b.0 - a.0;
            if width <= 0.0 {
                return b.1;
            }
            let m0 = if k == 0 {
                0.0
            } else {
                slope(span(k - 1), span(k))
            };
            let m1 = if k + 2 >= keys.len() {
                0.0
            } else {
                slope(span(k), span(k + 1))
            };
            return hermite(a.1, b.1, m0 * width, m1 * width, (s - a.0) / width);
        }
    }
    last.1
}

/// Degrees to radians.
pub const DEG: f64 = TAU / 360.0;
