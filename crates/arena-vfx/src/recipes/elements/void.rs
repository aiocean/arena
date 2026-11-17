//! `shadow/void-orb`, after the engine's: a black orb hangs 62 px over its origin with a pale rim crawling round it; a
//! dark vortex winds into it, dust is drawn up off the floor, motes and streaks are pulled in, and light from behind
//! bends round its edge. Played on the floor under the foe; it loops every 3.2 s until the rules stop it.
//!
//! The engine takes the light back off the orb at the end (`unlight`); this kit has no way to take light back, so
//! every light drawn over the layer the orb is on skips the orb's disc instead (`lit`, `lit_px`).

use crate::vfx::kit::{
    Layer, Opts, Painter, Recipe, Rgba, Rng, Show, TAU, WHITE, clamp, cycle, ease, fbm, js_max,
    js_min, js_sign, libm, ramp, smooth,
};

pub const RECIPE: Recipe = Recipe::new("shadow/void-orb", true, 3.2, make);

// the orb's centre and radius, the vortex's reach, the gap matter can't orbit in, the radius light bends to
const Y: f64 = -62.0;
const R: f64 = 12.0;
const VR: f64 = 48.0;
const GAP: f64 = R + 9.0;
const E: f64 = R + 4.5;
const D: f64 = 3.2;
const ARMS: f64 = 3.0;
const K: f64 = 7.0;
const WIND: f64 = (TAU * 2.0) / D;
/// Two gulps a loop: the rim flares, the vortex brightens, a ring is drawn in.
const GULPS: [f64; 2] = [0.2, 0.68];

/// Void black, deep violet, a white-lilac edge.
const VOID: [Rgba; 8] = [
    Rgba::hex24(0x06040b),
    Rgba::hex24(0x150c26),
    Rgba::hex24(0x2a1150),
    Rgba::hex24(0x461a84),
    Rgba::hex24(0x6f2cc0),
    Rgba::hex24(0x9d5cec),
    Rgba::hex24(0xcfa6ff),
    Rgba::hex24(0xf6eaff),
];

fn fwrap(x: f64) -> f64 {
    x - libm::floor(x)
}

fn gulp(t: f64) -> f64 {
    GULPS.iter().fold(0.0, |s, &g| {
        let v = ((fwrap(t / D - g + 0.5) - 0.5) * D) / 0.07;
        s + libm::exp(-(v * v))
    })
}

/// A speck of light behind the orb on a looping path, seen through the lens.
struct Source {
    cx: f64,
    cy: f64,
    rho: f64,
    n: f64,
    ph: f64,
    s: f64,
    sy: f64,
}

const fn src(cx: f64, cy: f64, rho: f64, n: f64, ph: f64, s: f64, sy: f64) -> Source {
    Source {
        cx,
        cy,
        rho,
        n,
        ph,
        s,
        sy,
    }
}

const SOURCES: [Source; 7] = [
    src(0.0, 0.0, 6.0, 1.0, 0.0, 2.6, 0.75),
    src(1.0, -1.0, 8.0, -1.0, 2.2, 2.0, 0.6),
    src(18.0, -6.0, 24.0, 1.0, 0.4, 2.4, 0.7),
    src(-26.0, 10.0, 30.0, 1.0, 2.6, 2.0, 0.55),
    src(8.0, 22.0, 20.0, 2.0, 4.1, 1.6, 0.8),
    src(-12.0, -26.0, 34.0, 1.0, 5.3, 1.8, 0.6),
    src(34.0, 18.0, 16.0, 2.0, 1.2, 1.4, 0.9),
];

/// One cell of the table round the orb: radius, angle, log radius, lumpy turbulence for the vortex.
#[derive(Clone, Copy)]
struct Cell {
    r: f32,
    th: f32,
    ln: f32,
    nz: f32,
}

