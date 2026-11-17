//! `weather/snow`, after the engine's (itself after Sprite Fusion's open-source snow,
//! https://www.spritefusion.com/pixel-art-vfx/snow, MIT, Copyright (c) 2026 Hugo Duprez): snow at three depths in a
//! gusty breeze (far specks, drifting flakes, big near crystals that spin and glow) settling on a sparkling drift.
//!
//! The duel plays it as Frost's flurry: a patch round the foe (its `width` and `height`, 48 x 72 world px), stopped
//! after 1.2 s. The engine's cold haze fills the patch to its edges; here it thins out toward the patch's sides and
//! top, so the flurry reads as a squall rather than a dark box, and the patch opens over 0.2 s.

use super::sky::{Cv, clip, count, pass, sized, trips};
use crate::kit::{
    Layer, Opts, Painter, Rgba, Show, TAU, WHITE, clamp, js_min, js_round, libm, mix, ramp, rand,
    smooth, wrap,
};

/// The effect's own canvas, which `width` and `height` default to.
const SIZE: (f64, f64) = (320.0, 180.0);
const D: f64 = 12.0;
/// Seconds the patch takes to open.
const OPEN: f64 = 0.2;

const PALETTE: [Rgba; 8] = [
    Rgba::hex24(0x0c1020),
    Rgba::hex24(0x161d36),
    Rgba::hex24(0x24304f),
    Rgba::hex24(0x3a4a72),
    Rgba::hex24(0x5f74a3),
    Rgba::hex24(0x93abd6),
    Rgba::hex24(0xcfe0fa),
    Rgba::hex24(0xffffff),
];

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(State {
        c: ramp(&o.palette_or(&PALETTE)),
        s: o.seed,
        cv: sized(o, SIZE),
        age: 0.0,
    })
}

struct State {
    c: [Rgba; 8],
    s: f64,
    cv: Cv,
    /// Seconds it has played (for its opening).
    age: f64,
}

/// The drift's top edge.
fn bank(x: f64) -> f64 {
    -3.0 + 1.6 * libm::sin(x * 0.045 + 1.3)
        + 1.1 * libm::sin(x * 0.12 + 0.4)
        + 0.6 * libm::sin(x * 0.31 + 2.0)
}

/// The gusts: a sway of the whole snowfall, periodic so the loop closes.
fn gust(t: f64) -> f64 {
    10.0 * libm::sin((TAU * t) / D + 0.6) + 4.0 * libm::sin((TAU * 2.0 * t) / D + 2.1)
}

/// One depth of snow: how many over the effect's own width, falls a loop, share of a fall in the air (the rest lying
/// on the drift), sway, wind, how much of the gust it takes.
struct Depth {
    n: usize,
    m: (f64, f64),
    air: f64,
    sway: (f64, f64),
    wind: f64,
    gust: f64,
}

// far to near
const LAYERS: [Depth; 3] = [
    Depth {
        n: 230,
        m: (1.0, 2.0),
        air: 1.0,
        sway: (1.0, 3.0),
        wind: 3.0,
        gust: 0.4,
    },
    Depth {
        n: 140,
        m: (2.0, 3.0),
        air: 0.8,
        sway: (3.0, 7.0),
        wind: 6.0,
        gust: 0.7,
    },
    Depth {
        n: 24,
        m: (2.0, 3.0),
        air: 0.74,
        sway: (8.0, 15.0),
        wind: 9.0,
        gust: 1.0,
    },
];

