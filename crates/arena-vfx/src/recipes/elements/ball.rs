//! `lightning/ball-lightning`, after the engine's: a crackling ball of plasma drifting on a wandering loop, spitting
//! arcs off its skin, leaving a wake of fading motes, and zapping the floor once a loop when it dips low. The duel's
//! orb: played on the floor (the ball wanders 42 px over it) and moved along by the rules.

use std::collections::HashMap;

use crate::kit::sim::GRAVITY;
use crate::kit::{
    Layer, Opts, Painter, Recipe, Rgba, Rng, Show, TAU, WHITE, clamp, cycle, js_max, js_min,
    js_round, libm, or_one, ramp, rng,
};

pub const RECIPE: Recipe = Recipe::new("lightning/ball-lightning", true, 4.0, make);

const FPS: f64 = 30.0;
const N: f64 = 120.0;
const P: f64 = 4.0;
const ZAP: f64 = 62.0;

/// Deep violet shadow, electric blue, icy white.
const VOLT: [Rgba; 8] = [
    Rgba(0x12, 0x0a, 0x33, 255),
    Rgba(0x2a, 0x1b, 0x78, 255),
    Rgba(0x46, 0x30, 0xc0, 255),
    Rgba(0x5a, 0x62, 0xf0, 255),
    Rgba(0x58, 0xa6, 0xff, 255),
    Rgba(0x9e, 0xd8, 0xff, 255),
    Rgba(0xe4, 0xf6, 0xff, 255),
    Rgba(0xff, 0xff, 0xff, 255),
];
/// Plasma sparks, head to tail.
const PLASMA: [Rgba; 4] = [
    Rgba(0xff, 0xff, 0xff, 255),
    Rgba(0xc8, 0xf8, 0xff, 255),
    Rgba(0x7f, 0xf3, 0xff, 255),
    Rgba(0x1a, 0xb8, 0xe6, 255),
];

struct Ball {
    c: [Rgba; 8],
    s: [Rgba; 4],
    seed: f64,
}

fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(Ball {
        c: ramp(&o.palette_or(&VOLT)),
        s: PLASMA,
        seed: o.seed,
    })
}

impl Show for Ball {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if layer == Layer::Front {
            draw(p, t, self);
        }
    }
}

/// Where the ball is: a closed, wandering path that dips toward the floor once a loop.
fn at(t: f64) -> (f64, f64) {
    let a = (t / P) * TAU;
    (
        38.0 * libm::sin(a) + 9.0 * libm::sin(2.0 * a + 1.0),
        -42.0 + 13.0 * libm::sin(2.0 * a + 0.4) + 6.0 * libm::sin(3.0 * a + 0.5),
    )
}