fn tables(n: usize) -> Vec<Cell> {
    let mut cells = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let x = i as f64 - VR + 0.5;
            let y = j as f64 - VR + 0.5;
            let a = libm::atan2(y, x);
            let d = libm::hypot(x, y);
            let l = libm::log(js_max(1.0, d));
            // the vortex only reads cells inside its ring: skip the noise elsewhere
            let nz = if d < GAP || d > VR {
                0.0
            } else {
                fbm(
                    libm::cos(a) * 2.5 + l * 2.4,
                    libm::sin(a) * 2.5 + l * 2.4,
                    17.0,
                    3,
                )
            };
            cells.push(Cell {
                r: d as f32,
                th: a as f32,
                ln: l as f32,
                nz: nz as f32,
            });
        }
    }
    cells
}

struct Orb {
    c: [Rgba; 8],
    seed: f64,
    cells: Vec<Cell>,
}

fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(Orb {
        c: ramp(&o.palette_or(&VOID)),
        seed: o.seed,
        cells: tables((VR * 2.0) as usize),
    })
}

impl Show for Orb {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        match layer {
            Layer::Back => draw_back(p, t, self),
            Layer::Front => draw_front(p, t, self),
        }
    }
}

/// Inside the orb's black disc (local).
fn in_orb(x: f64, y: f64) -> bool {
    x * x + (y - Y) * (y - Y) < (R - 0.6) * (R - 0.6)
}

/// A radial glow that leaves the orb's disc dark.
fn lit(p: &mut Painter, x: f64, y: f64, r: f64, c: Rgba, k: f64) {
    if r < 1.0 || k <= 0.004 {
        return;
    }
    p.each(x - r, y - r, x + r, y + r, |p, lx, ly, sx, sy| {
        if in_orb(lx, ly) {
            return;
        }
        let d = libm::hypot(lx - x, ly - y) / r;
        if d < 1.0 {
            let f = 1.0 - d;
            p.add(sx, sy, c, f * f * k);
        }
    });
}

/// Light on one pixel, unless it is on the orb.
fn lit_px(p: &mut Painter, x: f64, y: f64, c: Rgba, k: f64) {
    if !in_orb(x, y) {
        p.light(x, y, c, k);
    }
}

/// The pixel cells of the annulus `r0 <= d < r1` round (x, y): `f(cx, cy, d)`.
fn annulus(x: f64, y: f64, r0: f64, r1: f64, mut f: impl FnMut(f64, f64, f64)) {
    let (r02, r12) = (r0 * r0, r1 * r1);
    let mut py = libm::floor(y - r1);
    while py <= libm::ceil(y + r1) {
        let cy = py + 0.5;
        let mut px = libm::floor(x - r1);
        while px <= libm::ceil(x + r1) {
            let cx = px + 0.5;
            let d2 = (cx - x) * (cx - x) + (cy - y) * (cy - y);
            if d2 < r12 && d2 >= r02 {
                f(cx, cy, libm::sqrt(d2));
            }
            px += 1.0;
        }
        py += 1.0;
    }
}

fn ring_fast(p: &mut Painter, x: f64, y: f64, r: f64, w: f64, c: Rgba, a: f64) {
    if r > 0.0 {
        annulus(x, y, js_max(0.0, r - w), r, |cx, cy, _| p.dot(cx, cy, c, a));
    }
}

fn ring_light(p: &mut Painter, x: f64, y: f64, big_r: f64, w: f64, c: Rgba, k: f64) {
    if w < 1.0 || k <= 0.004 {
        return;
    }
    annulus(x, y, js_max(0.0, big_r - w), big_r + w, |cx, cy, d| {
        let f = 1.0 - libm::fabs(d - big_r) / w;
        lit_px(p, cx, cy, c, f * f * k);
    });
}

/// A ring of light hugging the black disc: `k` at radius `r0`, falling to nothing at `r1`.
fn halo(p: &mut Painter, x: f64, y: f64, r0: f64, r1: f64, c: Rgba, k: f64) {
    if r1 <= r0 || k <= 0.004 {
        return;
    }
    let w = r1 - r0;
    annulus(x, y, js_max(0.0, r0), r1, |cx, cy, d| {
        let f = 1.0 - (d - r0) / w;
        lit_px(p, cx, cy, c, f * f * k);
    });
}

