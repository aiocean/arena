//! Tone painting (the engine's `tone`, the parts the stroke marks use): each pixel a tone, 0 (its darkest edge) to
//! 15 (its white core), turned to colour by a ramp; strokes keyed at whole moments on smooth curves.

use crate::kit::{Painter, Rgba, TAU, clamp, js_round, libm, rand, step_at};

/// The brightest step, as a tone (a ramp of 16 steps).
const LAST: f64 = 15.0;

/// The default colourway (the engine's variant 0, ember: red to yellow), dark to bright.
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

/// The colour of tone `tone` (0..1, clamped) on `ramp`; nothing for NaN.
fn color(ramp: &[Rgba], tone: f64) -> Rgba {
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

/// Strokes in tone, gathered for a frame and painted pixel by pixel; the brighter tone wins where they overlap;
/// `fold` repeats every stroke round the origin.
pub struct Ink {
    segs: Vec<Seg>,
    fold: u32,
    grain: (f64, f64, f64),
}

impl Ink {
    pub fn new(fold: u32) -> Self {
        Ink {
            segs: Vec::new(),
            fold: fold.max(1),
            grain: (0.0, 1.0, 0.0),
        }
    }

    /// Speckle bright strokes: a share `share` of the pixels at or past step 11 take `keep` of their tone.
    pub fn grain(mut self, share: f64, keep: f64, seed: f64) -> Self {
        self.grain = (share, keep, seed);
        self
    }

    fn seg(&mut self, a: (f64, f64), b: (f64, f64), r: (f64, f64), tone: (f64, f64)) {
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

    /// A path through knots, a segment between each pair.
    pub fn path(&mut self, pts: &[Knot]) {
        for w in pts.windows(2) {
            if let [a, b] = w {
                self.seg((a.0, a.1), (b.0, b.1), (a.2, b.2), (a.3, b.3));
            }
        }
    }

    fn tone(&self, x: f64, y: f64) -> Option<f64> {
        let mut best: Option<(f64, f64, f64, usize)> = None;
        for k in 0..self.fold {
            let (px, py) = turn(x, y, self.fold, k);
            for (i, s) in self.segs.iter().enumerate() {
                if px < s.lo.0 || px > s.hi.0 || py < s.lo.1 || py > s.hi.1 {
                    continue;
                }
                if let Some((v, along, across)) = s.tone_at(px, py)
                    && best.is_none_or(|b| v > b.0)
                {
                    best = Some((v, along, across, i));
                }
            }
        }
        let (v, along, across, i) = best?;
        let (share, keep, seed) = self.grain;
        let speck = v >= 11.0
            && share > 0.0
            && rand(&[i as f64, libm::floor(along), libm::floor(across), seed]) < share;
        Some(if speck { v * keep } else { v })
    }

    /// Paint every covered pixel of the box `[-half, half)` round the origin in its tone's colour, with light on
    /// its own pixels: `light` times how far its tone is past step 10.
    pub fn paint(&self, p: &mut Painter, ramp: &[Rgba], half: f64, light: f64) {
        if self.segs.is_empty() {
            return;
        }
        // only the box the strokes (and their folds) can reach
        let mut reach: f64 = 0.0;
        for s in &self.segs {
            for (x, y) in [s.lo, s.hi, (s.lo.0, s.hi.1), (s.hi.0, s.lo.1)] {
                reach = reach.max(x.abs()).max(y.abs());
            }
        }
        let h = (reach + 1.0).min(half);
        p.shade(-h, -h, h - 1.0, h - 1.0, |x, y, o| {
            let v = self.tone(x, y)?;
            o.k = light * (v - 10.0).max(0.0);
            Some(color(ramp, v / LAST))
        });
    }
}

/// A point turned back by `k` of `fold` turns about the origin; quarter turns are exact.
fn turn(x: f64, y: f64, fold: u32, k: u32) -> (f64, f64) {
    if (4 * k).is_multiple_of(fold) {
        return match (4 * k / fold) % 4 {
            1 => (y, -x),
            2 => (-x, -y),
            3 => (-y, x),
            _ => (x, y),
        };
    }
    let a = -f64::from(k) * TAU / f64::from(fold);
    let (s, c) = (libm::sin(a), libm::cos(a));
    (x * c - y * s, x * s + y * c)
}

impl Seg {
    fn tone_at(&self, x: f64, y: f64) -> Option<(f64, f64, f64)> {
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
        Some((v, ox * ux + oy * uy, oy * ux - ox * uy))
    }
}

/// The pose at moment `s` of a stroke keyed at whole moments, held after the last key, each number on a smooth
/// curve through its keys.
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
        ease(a, b, m0, m1, f)
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

fn ease(a: f64, b: f64, m0: f64, m1: f64, f: f64) -> f64 {
    let (f2, f3) = (f * f, f * f * f);
    (2.0 * f3 - 3.0 * f2 + 1.0) * a
        + (f3 - 2.0 * f2 + f) * m0
        + (3.0 * f2 - 2.0 * f3) * b
        + (f3 - f2) * m1
}

/// A value keyed at moments `(s, value)`, on a smooth curve through the keys and held past either end.
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
            return ease(a.1, b.1, m0 * width, m1 * width, (s - a.0) / width);
        }
    }
    last.1
}
