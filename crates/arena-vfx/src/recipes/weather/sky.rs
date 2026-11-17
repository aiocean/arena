//! What the weather shares (the engine's `weather/lib.rs`): a weather effect fills a canvas its `width` and `height`
//! ask for, its origin mid-width and 10 px above the canvas's foot (the ground at the foot, a taller canvas more sky),
//! and draws only inside it.

use crate::vfx::kit::{Opts, Painter, Rgba, js_round, libm, rand};

/// The canvas around the origin, local px: `x0 <= x < x1`, `y0 <= y < y1`.
#[derive(Clone, Copy, Debug)]
pub struct Cv {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
}

impl Cv {
    pub fn span(&self) -> f64 {
        self.x1 - self.x0
    }

    pub fn tall(&self) -> f64 {
        self.y1 - self.y0
    }
}

/// The canvas `width` x `height` (held to 1..=2048, whole pixels; the effect's own `own` when not given), 10 px of it
/// under the origin as on the effect's own 320 x 180 canvas.
pub fn sized(o: &Opts, own: (f64, f64)) -> Cv {
    let side = |v: f64| js_round(v.clamp(1.0, 2048.0));
    let (w, h) = (side(o.width_or(own.0)), side(o.height_or(own.1)));
    let ox = libm::floor(w / 2.0);
    let oy = h - 10.0;
    Cv {
        x0: -ox,
        y0: -oy,
        x1: w - ox,
        y1: 10.0,
    }
}

/// Draw only inside the canvas from here on.
pub fn clip(p: &mut Painter, c: Cv) {
    p.clip_box(c.x0, c.y0, c.x1, c.y1);
}

/// `n` things over `span` px, as dense as the effect's `n` over its own `own`.
pub fn count(n: usize, span: f64, own: f64) -> usize {
    js_round(n as f64 * span / own).max(0.0) as usize
}

/// Falling things for a loop of `d` s: `n` slots, each falling `m` whole times a loop (`m0..=m1`), each fall somewhere
/// new: `f(u, k, m, i)`, `u` 0..1 through the fall, `k` its number (`wrap(k, m)` names it).
pub fn trips(
    t: f64,
    d: f64,
    n: usize,
    seed: f64,
    m0: f64,
    m1: f64,
    mut f: impl FnMut(f64, f64, f64, f64),
) {
    for i in 0..n {
        let i = i as f64;
        let m = m0 + libm::floor(rand(&[seed, i, 72.0]) * (m1 - m0 + 1.0));
        let s = (t / d) * m + rand(&[seed, i, 71.0]);
        let k = libm::floor(s);
        f(s - k, k, m, i);
    }
}

/// What a [`pass`] sets for a pixel: colour (NONE for none), alpha (dithered, or blended when `b`), light colour (NONE:
/// the pixel's colour) and amount.
pub struct Out {
    pub c: Rgba,
    pub a: f64,
    pub b: bool,
    pub l: Rgba,
    pub k: f64,
}

/// A per-pixel shader over the canvas rows `y0..=y1` (cut to the canvas): `f(x, y, out)` for the pixel centred at
/// (x, y).
pub fn pass(p: &mut Painter, c: Cv, y0: f64, y1: f64, mut f: impl FnMut(f64, f64, &mut Out)) {
    let first = libm::floor(y0).max(c.y0);
    let last = libm::ceil(y1).min(c.y1 - 1.0);
    let mut y = first;
    while y <= last {
        let mut x = c.x0;
        while x < c.x1 {
            let mut o = Out {
                c: Rgba::NONE,
                a: 1.0,
                b: false,
                l: Rgba::NONE,
                k: 0.0,
            };
            f(x + 0.5, y + 0.5, &mut o);
            if o.c != Rgba::NONE {
                if o.b {
                    p.blend(x, y, o.c, o.a);
                } else {
                    p.dot(x, y, o.c, o.a);
                }
            }
            if o.k > 0.0 {
                p.light(x, y, if o.l != Rgba::NONE { o.l } else { o.c }, o.k);
            }
            x += 1.0;
        }
        y += 1.0;
    }
}