/// The vortex: three arms of dark dust winding inward, a dithered veil threaded with bright filaments streaming in,
/// a lit leading edge, heating to pale violet near the rim.
fn vortex(p: &mut Painter, c: &[Rgba; 8], cells: &[Cell], t: f64, g: f64) {
    let n = (VR * 2.0) as usize;
    let wf = (TAU * 12.0) / D;
    for j in 0..n {
        for i in 0..n {
            let Some(cell) = cells.get(j * n + i) else {
                continue;
            };
            let r = f64::from(cell.r);
            if r < GAP || r > VR {
                continue;
            }
            let (th, ln, nz) = (f64::from(cell.th), f64::from(cell.ln), f64::from(cell.nz));
            let env = smooth(VR, VR * 0.42, r) * smooth(GAP, GAP + 4.0, r);
            let ph = ARMS * th + K * ln + WIND * t + nz * 2.6;
            let a = (0.5 + 0.5 * libm::cos(ph)) * env * (1.0 + 0.4 * g);
            if a < 0.16 {
                continue;
            }
            let x = i as f64 - VR + 0.5;
            let y = Y + j as f64 - VR + 0.5;
            let hot = 1.0 - smooth(GAP, GAP + 16.0, r);
            let fil = libm::cos(12.0 * th + 28.0 * ln + wf * t + nz * 1.6);
            let dash = libm::cos(9.0 * ln * 2.2 + 2.2 * th + wf * 0.75 * t);
            if fil > 0.86 && a > 0.3 && dash > -0.55 {
                let col = if hot > 0.6 {
                    c[7]
                } else if (hot > 0.25 || fil > 0.97) && dash > 0.0 {
                    c[6]
                } else {
                    c[5]
                };
                p.px(x, y, col);
                p.light(x, y, col, 0.1 + 0.2 * hot);
                continue;
            }
            if a > 0.36 && a < 0.46 && libm::sin(ph) > 0.0 {
                p.px(x, y, if hot > 0.4 { c[6] } else { c[4] });
                continue;
            }
            p.blend(
                x,
                y,
                if a > 0.6 { c[3] } else { c[2] },
                js_min(0.75, (a - 0.16) * 1.4),
            );
        }
    }
}

/// What the orb's gravity does to light from behind it: the faint Einstein ring, and specks smeared into arcs.
fn lensing(p: &mut Painter, c: &[Rgba; 8], t: f64, g: f64) {
    let st = 0.7 / E;
    let mut a = 0.0;
    while a < TAU {
        let f = 0.5 + 0.5 * libm::cos(5.0 * a - (TAU * 2.0 * t) / D) + 0.5 * g;
        let col = if f > 0.85 {
            c[5]
        } else if f > 0.45 {
            c[4]
        } else {
            c[3]
        };
        p.px(libm::cos(a) * E, Y + libm::sin(a) * E, col);
        a += st;
    }
    for s in &SOURCES {
        let q = (TAU * s.n * t) / D + s.ph;
        let sx = s.cx + s.rho * libm::cos(q);
        let sy = s.cy + s.rho * libm::sin(q) * s.sy;
        let b = js_max(0.3, libm::hypot(sx, sy));
        let al = libm::atan2(sy, sx);
        let root = libm::sqrt(b * b + 4.0 * E * E);
        let images = [
            ((b + root) / 2.0, al, 1.0),
            ((root - b) / 2.0, al + core::f64::consts::PI, 0.55),
        ];
        for (r, a0, w) in images {
            if r < R + 1.5 {
                continue;
            }
            let mag = (r / b) * w;
            let hw = js_min(core::f64::consts::PI, (s.s * 1.6) / (2.0 * b));
            let step = 0.6 / r;
            let mut a = a0 - hw;
            while a <= a0 + hw + 1e-6 {
                let u = if hw > 0.0 {
                    libm::fabs(a - a0) / hw
                } else {
                    0.0
                };
                let x = libm::cos(a) * r;
                let y = Y + libm::sin(a) * r;
                let col = if u < 0.3 && mag > 0.8 {
                    WHITE
                } else if u < 0.65 {
                    c[7]
                } else {
                    c[5]
                };
                p.dot(x, y, col, js_min(1.0, mag * 1.4) * (1.15 - u));
                if mag > 1.2 && u < 0.6 {
                    p.dot(x + libm::cos(a), y + libm::sin(a), c[6], 0.8);
                }
                a += step;
            }
            p.glow(
                libm::cos(a0) * r,
                Y + libm::sin(a0) * r,
                4.0 + js_min(10.0, hw * r),
                c[6],
                js_min(0.5, 0.18 * mag),
            );
        }
    }
}