impl Show for State {
    fn update(&mut self, dt: f64) {
        self.age += dt;
    }

    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        p.fade *= smooth(0.0, OPEN, self.age + 1.0 / 30.0);
        clip(p, self.cv);
        let (cv, c) = (self.cv, &self.c);
        if layer == Layer::Back {
            // a cold haze, thicker low down and thinning to the patch's sides and top
            let haze = mix(c[1], c[2], 0.4);
            let (top, half) = (cv.y0, cv.span() / 2.0);
            let mid = (cv.x0 + cv.x1) / 2.0;
            let edge = js_min(10.0, half * 0.4);
            pass(p, cv, top, cv.y1 - 1.0, |x, y, out| {
                let side = smooth(0.0, edge, half - (x - mid).abs());
                let lid = smooth(top, top + cv.tall() * 0.3, y);
                out.c = haze;
                out.a = (0.26 + 0.22 * smooth(top, 0.0, y)) * side * lid;
                out.b = true;
            });
            depth(p, self, t, 0);
            drift(p, self, t);
            depth(p, self, t, 1);
        } else {
            depth(p, self, t, 2);
        }
        p.unclip();
    }
}

fn depth(q: &mut Painter, st: &State, t: f64, l: usize) {
    let Some(ly) = LAYERS.get(l) else {
        return;
    };
    let (cv, s, c) = (st.cv, st.s, &st.c);
    let (left, span) = (cv.x0, cv.span());
    let sd = s * 13.0 + l as f64;
    let n = count(ly.n, span, SIZE.0);
    trips(t, D, n, sd, ly.m.0, ly.m.1, |u, k, m, i| {
        let j = wrap(k, m);
        let dur = D / m;
        let r = |n: f64| rand(&[sd, i, j, n]);
        let tf = dur * ly.air;
        let age = js_min(u * dur, tf);
        let sw = ly.sway.0 + r(3.0) * (ly.sway.1 - ly.sway.0);
        let f = 0.25 + r(4.0) * 0.35;
        let ph = r(5.0) * TAU;
        let x_at = |a: f64| {
            left - 15.0
                + r(2.0) * (span + 30.0)
                + ly.wind * a
                + (gust(t - u * dur + a) - gust(t - u * dur)) * ly.gust
                + sw * libm::sin(TAU * f * a + ph)
        };
        let (x, xl) = (x_at(age), x_at(tf));
        let y0 = cv.y0 - 6.0;
        let y1 = if l != 0 {
            bank(xl) - 0.5
        } else {
            -110.0 + r(1.0) * 106.0
        };
        let y = y0 + (y1 - y0) * (age / tf);
        let rest = u * dur - tf;
        if l == 0 {
            // far: a speck, thinning out before it's gone
            let a = js_min(1.0, (1.0 - u) * 6.0);
            q.dot(x, y, if r(6.0) < 0.5 { c[4] } else { c[5] }, a);
            return;
        }
        let fade = if rest > 0.0 {
            1.0 - rest / (dur - tf)
        } else {
            1.0
        };
        if l == 1 {
            // a drifting flake: a point with a smaller one beside it, a plus when it's big
            let big = r(6.0) < 0.3;
            if rest > 0.0 {
                q.dot(xl, y1, c[7], fade * 0.9);
                return;
            }
            q.px(x, y, if big { WHITE } else { c[7] });
            if big {
                for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    q.px(x + dx, y + dy, c[5]);
                }
            } else {
                q.px(x + 1.0, y, c[6]);
                q.px(x, y + 1.0, c[6]);
                q.px(x + 1.0, y + 1.0, c[5]);
            }
            return;
        }
        // a near crystal: arms with side branches, spinning, a white heart, a cold glow; it lies down on the drift
        let rad = if r(6.0) < 0.4 {
            3
        } else if r(6.0) < 0.8 {
            4
        } else {
            5
        };
        let spin = 1.5 + r(7.0) * 2.5;
        if rest > 0.0 {
            q.dot(xl - 1.0, y1, c[6], fade);
            q.dot(xl, y1, WHITE, fade);
            q.dot(xl + 1.0, y1, c[6], fade);
            q.glow(xl, y1, 5.0, c[5], 0.25 * fade);
            return;
        }
        let turn = libm::floor(age * spin + r(8.0) * 2.0) % 2.0 != 0.0;
        crystal(q, x, y, rad, turn, c);
    });
}

type Dirs = [(i32, i32); 4];
const AXIS: Dirs = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const DIAG: Dirs = [(1, 1), (-1, -1), (1, -1), (-1, 1)];