fn draw(p: &mut Painter, t: f64, o: &Ball) {
    let (c, s) = (&o.c, &o.s);
    let f = libm::floor(t * FPS + 1e-6) % N;
    let mut r = rng(&[o.seed, 1.0, f]);
    let (x, y) = at(t);
    // light under the ball on the floor, stronger as it comes low
    p.glow(
        x,
        -2.0,
        30.0,
        c[3],
        0.12 + 0.3 * clamp((y + 60.0) / 40.0, 0.0, 1.0),
    );
    // the wake: where it has been, shrinking and cooling
    for i in (1..=14).rev() {
        let (wx, wy) = at(t - f64::from(i) * 0.034);
        let u = f64::from(i) / 14.0;
        let rr = 3.0 * (1.0 - u) + 0.3;
        if u > 0.6 && (f64::from(i) + f) % 2.0 != 0.0 {
            continue;
        }
        p.circle(wx, wy, rr + 0.8, if u < 0.35 { c[3] } else { c[2] }, 1.0);
        let core = if u < 0.35 {
            c[6]
        } else if u < 0.7 {
            c[5]
        } else {
            c[4]
        };
        p.circle(wx, wy, rr, core, 1.0);
        p.glow(wx, wy, 7.0 * (1.0 - u) + 3.0, c[4], 0.25 * (1.0 - u));
    }
    // motes shed along it, drifting and dying
    cycle(
        t,
        P,
        40,
        o.seed + 5.0,
        |g| g.range(0.5, 1.0),
        |age, life, q, _| {
            let born = t - age;
            let (bx, by) = at(born);
            let k = 1.0 - age / life;
            let a = q.range(0.0, TAU);
            let sp = q.range(4.0, 14.0);
            let mx = bx + libm::cos(a) * sp * age + q.range(-3.0, 3.0);
            let my = by + libm::sin(a) * sp * age + 6.0 * age + q.range(-3.0, 3.0);
            if k > 0.25 || f % 2.0 != 0.0 {
                let col = if k > 0.6 {
                    c[6]
                } else if k > 0.3 {
                    c[4]
                } else {
                    c[3]
                };
                p.px(mx, my, col);
            }
            p.glow(mx, my, 3.0, c[4], 0.3 * k);
        },
    );
    // the ball: a churning skin with hot spots rolling round it, a white heart, corona bristling off it
    let r0 = 5.2 + 0.6 * libm::sin(t * 9.0) + 0.4 * r.range(-1.0, 1.0);
    p.circle(x, y, r0 + 1.3, c[2], 1.0);
    p.circle(x, y, r0, c[4], 1.0);
    p.circle(x - 0.4, y - 0.4, r0 * 0.72, c[5], 1.0);
    for i in 0..3 {
        let fi = f64::from(i);
        let a = t * (5.0 + fi * 2.3) + fi * 2.1;
        let d = r0 * 0.6;
        p.circle(
            x + libm::cos(a) * d,
            y + libm::sin(a) * d * 0.8,
            1.2,
            if i != 0 { c[6] } else { WHITE },
            1.0,
        );
    }
    p.circle(x - 0.6, y - 0.6, r0 * 0.32, WHITE, 1.0);
    for i in 0..8 {
        if r.next() < 0.35 {
            continue;
        }
        let a = (f64::from(i) / 8.0) * TAU + t * 2.0 + r.range(-0.2, 0.2);
        let l = r.range(1.5, 4.0);
        p.line(
            x + libm::cos(a) * (r0 + 1.0),
            y + libm::sin(a) * (r0 + 1.0),
            x + libm::cos(a) * (r0 + 1.0 + l),
            y + libm::sin(a) * (r0 + 1.0 + l),
            if i % 2 != 0 { c[5] } else { c[4] },
            1.0,
        );
    }
    p.glow(x, y, 30.0, c[4], 0.3);
    p.glow(x, y, 9.0, c[6], 0.1);
    // arcs spat off its skin, re-struck every frame
    let mut i = 0;
    loop {
        if i >= 2 + i32::from(r.next() < 0.4) {
            break;
        }
        i += 1;
        let a = r.range(0.0, TAU);
        let l = r.range(5.0, 11.0);
        let x0 = x + libm::cos(a) * (r0 + 1.0);
        let y0 = y + libm::sin(a) * (r0 + 1.0);
        let b = a + r.range(-0.7, 0.7);
        let bolt = zigzag(
            &mut r,
            x0,
            y0,
            x0 + libm::cos(b) * l,
            y0 + libm::sin(b) * l,
            0.4,
            0.3,
        );
        let k = r.range(0.4, 0.7);
        crackle(p, &bolt, k, c, 0.25);
    }
    // once a loop, low over the floor, it zaps down: a bolt, the floor lit, sparks kicked up
    let za = f - ZAP;
    let (zx, _) = at(ZAP / FPS);
    let gx = zx + rng(&[o.seed, 3.0]).range(-10.0, 10.0);
    if (0.0..4.0).contains(&za) {
        let k = [1.0, 0.6, 0.9, 0.4]
            .get(za as usize)
            .copied()
            .unwrap_or(0.4);
        let bolt = zigzag(&mut rng(&[o.seed, 4.0, f]), x, y + r0, gx, 0.0, 0.28, 0.4);
        stroke(p, &bolt, k, c, 0.7, za == 0.0);
        p.circle(gx, -1.0, 2.5 * k + 1.0, c[5], 1.0);
        p.px(gx, -1.0, WHITE);
        p.glow(gx, -4.0, 34.0, c[4], 0.6 * k);
    }
    let sa = (((t - ZAP / FPS) % P) + P) % P;
    for j in 0..14 {
        let mut q = rng(&[o.seed, 7.0, f64::from(j)]);
        let life = q.range(0.25, 0.55);
        if sa >= life {
            continue;
        }
        let a = -core::f64::consts::FRAC_PI_2 + q.range(-1.3, 1.3);
        let sp = q.range(50.0, 170.0);
        let (sx, sy, vx, vy) = flight(gx, -1.0, libm::cos(a) * sp, libm::sin(a) * sp, sa, 0.0);
        spark(p, sx, sy, vx, vy, 1.0 - sa / life, s);
    }
}

// ---- the lightning helpers (the engine's `lightning/lib.rs`)

/// One branch of a bolt: its points and its width (1 for the trunk, thinner for forks).
struct Path {
    pts: Vec<(f64, f64)>,
    w: f64,
}

/// A bolt: its paths, the trunk first.
struct Bolt {
    paths: Vec<Path>,
}

/// A bolt from (x0, y0) to (x1, y1) by midpoint displacement down to ~3 px segments, with forks.
fn zigzag(r: &mut Rng, x0: f64, y0: f64, x1: f64, y1: f64, jag: f64, forks: f64) -> Bolt {
    Bolt {
        paths: zig(r, x0, y0, x1, y1, jag, forks, 1.0, 0),
    }
}

