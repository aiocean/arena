//! `weather/fireflies`, after the engine's (itself after Sprite Fusion's open-source fireflies,
//! https://www.spritefusion.com/pixel-art-vfx/fireflies, MIT, Copyright (c) 2026 Hugo Duprez): fireflies wander over a
//! meadow at night, each glowing up and fading on its own rhythm, close ones a dark body with beating wings and a
//! lantern, the grass blades swaying underneath and catching their light.
//!
//! The scene plays it as the stage's night, the width of the world, never stopped: all of it is drawn behind the
//! fighters. The engine's haze over the whole canvas (a flat tint at about half alpha) is left to the scene's backdrop,
//! which can carry it once instead of blending every pixel of the stage every frame.

use super::sky::{Cv, clip, count, sized};
use crate::vfx::kit::{
    Layer, Opts, Painter, Rgba, Show, TAU, WHITE, js_max, js_min, js_round, libm, loop_noise, mix,
    ramp, rand,
};

/// The effect's own canvas, which `width` and `height` default to.
const SIZE: (f64, f64) = (320.0, 180.0);
const D: f64 = 12.0;
/// Flies over the effect's own width.
const FLIES: usize = 32;

const PALETTE: [Rgba; 6] = [
    Rgba::hex24(0x2a3a10),
    Rgba::hex24(0x5a7a1a),
    Rgba::hex24(0x9ad02a),
    Rgba::hex24(0xd6ff5a),
    Rgba::hex24(0xf4ffb8),
    Rgba::hex24(0xffffff),
];
const GRASS: [Rgba; 6] = [
    Rgba::hex24(0x070f0b),
    Rgba::hex24(0x0c1a12),
    Rgba::hex24(0x13281a),
    Rgba::hex24(0x1d3a24),
    Rgba::hex24(0x2b5232),
    Rgba::hex24(0x3f6e40),
];

pub fn make(o: &Opts) -> Box<dyn Show> {
    let cv = sized(o, SIZE);
    Box::new(State {
        c: ramp(&o.palette_or(&PALETTE)),
        g: GRASS,
        s: o.seed,
        cv,
        flies: count(FLIES, cv.span(), SIZE.0),
    })
}

struct State {
    c: [Rgba; 6],
    g: [Rgba; 6],
    s: f64,
    cv: Cv,
    flies: usize,
}

/// `2 pi n t / D + ph`: `n` whole cycles a loop.
fn cyc(t: f64, n: f64, ph: f64) -> f64 {
    (TAU * n * t) / D + ph
}

/// A firefly's path and glow, both whole cycles a loop. `depth` 0 far .. 2 near.
struct Fly {
    i: f64,
    depth: u8,
    glow: f64,
    k: f64,
    cx: f64,
    cy: f64,
    ax: f64,
    ay: f64,
    n1: f64,
    n2: f64,
    ph1: f64,
    ph2: f64,
}

impl Fly {
    /// Fly `i` of `n`: the first 14 in 32 far, the next 12 mid, the rest near.
    fn new(s: f64, i: usize, n: usize, cv: Cv, t: f64) -> Fly {
        let depth: u8 = if i * FLIES < 14 * n {
            0
        } else if i * FLIES < 26 * n {
            1
        } else {
            2
        };
        let i = i as f64;
        let r = |n: f64| rand(&[s, i, n]);
        let k = [0.5, 0.8, 1.0][usize::from(depth)];
        let (left, span) = (cv.x0, cv.span());
        let cx = left + 10.0 + r(1.0) * (span - 20.0);
        let cy = -16.0 - r(2.0) * if depth == 2 { 70.0 } else { 130.0 };
        let ax = (14.0 + r(3.0) * 26.0) * k;
        let ay = (8.0 + r(4.0) * 14.0) * k;
        let n1 = 1.0 + libm::floor(r(5.0) * 2.0);
        let n2 = 1.0 + libm::floor(r(6.0) * 3.0);
        // its rhythm: a quick rise, a hold, a slow fade, dark; some flash twice
        let per = D / (3.0 + libm::floor(r(9.0) * 4.0));
        let u = (((t / per + r(10.0)) % 1.0) + 1.0) % 1.0;
        let two = r(11.0) < 0.35;
        let pulse = |v: f64| {
            if v < 0.0 {
                0.0
            } else if v < 0.06 {
                v / 0.06
            } else if v < 0.14 {
                1.0
            } else if v < 0.4 {
                1.0 - (v - 0.14) / 0.26
            } else {
                0.0
            }
        };
        let glow = js_max(pulse(u), if two { pulse(u - 0.22) * 0.8 } else { 0.0 });
        Fly {
            i,
            depth,
            glow,
            k,
            cx,
            cy,
            ax,
            ay,
            n1,
            n2,
            ph1: r(7.0) * TAU,
            ph2: r(8.0) * TAU,
        }
    }