/// The orb: void black, a dull glint and a crescent of reflected light inside, and a pale rim crawling round.
fn orb(p: &mut Painter, c: &[Rgba; 8], t: f64, g: f64) {
    p.shade(-R - 1.0, Y - R - 1.0, R + 1.0, Y + R + 1.0, |x, y, _| {
        let (dx, dy) = (x, y - Y);
        let d = libm::sqrt(dx * dx + dy * dy);
        if d > R + 0.5 {
            return None;
        }
        let a = libm::atan2(dy, dx);
        if d > R - 0.6 {
            let f = 0.5 * libm::cos(3.0 * a - (TAU * 2.0 * t) / D)
                + 0.3 * libm::cos(7.0 * a + (TAU * 3.0 * t) / D + 1.0)
                + 0.25 * libm::cos(13.0 * a - (TAU * 5.0 * t) / D + 2.0)
                + g;
            return Some(if f > 0.5 {
                WHITE
            } else if f > 0.05 {
                c[6]
            } else if f > -0.4 {
                c[5]
            } else {
                c[4]
            });
        }
        if R - d < 1.4 && libm::cos(a - 0.8) > 0.6 {
            return Some(c[2]);
        }
        let (gx, gy) = (dx + R * 0.42, dy + R * 0.45);
        if gx * gx + gy * gy < 1.6 {
            return Some(c[3]);
        }
        Some(c[0])
    });
}

fn draw_back(p: &mut Painter, t: f64, o: &Orb) {
    let c = &o.c;
    let g = gulp(t);
    p.glow(0.0, -1.0, 52.0, c[4], 0.22 + 0.1 * g);
    vortex(p, c, &o.cells, t, g);
    lensing(p, c, t, g);
}