/// A near crystal, kept crisp: long arms on one set of four directions, short ones on the other, nubs near the tips;
/// turning trades which set is long.
fn crystal(q: &mut Painter, x: f64, y: f64, r: i32, turn: bool, c: &[Rgba; 8]) {
    let (xi, yi) = (js_round(x) as i32, js_round(y) as i32);
    let (long, short) = if turn { (DIAG, AXIS) } else { (AXIS, DIAG) };
    let arm = if turn { r - 1 } else { r };
    let px = |q: &mut Painter, x: i32, y: i32, col: Rgba| q.px(f64::from(x), f64::from(y), col);
    for (dx, dy) in long {
        for d in 1..=arm {
            px(
                q,
                xi + dx * d,
                yi + dy * d,
                if d == arm { WHITE } else { c[6] },
            );
        }
        let nubs: &[i32] = if turn {
            if arm >= 3 { &[arm - 1] } else { &[] }
        } else if arm >= 5 {
            &[arm - 1, arm - 2]
        } else if arm >= 4 {
            &[arm - 1]
        } else {
            &[]
        };
        for &n in nubs {
            let (ox, oy) = if turn { (dx, -dy) } else { (dy, -dx) };
            px(
                q,
                xi + dx * n + if turn { 0 } else { ox },
                yi + dy * n + oy,
                c[4],
            );
            px(
                q,
                xi + dx * n + if turn { -dx } else { -ox } + if turn { dx } else { 0 },
                yi + dy * n + if turn { 0 } else { -oy },
                c[4],
            );
        }
    }
    let reach = if turn { 1 } else { (arm / 2).max(1) };
    for (dx, dy) in short {
        for d in 1..=reach {
            px(
                q,
                xi + dx * d,
                yi + dy * d,
                if d == 1 { c[6] } else { c[5] },
            );
        }
    }
    px(q, xi, yi, WHITE);
    // a faint halo, wide enough not to fill the gaps between the arms
    q.glow(
        f64::from(xi) + 0.5,
        f64::from(yi) + 0.5,
        f64::from(r) * 2.5 + 5.0,
        c[5],
        0.13,
    );
    q.light(f64::from(xi), f64::from(yi), c[6], 0.4);
}

/// The drift on the floor: a lumpy bank with a white crest, blue shade below, and glints that twinkle.
fn drift(q: &mut Painter, st: &State, t: f64) {
    let (cv, s, c) = (st.cv, st.s, &st.c);
    pass(q, cv, -7.0, 9.0, |x, y, out| {
        let d = y - bank(x);
        if d < 0.0 {
            return;
        }
        out.c = if d < 1.0 {
            WHITE
        } else if d < 2.0 {
            c[7]
        } else if d < 3.5 {
            c[6]
        } else if d < 6.0 {
            c[5]
        } else {
            c[4]
        };
        if d < 1.0 {
            out.l = c[6];
            out.k = 0.12;
        }
    });
    let (left, span) = (cv.x0, cv.span());
    for i in 0..count(18, span, SIZE.0) {
        let i = i as f64;
        let x = left + 4.0 + rand(&[s, i, 81.0]) * (span - 8.0);
        let cycles = 1.0 + libm::floor(rand(&[s, i, 82.0]) * 3.0);
        let tw = libm::sin((TAU * cycles * t) / D + rand(&[s, i, 83.0]) * TAU);
        if tw < 0.55 {
            continue;
        }
        let y = libm::ceil(bank(x)) + libm::floor(rand(&[s, i, 84.0]) * 3.0);
        let k = clamp((tw - 0.55) / 0.45, 0.0, 1.0);
        q.px(x, y, WHITE);
        if k > 0.5 {
            for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, -1.0)] {
                q.px(x + dx, y + dy, c[6]);
            }
        }
        q.glow(x + 0.5, y + 0.5, 6.0, c[6], 0.35 * k);
    }
}