#[allow(clippy::too_many_arguments)]
fn zig(
    r: &mut Rng,
    x0: f64,
    y0: f64,
    x1: f64,
    y1: f64,
    jag: f64,
    forks: f64,
    w: f64,
    depth: u32,
) -> Vec<Path> {
    let l = libm::hypot(x1 - x0, y1 - y0);
    let mut pts = vec![(x0, y0), (x1, y1)];
    let mut off = l * jag;
    while (pts.len() as f64) < 2.0 + l / 3.0 {
        let mut next = vec![(x0, y0)];
        for pair in pts.windows(2) {
            let (&(ax, ay), &(bx, by)) = match (pair.first(), pair.get(1)) {
                (Some(a), Some(b)) => (a, b),
                _ => continue,
            };
            let hl = libm::hypot(bx - ax, by - ay);
            let hl = if hl == 0.0 { 1.0 } else { hl };
            let o = r.range(-off, off);
            next.push((
                (ax + bx) / 2.0 + (-(by - ay) / hl) * o,
                (ay + by) / 2.0 + ((bx - ax) / hl) * o,
            ));
            next.push((bx, by));
        }
        pts = next;
        off *= 0.52;
    }
    let n = pts.len();
    let mut forked = Vec::new();
    if depth < 2 {
        let d = f64::from(depth);
        let mut i = 2;
        while i + 3 < n {
            if r.chance(forks / (1.0 + d) / js_max(4.0, n as f64 / 6.0))
                && let (Some(&(ax, ay)), Some(&(bx, by))) =
                    (pts.get(i), pts.get((n - 1).min(i + 3)))
            {
                let a = libm::atan2(by - ay, bx - ax) + r.sign() * r.range(0.35, 0.9);
                let len = (l - (i as f64 / n as f64) * l) * r.range(0.25, 0.55);
                forked.extend(zig(
                    r,
                    ax,
                    ay,
                    ax + libm::cos(a) * len,
                    ay + libm::sin(a) * len,
                    jag * 1.1,
                    forks,
                    w * 0.55,
                    depth + 1,
                ));
            }
            i += 1;
        }
    }
    let mut out = vec![Path { pts, w }];
    out.extend(forked);
    out
}

/// Pixels gathered at a level (higher wins), then drawn once each, so a fork's halo never covers the trunk's body.
#[derive(Default)]
struct Ink {
    level: HashMap<(i64, i64), u8>,
    hit: Vec<(i64, i64)>,
}

impl Ink {
    fn mark(&mut self, x: f64, y: f64, lvl: u8) {
        let (ix, iy) = (libm::floor(x) as i64, libm::floor(y) as i64);
        if !(-512..512).contains(&ix) || !(-512..512).contains(&iy) {
            return;
        }
        let v = self.level.entry((ix, iy)).or_insert(0);
        if *v == 0 {
            self.hit.push((ix, iy));
        }
        if lvl > *v {
            *v = lvl;
        }
    }

    fn stamp(&mut self, x: f64, y: f64, r: f64, lvl: u8) {
        if r <= 0.5 {
            return self.mark(js_round(x), js_round(y), lvl);
        }
        let r2 = r * r;
        let mut py = libm::floor(y - r);
        while py <= libm::ceil(y + r) {
            let dy = py + 0.5 - y;
            let mut px = libm::floor(x - r);
            while px <= libm::ceil(x + r) {
                let dx = px + 0.5 - x;
                if dx * dx + dy * dy <= r2 {
                    self.mark(px, py, lvl);
                }
                px += 1.0;
            }
            py += 1.0;
        }
    }

    /// Level n in `cols[n - 1]` at alpha `alphas[n - 1]`.
    fn flush(self, p: &mut Painter, cols: [Rgba; 4], alphas: [f64; 4]) {
        for (x, y) in self.hit {
            let v = usize::from(self.level.get(&(x, y)).copied().unwrap_or(0));
            let (Some(&c), Some(&a)) = (cols.get(v.wrapping_sub(1)), alphas.get(v.wrapping_sub(1)))
            else {
                continue;
            };
            let (x, y) = (x as f64, y as f64);
            if a >= 1.0 {
                p.px(x, y, c);
            } else if a > 0.0 {
                p.dot(x, y, c, a);
            }
        }
    }
}

/// Visit points half a pixel apart along each segment of `pts`.
fn walk(pts: &[(f64, f64)], mut f: impl FnMut(f64, f64)) {
    for pair in pts.windows(2) {
        let (&(ax, ay), &(bx, by)) = match (pair.first(), pair.get(1)) {
            (Some(a), Some(b)) => (a, b),
            _ => continue,
        };
        let l = js_max(1.0, libm::hypot(bx - ax, by - ay));
        let mut d = 0.0;
        while d <= l {
            f(ax + ((bx - ax) * d) / l, ay + ((by - ay) * d) / l);
            d += 0.5;
        }
    }
}

