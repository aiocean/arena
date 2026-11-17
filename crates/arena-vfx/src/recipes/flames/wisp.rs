//! `critters/wisp`: a will-o'-the-wisp loops a figure-eight over the floor: a little soul-flame with a face, its
//! flame tail licking up and streaming behind, shedding puffs and sparks (the engine's `critters/wisp`, with the
//! critters kit's tongue and puff, adapted from Sprite Fusion's open-source fire, MIT License, (c) 2026 Hugo Duprez).

use crate::kit::sim::{Flags, Kind, Particle, Particles, ParticlesOpts};
use crate::kit::{
    Layer, Opts, Painter, Rgba, Show, TAU, clamp, cycle, fly, js_max, js_min, js_round, libm, ramp,
    tile_noise, wrap,
};

pub const D: f64 = 3.2;
/// How high it flies (the engine's `hover` option, which the duel leaves at its default).
const HOVER: f64 = 36.0;

const PALETTE: [Rgba; 7] = [
    Rgba(0x0a, 0x1f, 0x2e, 255),
    Rgba(0x0f, 0x3d, 0x52, 255),
    Rgba(0x14, 0x70, 0x7e, 255),
    Rgba(0x1f, 0xa8, 0xa8, 255),
    Rgba(0x4f, 0xe0, 0xc8, 255),
    Rgba(0xa8, 0xff, 0xf0, 255),
    Rgba(255, 255, 255, 255),
];

/// The loop time for this critter: other seeds start it elsewhere in its loop.
fn shift(t: f64, d: f64, seed: f64) -> f64 {
    wrap(t + (seed - 1.0) * 0.618 * d, d)
}

/// Smooth noise in -1..1 that repeats every `d` seconds.
fn lnoise(t: f64, d: f64, f: f64, off: f64, seed: f64) -> f64 {
    tile_noise((t / d) * f + off, 0.5, f, 0.0, seed)
}

const LAYERS: [(f64, f64, usize); 4] = [
    (1.0, 1.0, 6),
    (0.76, 0.86, 4),
    (0.54, 0.68, 2),
    (0.3, 0.42, 0),
];

fn step(heat: &[Rgba], i: usize) -> Rgba {
    heat.get(i).copied().unwrap_or(Rgba::NONE)
}

/// A tongue of flame standing on (x, y), h tall, w half-wide at its root, its tip swaying sx; just layer `only`.
#[allow(clippy::too_many_arguments)]
fn tongue(
    p: &mut Painter,
    x: f64,
    y: f64,
    h: f64,
    w: f64,
    sx: f64,
    t: f64,
    d: f64,
    n: f64,
    heat: &[Rgba],
    only: usize,
) {
    if h < 1.0 {
        return;
    }
    let f1 = js_max(1.0, js_round(9.0 * d));
    let f2 = js_max(1.0, js_round(4.0 * d));
    let Some(&(lw, lh, ri)) = LAYERS.get(only) else {
        return;
    };
    let hh = h * lh;
    let col = step(heat, ri);
    let mut r = 0.0;
    while r <= hh {
        let u = r / js_max(1.0, hh);
        let half = w * lw * libm::pow(1.0 - u, 0.85) * js_min(1.0, (r + 1.5) / 3.5)
            + lnoise(t, d, f1, r * 0.45, n) * 0.5;
        let cx = x + sx * u * u * lh + lnoise(t, d, f2, r * 0.2, n + 7.0) * u * 1.1;
        if half >= 0.3 {
            let mut px = js_round(cx - half);
            while px <= js_round(cx + half) {
                p.px(px, js_round(y - r), col);
                px += 1.0;
            }
        }
        r += 1.0;
    }
}