fn draw_front(p: &mut Painter, t: f64, o: &Orb) {
    let c = &o.c;
    let g = gulp(t);
    let s = o.seed;
    // dust drawn up off the floor, curving into the orb
    cycle(
        t,
        D,
        14,
        s + 2.0,
        |r: &mut Rng| r.range(1.6, 2.4),
        |age, l, r, _| {
            let x0 = r.range(-44.0, 44.0);
            let u = libm::pow(age / l, 1.7);
            let side = if js_sign(x0) == 0.0 { 1.0 } else { js_sign(x0) };
            let ea = core::f64::consts::FRAC_PI_2 + side * r.range(0.3, 1.0);
            let ex = libm::cos(ea) * R;
            let ey = Y + libm::sin(ea) * R;
            let mx = x0 * 0.2;
            let my = -18.0 - r.range(0.0, 12.0);
            let x = (1.0 - u) * (1.0 - u) * x0 + 2.0 * (1.0 - u) * u * mx + u * u * ex;
            let y = -((1.0 - u) * (1.0 - u)) + 2.0 * (1.0 - u) * u * my + u * u * ey;
            if age < 0.5 {
                p.disc(x0, -1.5, 2.0 + age * 6.0, c[3], 0.4 * (1.0 - age / 0.5));
            }
            let col = if u > 0.6 {
                c[7]
            } else if u > 0.25 {
                c[6]
            } else {
                c[5]
            };
            p.dot(x, y, col, js_min(1.0, age * 4.0));
            if u > 0.3 {
                lit(p, x, y, 3.0, c[5], 0.25);
            }
        },
    );
    // motes swirl in along the arms, faster as they fall, and spark on the rim where they're swallowed
    cycle(
        t,
        D,
        30,
        s,
        |r: &mut Rng| r.range(1.1, 1.8),
        |age, l, r, _| {
            let mut th0 = r.range(0.0, TAU);
            if libm::sin(th0) > 0.75 {
                th0 = -th0;
            }
            let r0 = r.range(30.0, 64.0);
            let sw = r.range(1.4, 2.6);
            let col = r
                .pick(&[WHITE, c[7], c[6], c[6], c[5]])
                .copied()
                .unwrap_or(WHITE);
            let pos = |aa: f64| {
                let u = clamp(aa / l, 0.0, 1.0);
                let k = libm::pow(u, 1.8);
                let rr = R + 1.0 + (r0 - R - 1.0) * (1.0 - k);
                let th = th0 - sw * (k + 0.5 * u);
                (libm::cos(th) * rr, Y + libm::sin(th) * rr, rr)
            };
            let (x, y, rr) = pos(age);
            let (tx, ty, _) = pos(age - 0.05);
            let a = js_min(1.0, age * 5.0);
            if rr < R + 3.0 {
                p.px(x, y, WHITE);
                lit(p, x, y, 5.0, c[6], 0.5);
                return;
            }
            p.line(tx, ty, (tx + x) / 2.0, (ty + y) / 2.0, c[4], a * 0.8);
            p.line((tx + x) / 2.0, (ty + y) / 2.0, x, y, col, a);
            lit(p, x, y, 3.0, col, 0.3 * a);
        },
    );
    // streaks: long dotted lines shot in from further out, white at the head (never from under the floor)
    cycle(
        t,
        D,
        9,
        s + 1.0,
        |_| 0.42,
        |age, l, r, _| {
            let mut th = r.range(0.0, TAU);
            if libm::sin(th) > 0.45 {
                th = -th;
            }
            let r0 = r.range(56.0, 74.0);
            let u = age / l;
            let k = (libm::exp(3.0 * u) - 1.0) / (libm::exp(3.0) - 1.0);
            let rr = r0 - (r0 - R - 1.0) * k;
            let len = 2.0 + 10.0 * k;
            let (ca, sa) = (libm::cos(th), libm::sin(th));
            let mut d = 0.0;
            while d <= len {
                let col = if d < 1.0 {
                    WHITE
                } else if d < len * 0.5 {
                    c[6]
                } else {
                    c[4]
                };
                let a = if d < len * 0.5 { 1.0 } else { 1.2 - d / len };
                p.dot(ca * (rr + d), Y + sa * (rr + d), col, a);
                d += 0.5;
            }
            lit(p, ca * rr, Y + sa * rr, 4.0, c[6], 0.4);
        },
    );
    // a gulp draws a faint ring in after it
    for gp in GULPS {
        let u = fwrap(t / D - gp) / (0.4 / D);
        if u >= 1.0 {
            continue;
        }
        let rr = R + 2.0 + (VR - R) * (1.0 - ease::in_quad(u));
        ring_fast(p, 0.0, Y, rr, 1.0, c[4], 0.35 * (1.0 - u));
        ring_light(p, 0.0, Y, rr, 4.0, c[5], 0.3 * (1.0 - u));
    }
    orb(p, c, t, g);
    halo(p, 0.0, Y, R - 0.5, R + 7.0, c[6], 0.6 + 0.3 * g);
    halo(p, 0.0, Y, R - 0.5, R + 46.0, c[4], 0.38 + 0.15 * g);
}