    fn pos(&self, tt: f64) -> (f64, f64) {
        (
            self.cx
                + self.ax * libm::sin(cyc(tt, self.n1, self.ph1))
                + 10.0 * self.k * loop_noise(tt / D, 1.5, self.i * 3.0),
            self.cy
                + self.ay * libm::sin(cyc(tt, self.n2, self.ph2))
                + 6.0 * self.k * loop_noise(tt / D, 1.5, self.i * 3.0 + 1.0),
        )
    }
}

impl Show for State {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if layer != Layer::Back {
            return;
        }
        clip(p, self.cv);
        let flies: Vec<Fly> = (0..self.flies)
            .map(|i| Fly::new(self.s, i, self.flies, self.cv, t))
            .collect();
        // where the lit ones are, for the grass tips
        let lit: Vec<(f64, f64, f64)> = flies
            .iter()
            .filter(|f| f.glow > 0.25 && f.depth > 0)
            .map(|f| {
                let (x, y) = f.pos(t);
                (x, y, f.glow)
            })
            .collect();
        grass(p, self, t, &lit, false);
        for f in flies.iter().filter(|f| f.depth < 2) {
            firefly(p, t, f, &self.c);
        }
        grass(p, self, t, &lit, true);
        for f in flies.iter().filter(|f| f.depth == 2) {
            firefly(p, t, f, &self.c);
        }
        p.unclip();
    }
}

fn firefly(q: &mut Painter, t: f64, f: &Fly, c: &[Rgba; 6]) {
    let (x, y) = f.pos(t);
    let g = f.glow;
    let (xi, yi) = (js_round(x), js_round(y));
    if f.depth == 0 {
        if g < 0.08 {
            return;
        }
        q.dot(
            xi,
            yi,
            if g > 0.6 { c[4] } else { c[3] },
            js_min(1.0, g * 1.6),
        );
        q.glow(x, y, 6.0, c[2], 0.45 * g);
        return;
    }
    // a short trail of light where it has just been, while it's lit
    if g > 0.3 {
        for j in 1..=4 {
            let j = f64::from(j);
            let (tx, ty) = f.pos(t - j * 0.05);
            q.dot(tx, ty, c[2], (g - 0.3) * (1.0 - j / 5.0) * 1.3);
        }
    }
    if f.depth == 1 {
        q.px(
            xi,
            yi,
            if g > 0.5 {
                c[5]
            } else if g > 0.15 {
                c[3]
            } else {
                c[1]
            },
        );
        if g > 0.4 {
            q.px(xi, yi + 1.0, c[2]);
        }
        q.glow(x, y, 9.0, c[2], 0.5 * g);
        q.glow(x, y, 3.0, c[4], 0.5 * g);
        return;
    }
    // a close one: a dark body, wings a blur above it, a lantern at its tail; a glint at the peak
    let (nx, _) = f.pos(t + 0.05);
    let dir = if nx >= x { 1.0 } else { -1.0 };
    let beat = libm::floor(t * 30.0) % 2.0;
    q.px(xi + dir, yi, c[0]);
    q.px(xi + dir * 2.0, yi, c[0]);
    let wing = mix(c[1], WHITE, 0.4);
    q.dot(xi, yi - 1.0 - beat, wing, 0.7);
    q.dot(xi + dir, yi - 1.0 - beat, wing, 0.7);
    q.px(xi, yi, if g > 0.3 { c[5] } else { c[1] });
    q.px(
        xi - dir,
        yi,
        if g > 0.5 {
            c[4]
        } else if g > 0.15 {
            c[3]
        } else {
            c[1]
        },
    );
    if g > 0.6 {
        q.px(xi - dir, yi + 1.0, c[3]);
    }
    if g > 0.7 {
        for (dx, dy) in [(0.0, -2.0), (0.0, 2.0), (-2.0, 0.0), (2.0, 0.0)] {
            q.dot(xi - dir + dx, yi + dy, c[3], (g - 0.7) * 3.0);
        }
    }
    q.glow(x, y, 28.0, c[2], 0.45 * g);
    q.glow(x, y, 10.0, c[4], 0.5 * g);
    q.glow(x, y, 4.0, c[5], 0.5 * g);
}