/// A puff of flame k of its life left, cooling down `heat` (bright-first).
fn fire_puff(p: &mut Painter, x: f64, y: f64, k: f64, size: f64, seed: f64, heat: &[Rgba]) {
    let age = 1.0 - k;
    let n = heat.len();
    let nf = n as f64;
    let hot = js_min(0.999, libm::pow(age, 0.62) + (seed % 1.0) * 0.1);
    let idx = js_min(nf - 2.0, libm::floor(hot * (nf - 1.0))).max(0.0) as usize;
    let r = size * (0.55 + 0.45 * js_min(1.0, age * 7.0)) * (1.0 - age * 0.55);
    let c = step(heat, idx);
    if idx + 2 >= n {
        return p.disc(x, y, r, c, js_min(1.0, k * 3.0) * 0.85);
    }
    p.circle(x, y, r, step(heat, (idx + 2).min(n - 1)), 1.0);
    p.circle(x - r * 0.16, y - r * 0.22, r * 0.78, c, 1.0);
    if r > 1.6 {
        p.circle(x, y - r * 0.9, r * 0.42, c, 1.0);
    }
    if idx <= 3 && r > 1.8 {
        p.circle(
            x - r * 0.28,
            y - r * 0.34,
            r * 0.36,
            step(heat, idx.saturating_sub(1)),
            1.0,
        );
    }
    if idx < 6 {
        p.glow(x, y, r * 2.4 + 2.0, c, 0.18 * k);
    }
}

/// A soft shadow: an ellipse blended over what's there, fading to nothing at its rim.
fn shadow(p: &mut Painter, cx: f64, cy: f64, rx: f64, ry: f64, c: Rgba, a: f64) {
    let mut y = libm::floor(cy - ry);
    while y <= libm::ceil(cy + ry) {
        let mut x = libm::floor(cx - rx);
        while x <= libm::ceil(cx + rx) {
            let dx = (x + 0.5 - cx) / rx;
            let dy = (y + 0.5 - cy) / ry;
            let d = dx * dx + dy * dy;
            if d < 1.0 {
                p.blend(x, y, c, a * libm::pow(1.0 - d, 0.7));
            }
            x += 1.0;
        }
        y += 1.0;
    }
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(Wisp {
        c: ramp(&o.palette_or(&PALETTE)),
        seed: o.seed,
    })
}

struct Wisp {
    c: [Rgba; 7],
    seed: f64,
}

impl Show for Wisp {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        match layer {
            Layer::Back => draw_back(p, t, self),
            Layer::Front => draw(p, t, self),
        }
    }
}

/// Where the wisp is `t` seconds into its loop, and how fast it moves.
struct Pose {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
}

fn at(t: f64) -> Pose {
    let a = (t / D) * TAU;
    Pose {
        x: 34.0 * libm::sin(a),
        y: -HOVER - 10.0 * libm::sin(2.0 * a),
        vx: ((34.0 * TAU) / D) * libm::cos(a),
        vy: ((-20.0 * TAU) / D) * libm::cos(2.0 * a),
    }
}