/// A bolt at full power: a deep halo, a pale body, a white core on the trunk, light every few points.
fn stroke(p: &mut Painter, b: &Bolt, k: f64, c: &[Rgba; 8], s: f64, hero: bool) {
    if k <= 0.02 {
        return;
    }
    let mut ink = Ink::default();
    for q in &b.paths {
        let trunk = q.w >= 1.0;
        if !trunk && k < 0.5 {
            continue;
        }
        let rh = (if trunk {
            if k > 0.6 { 2.1 } else { 1.6 }
        } else {
            1.1
        }) * s;
        let rb = (if trunk {
            if k > 0.6 { 1.3 } else { 0.8 }
        } else {
            0.5
        }) * s;
        walk(&q.pts, |x, y| {
            ink.stamp(x, y, rh, 1);
            ink.stamp(x, y, rb, if trunk { 3 } else { 2 });
            if trunk && k > 0.3 {
                ink.mark(x, y, 4);
            }
        });
        for pt in q.pts.iter().take(q.pts.len().saturating_sub(1)).step_by(3) {
            let r = if trunk { 14.0 } else { 7.0 };
            p.glow(pt.0, pt.1, r * s, c[4], 0.5 * k * q.w);
        }
    }
    let cols = if hero {
        [c[3], c[6], c[7], c[7]]
    } else {
        [c[2], c[4], c[5], c[7]]
    };
    ink.flush(p, cols, [1.0; 4]);
}

const CROSS: [(f64, f64); 4] = [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)];

/// A thin crackle for short arcs that live a frame or two.
fn crackle(p: &mut Painter, b: &Bolt, k: f64, c: &[Rgba; 8], glow_k: f64) {
    if k <= 0.02 {
        return;
    }
    let mut ink = Ink::default();
    for q in &b.paths {
        let thick = q.w >= 1.0;
        walk(&q.pts, |x, y| {
            if k > 0.45 && thick {
                for (dx, dy) in CROSS {
                    ink.mark(x + dx * 1.5, y + dy * 1.5, 1);
                }
            }
            if k > 0.25 && (thick || q.w > 0.4) {
                ink.mark(x + 1.0, y, 2);
                ink.mark(x, y + 1.0, 2);
            }
            ink.mark(x, y, if thick && k > 0.3 { 4 } else { 3 });
        });
        for pt in q.pts.iter().take(q.pts.len().saturating_sub(1)).step_by(3) {
            let r = if thick { 9.0 } else { 5.0 };
            p.glow(pt.0, pt.1, r * (0.5 + k * 0.5), c[4], glow_k * k * q.w);
        }
    }
    let halo = js_min(1.0, (k - 0.45) * 1.6);
    ink.flush(p, [c[3], c[5], c[5], c[7]], [halo, 1.0, 1.0, 1.0]);
}

/// Where a spark launched at (x, y, vx, vy) is after `age` seconds (gravity, drag, a bounce on the floor).
fn flight(x: f64, y: f64, vx: f64, vy: f64, age: f64, floor: f64) -> (f64, f64, f64, f64) {
    const G: f64 = 0.45;
    const DRAG: f64 = 2.2;
    const BOUNCE: f64 = 0.45;
    let dt = 1.0 / 120.0;
    let (mut x, mut y, mut vx, mut vy) = (x, y, vx, vy);
    let mut s = 0.0;
    while s < age {
        let h = js_min(dt, age - s);
        vy += GRAVITY * G * h;
        vx -= vx * DRAG * h;
        vy -= vy * DRAG * h;
        let ny = y + vy * h;
        x += vx * h;
        if vy > 0.0 && y <= floor && ny >= floor {
            vy = -vy * BOUNCE;
            vx *= 0.7;
            y = floor - 0.25;
        } else {
            y = ny;
        }
        s += dt;
    }
    (x, y, vx, vy)
}

/// A spark: a streak stretched by speed, a hot head, light.
fn spark(p: &mut Painter, x: f64, y: f64, vx: f64, vy: f64, k: f64, s: &[Rgba; 4]) {
    let sp = or_one(libm::hypot(vx, vy));
    let (ux, uy) = (vx / sp, vy / sp);
    let head = if k > 0.6 {
        s[0]
    } else if k > 0.3 {
        s[1]
    } else {
        s[2]
    };
    let len = js_min(9.0, 1.0 + sp * 0.022);
    let tail = if k > 0.5 { head } else { s[3] };
    p.line(
        x - ux * len,
        y - uy * len,
        x - ux * len * 0.4,
        y - uy * len * 0.4,
        tail,
        1.0,
    );
    p.line(x - ux * len * 0.45, y - uy * len * 0.45, x, y, head, 1.0);
    p.glow(x, y, 3.0, head, 0.45 * k);
}