/// The meadow: blades in two rows, swaying as the breeze rolls across, a few seed heads; blade tips near a lit
/// firefly catch its light.
fn grass(q: &mut Painter, st: &State, t: f64, lit: &[(f64, f64, f64)], front: bool) {
    let (cv, s, g, c) = (st.cv, st.s, &st.g, &st.c);
    // the blades stand 2 pixels past each side of the canvas
    let (left, span) = (cv.x0 - 2.0, cv.span() + 4.0);
    let n = count(if front { 150 } else { 230 }, span, 324.0);
    if n == 0 {
        return;
    }
    let sd = s * 3.0 + if front { 1.0 } else { 0.0 };
    for i in 0..n {
        let i = i as f64;
        let r = |k: f64| rand(&[sd, i, k]);
        let x = left + (i + r(1.0)) * (span / n as f64);
        let h = if front { 4.0 } else { 5.0 }
            + r(2.0) * if front { 8.0 } else { 11.0 }
            + if r(3.0) < 0.1 { 6.0 } else { 0.0 };
        let base = if front { 9.0 } else { 2.0 } - r(4.0) * 3.0;
        let sway = (1.2 + r(5.0)) * libm::sin(cyc(t, 2.0, -x * 0.03) + r(6.0))
            + 0.6 * libm::sin(cyc(t, 5.0, x * 0.07));
        let tone = if front {
            1 + libm::floor(r(7.0) * 3.0) as usize
        } else {
            1 + libm::floor(r(7.0) * 2.0) as usize
        };
        let mut tip_light = 0.0;
        let (tx, ty) = (x + sway, base - h);
        for &(fx, fy, glow) in lit {
            // cheap reject before the distance: only flies within 28 px light a tip
            if (fx - tx).abs() >= 28.0 || (fy - ty).abs() >= 28.0 {
                continue;
            }
            let d = libm::hypot(fx - tx, fy - ty);
            if d < 28.0 {
                tip_light = js_max(tip_light, glow * (1.0 - d / 28.0));
            }
        }
        let low = g.get(tone).copied().unwrap_or(g[1]);
        let high = g.get((tone + 1).min(5)).copied().unwrap_or(g[2]);
        let mut row = 0.0;
        while row < h {
            let u = row / h;
            let col = if u > 0.8 && tip_light > 0.2 {
                if tip_light > 0.55 && u > 0.9 {
                    c[3]
                } else {
                    c[2]
                }
            } else if u > 0.7 {
                high
            } else {
                low
            };
            q.px(x + sway * u * u, base - row, col);
            row += 1.0;
        }
        if r(8.0) < 0.07 {
            q.px(tx, ty - 1.0, if tip_light > 0.2 { c[3] } else { g[5] });
            q.px(tx - 1.0, ty, g[4]);
            q.px(tx + 1.0, ty, g[4]);
        }
    }
}