fn draw(p: &mut Painter, time: f64, o: &Wisp) {
    let t = shift(time, D, o.seed);
    let c = &o.c;
    let s = at(t);
    let heat = [c[6], c[6], c[5], c[5], c[4], c[4], c[3], c[3], c[3]];
    let lean = clamp(-s.vx / 80.0, -0.9, 0.9);
    let stretch = clamp(s.vy / 120.0, -0.3, 0.4);
    let (x0, y0) = (s.x, s.y);
    // flame puffs shed along its path, cooling down the ramp as they drift up
    cycle(
        t,
        D,
        30,
        o.seed * 7.0 + 1.0,
        |r| r.range(0.3, 0.6),
        |age, life, r, _| {
            let s0 = at(wrap(t - age, D));
            let px = s0.x + r.range(-3.0, 3.0);
            let py = s0.y - r.range(0.0, 4.0);
            let vx = r.range(-8.0, 8.0);
            let vy = -r.range(6.0, 16.0);
            let (x, y) = fly(px, py, vx, vy, age, -40.0, 2.5);
            let size = r.range(1.2, 2.6);
            let seed = r.next() * 100.0;
            fire_puff(p, x, y, 1.0 - age / life, size, seed, &heat);
        },
    );
    // the tail: three tongues off the top of its head, streaming back as it flies, laid down layer by layer
    let tails: [(f64, f64, f64, f64); 3] = [
        (-3.5, 13.0, 2.5, 3.0),
        (3.5, 14.0, 2.5, 7.0),
        (0.0, 20.0, 3.2, 11.0),
    ];
    for li in 0..4 {
        for (dx, h, w, n) in tails {
            let hh = (h + 0.25 * h * lnoise(t, D, 9.0, n, 5.0)) * (1.0 + stretch);
            tongue(
                p,
                x0 + dx,
                y0 - 2.0,
                hh,
                w,
                lnoise(t, D, 6.0, n, 40.0) * 1.5 + lean * hh * 0.9,
                t,
                D,
                n,
                &heat,
                li,
            );
        }
    }
    // the head: a ball of cold fire, white at the heart, its rim flickering
    let rad = 7.5;
    p.shade(
        x0 - rad - 2.0,
        y0 - rad - 2.0,
        x0 + rad + 2.0,
        y0 + rad + 2.0,
        |x, y, _| {
            let dx = x - x0;
            let dy = y - y0;
            let ang = libm::atan2(dy, dx);
            let rr = rad
                + 0.7 * lnoise(t, D, 16.0, ang * 1.6 + 10.0, 3.0)
                + if dy > 0.0 { -0.6 } else { 0.0 };
            let d = libm::hypot(dx, dy) / rr;
            if d > 1.0 {
                return None;
            }
            Some(if d < 0.42 {
                c[6]
            } else if d < 0.68 {
                c[5]
            } else if d < 0.88 {
                c[4]
            } else {
                c[3]
            })
        },
    );
    // its face: eyes looking where it's going, a blink, a little mouth
    let look = js_round(clamp(s.vx / 60.0, -1.0, 1.0));
    let ph = wrap(t, D);
    let blink = ph > 2.2 && ph < 2.28;
    for sd in [-1.0, 1.0] {
        let ex = js_round(x0) + sd * 2.0 - if sd > 0.0 { 0.0 } else { 1.0 } + look;
        let ey = js_round(y0) - 1.0;
        if blink {
            p.px(ex, ey + 1.0, c[1]);
            p.px(ex + 1.0, ey + 1.0, c[1]);
        } else {
            for k in 0..3 {
                p.px(ex, ey + f64::from(k), c[1]);
                p.px(ex + 1.0, ey + f64::from(k), c[1]);
            }
            p.px(ex + if look > 0.0 { 1.0 } else { 0.0 }, ey, c[6]);
        }
    }
    p.px(js_round(x0) + look, js_round(y0) + 3.0, c[2]);
    p.px(js_round(x0) - 1.0 + look, js_round(y0) + 3.0, c[2]);
    // sparks wander up off it
    let mut ps = Particles::new(ParticlesOpts {
        sparks: vec![c[6], c[5], c[4], c[3]],
        ..ParticlesOpts::default()
    });
    ps.set_time(t);
    cycle(
        t,
        D,
        26,
        o.seed * 7.0 + 2.0,
        |r| r.range(0.8, 1.6),
        |age, life, r, _| {
            let s0 = at(wrap(t - age, D));
            let sd = r.next() * 100.0;
            let px = s0.x + r.range(-5.0, 5.0);
            let py = s0.y - r.range(4.0, 12.0);
            let vx = r.range(-20.0, 20.0);
            let vy = -r.range(20.0, 45.0);
            let (x, y) = fly(px, py, vx, vy, age, -15.0, 1.6);
            let sx = x + libm::sin(age * 2.3 + sd) * 3.0 * js_min(1.0, age);
            let size = if r.chance(0.3) { 2.0 } else { 1.0 };
            if age >= 0.0 && age < life {
                ps.list_mut().push(Particle {
                    kind: Kind::Ember,
                    x: sx,
                    y,
                    vx: 0.0,
                    vy: 0.0,
                    life: life - age,
                    max: life,
                    col: c[4],
                    size,
                    seed: sd,
                    delay: 0.0,
                    flags: Flags::NONE,
                });
            }
        },
    );
    ps.draw(p, Layer::Front);
    // light: a cold glow round it, a hot heart
    let fl = 0.85 + 0.15 * lnoise(t, D, 22.0, 0.0, 9.0);
    p.glow(x0 + lean * 3.0, y0 - 6.0, 46.0 * fl, c[4], 0.38);
    p.glow(x0, y0 - 1.0, 13.0, c[5], 0.35 * fl);
}

fn draw_back(p: &mut Painter, time: f64, o: &Wisp) {
    let t = shift(time, D, o.seed);
    let c = &o.c;
    let s = at(t);
    let near = clamp(1.0 - (-s.y - 24.0) / 30.0, 0.0, 1.0);
    // the floor lit cold beneath it, brighter as it dips
    shadow(p, s.x, 0.0, 7.0, 1.2, c[0], 0.4);
    let mut x = -24.0;
    while x <= 24.0 {
        p.glow(
            s.x + x,
            -1.0,
            16.0,
            c[4],
            (0.08 + 0.1 * near) * (1.0 - x.abs() / 32.0),
        );
        x += 6.0;
    }
}
