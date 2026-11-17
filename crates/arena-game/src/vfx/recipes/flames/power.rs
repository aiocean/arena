//! `feedback/power-up`: a buff in its stat's colour: streaks drawn in, the body flashing, a shock racing along the
//! floor, then an aura gripping the body's outline and licking upward, with speed streaks and motes (the engine's
//! `feedback/power-up`, a port of Sprite Fusion's power up, MIT License, (c) 2026 Hugo Duprez).
//!
//! The engine grips the outline of the character's sprite when the game hands it one; the game here hands none, so
//! it grips the engine's stand-in body (head, trunk and legs), as the engine does without an actor. The shake and
//! flash cues are left out.

use crate::vfx::kit::{
    Layer, Opts, Painter, Rgba, Rng, Show, TAU, WHITE, clamp, js_max, js_min, js_round, libm, rgb,
    scale, smooth, to_int32,
};

pub const D: f64 = 3.0;

const PALETTE: [Rgba; 7] = [
    Rgba::hex24(0xffffff),
    Rgba::hex24(0xe0fcff),
    Rgba::hex24(0x8ff0ff),
    Rgba::hex24(0x3fd0ff),
    Rgba::hex24(0x2290e6),
    Rgba::hex24(0x1f4fab),
    Rgba::hex24(0x172a68),
];
const DUSTC: Rgba = Rgba::hex24(0x6a5e82);

const BURST: f64 = 0.32;
const HOLD: f64 = 1.75;
const FADE: f64 = 2.35;
const PULSE1: f64 = BURST + 0.55;
const PULSE2: f64 = BURST + 1.0;
/// The box the body and its aura are measured in, the feet at its bottom middle: x, y, w, h.
const BOX: (i32, i32, i32, i32) = (-32, -70, 64, 76);
const MID_Y: f64 = -22.0;
/// The source's world unit in art pixels.
const PX: f64 = 2.0;
/// Chamfer costs: a straight step, a diagonal one.
const STEP: u16 = 3;
const DIAGONAL: u16 = 4;

const BAYER: [f64; 16] = [
    0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0, 3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0,
];

fn bayer(x: f64, y: f64) -> f64 {
    let (x, y) = (to_int32(x), to_int32(y));
    let i = (((y & 3) << 2) | (x & 3)) as usize;
    (BAYER.get(i).copied().unwrap_or(0.0) + 0.5) / 16.0
}

/// The source's `hash(n, k)`.
fn hash(n: f64, k: f64) -> f64 {
    let m = to_int32(n + 1.0 + k * 7919.0).wrapping_mul(to_int32(2_654_435_761.0)) as u32;
    f64::from(m % 100_000) / 100_000.0
}

/// The source's value noise in [-1, 1] over its own `hash`.
fn noise2(x: f64, y: f64, seed: f64) -> f64 {
    let (ix, iy) = (libm::floor(x), libm::floor(y));
    let (fx, fy) = (x - ix, y - iy);
    let u = fx * fx * (3.0 - 2.0 * fx);
    let v = fy * fy * (3.0 - 2.0 * fy);
    let h = |a: f64, b: f64| hash(a * 73.0 + b * 9301.0, seed) * 2.0 - 1.0;
    let top = h(ix, iy) + (h(ix + 1.0, iy) - h(ix, iy)) * u;
    let bot = h(ix, iy + 1.0) + (h(ix + 1.0, iy + 1.0) - h(ix, iy + 1.0)) * u;
    top + (bot - top) * v
}

fn out_expo(t: f64) -> f64 {
    if t >= 1.0 {
        1.0
    } else {
        1.0 - libm::pow(2.0, -10.0 * clamp(t, 0.0, 1.0))
    }
}

/// The source's `mix`: each channel `c1 * (1 - t) + c2 * t`, truncated.
fn mix(c1: Rgba, c2: Rgba, t: f64) -> Rgba {
    let u = 1.0 - t;
    let ch = |a: u8, b: u8| f64::from(a) * u + f64::from(b) * t;
    rgb(ch(c1.0, c2.0), ch(c1.1, c2.1), ch(c1.2, c2.2))
}

fn pulse(t: f64, p: f64, w: f64) -> f64 {
    libm::exp(-libm::pow((t - p) / w, 2.0))
}

/// A pixel at alpha `a`, faded by ordered dither (by local pixel).
fn dot(p: &mut Painter, x: f64, y: f64, c: Rgba, a: f64) {
    let (px, py) = (libm::floor(x), libm::floor(y));
    if a >= 1.0 || a > bayer(px, py) {
        p.px(px, py, c);
    }
}

fn disc(p: &mut Painter, cx: f64, cy: f64, r: f64, c: Rgba) {
    if r <= 0.5 {
        p.px(js_round(cx), js_round(cy), c);
        return;
    }
    let r2 = r * r;
    let mut y = libm::floor(cy - r);
    while y <= libm::ceil(cy + r) {
        let dy = y + 0.5 - cy;
        let mut x = libm::floor(cx - r);
        while x <= libm::ceil(cx + r) {
            let dx = x + 0.5 - cx;
            if dx * dx + dy * dy <= r2 {
                p.px(x, y, c);
            }
            x += 1.0;
        }
        y += 1.0;
    }
}

fn alpha_disc(p: &mut Painter, cx: f64, cy: f64, r: f64, c: Rgba, a: f64) {
    if a <= 0.01 {
        return;
    }
    let r2 = r * r;
    let mut y = libm::floor(cy - r);
    while y <= libm::ceil(cy + r) {
        let dy = y + 0.5 - cy;
        let mut x = libm::floor(cx - r);
        while x <= libm::ceil(cx + r) {
            let dx = x + 0.5 - cx;
            if dx * dx + dy * dy <= r2 {
                p.blend(x, y, c, a);
            }
            x += 1.0;
        }
        y += 1.0;
    }
}

fn line(p: &mut Painter, x0: f64, y0: f64, x1: f64, y1: f64, c: Rgba) {
    p.line(
        js_round(x0),
        js_round(y0),
        js_round(x1),
        js_round(y1),
        c,
        1.0,
    );
}

fn glow(p: &mut Painter, x: f64, y: f64, r: f64, c: Rgba, k: f64) {
    if k > 0.01 {
        p.glow(x, y, r, c, k);
    }
}

/// The stand-in body: which cells of [`BOX`] it covers and each cell's chamfer distance from it.
struct Body {
    mask: Vec<bool>,
    chamfer: Vec<u8>,
}

impl Body {
    fn new() -> Body {
        let (bx, by, w, h) = BOX;
        let (w, h) = (w as usize, h as usize);
        let mask: Vec<bool> = (0..w * h)
            .map(|k| {
                let px = f64::from(bx + (k % w) as i32) + 0.5;
                let py = f64::from(by + (k / w) as i32) + 0.5;
                let head = libm::hypot(px, py + 38.0) < 4.5;
                let trunk = px.abs() < 2.2 && py > -34.0 && py < -14.0;
                let legs = (-14.0..0.0).contains(&py) && (px.abs() - (py + 14.0) * 0.28).abs() < 1.4;
                head || trunk || legs
            })
            .collect();
        let mut dist: Vec<u8> = mask.iter().map(|&m| if m { 0 } else { 255 }).collect();
        let relax = |dist: &mut [u8], i: usize, j: usize, cost: u16| {
            if let (Some(&dj), Some(&di)) = (dist.get(j), dist.get(i)) {
                let v = u16::from(dj) + cost;
                if v < u16::from(di)
                    && let Some(d) = dist.get_mut(i)
                {
                    *d = v as u8;
                }
            }
        };
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if x > 0 {
                    relax(&mut dist, i, i - 1, STEP);
                }
                if y > 0 {
                    relax(&mut dist, i, i - w, STEP);
                    if x > 0 {
                        relax(&mut dist, i, i - w - 1, DIAGONAL);
                    }
                    if x + 1 < w {
                        relax(&mut dist, i, i - w + 1, DIAGONAL);
                    }
                }
            }
        }
        for y in (0..h).rev() {
            for x in (0..w).rev() {
                let i = y * w + x;
                if x + 1 < w {
                    relax(&mut dist, i, i + 1, STEP);
                }
                if y + 1 < h {
                    relax(&mut dist, i, i + w, STEP);
                    if x + 1 < w {
                        relax(&mut dist, i, i + w + 1, DIAGONAL);
                    }
                    if x > 0 {
                        relax(&mut dist, i, i + w - 1, DIAGONAL);
                    }
                }
            }
        }
        Body {
            mask,
            chamfer: dist,
        }
    }

    fn cell(k: usize) -> (f64, f64) {
        let (bx, by, w, _) = BOX;
        let w = w as usize;
        (
            f64::from(bx + (k % w) as i32),
            f64::from(by + (k / w) as i32),
        )
    }

    fn covers(&self, x: f64, y: f64) -> bool {
        let (bx, by, w, h) = BOX;
        let (i, j) = (x as i32 - bx, y as i32 - by);
        if i < 0 || j < 0 || i >= w || j >= h {
            return false;
        }
        self.mask.get((j * w + i) as usize).copied().unwrap_or(false)
    }

    /// A random cell of the body: up to `tries` cells drawn, the first it covers.
    fn pick(&self, r: &mut Rng, tries: u32) -> Option<(f64, f64)> {
        let (bx, by, w, h) = BOX;
        for _ in 0..tries {
            let i = r.int(0.0, f64::from(w) - 1.0);
            let j = r.int(0.0, f64::from(h) - 1.0);
            let (x, y) = (f64::from(bx) + i, f64::from(by) + j);
            if self.covers(x, y) {
                return Some((x, y));
            }
        }
        None
    }

    /// The cells off the body at most `max` from it, with their distance.
    fn around(&self, max: f64) -> impl Iterator<Item = (f64, f64, f64)> + '_ {
        self.chamfer.iter().enumerate().filter_map(move |(k, &c)| {
            let d = f64::from(c) / f64::from(STEP);
            (d != 0.0 && d <= max).then(|| {
                let (x, y) = Body::cell(k);
                (x, y, d)
            })
        })
    }

    fn cells(&self) -> impl Iterator<Item = (f64, f64)> + '_ {
        self.mask
            .iter()
            .enumerate()
            .filter(|&(_, &m)| m)
            .map(|(k, _)| Body::cell(k))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    In,
    Dust,
    Streak,
    Star,
    Wisp,
}

struct Mote {
    k: Kind,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    max: f64,
    c: Rgba,
    s: f64,
    front: bool,
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    let pal = o.palette_or(&PALETTE);
    let c = |i: usize| pal.get(i).copied().unwrap_or(WHITE);
    Box::new(Power {
        c: [c(0), c(1), c(2), c(3), c(4), c(5)],
        s: 1.0,
        r: Rng::new(o.seed as u32),
        motes: Vec::new(),
        body: Body::new(),
        t: 0.0,
        burst: false,
    })
}

struct Power {
    /// The palette's first six steps, light to dark.
    c: [Rgba; 6],
    /// How big and strong the buff is.
    s: f64,
    r: Rng,
    motes: Vec<Mote>,
    body: Body,
    t: f64,
    burst: bool,
}

impl Power {
    fn top(&mut self) -> Rgba {
        let top = [self.c[0], self.c[1], self.c[2]];
        self.r.pick(&top).copied().unwrap_or(WHITE)
    }

    fn aura(&self) -> f64 {
        let t = self.t;
        smooth(BURST - 0.04, BURST + 0.12, t)
            * (1.0 - smooth(HOLD, FADE, t))
            * (0.85 + 0.15 * libm::sin(t * 13.0))
    }

    fn flare(&self) -> f64 {
        let t = self.t;
        js_max(pulse(t, PULSE1, 0.07), pulse(t, PULSE2, 0.07)) + pulse(t, BURST, 0.08)
    }

    fn on_body(&mut self) -> (f64, f64) {
        match self.body.pick(&mut self.r, 40) {
            Some((x, y)) => (x + 0.5, y + 0.5),
            None => (f64::from(BOX.0) + f64::from(BOX.2 / 2), MID_Y),
        }
    }

    fn step(&mut self, dt: f64) {
        self.t += dt;
        let (s, c) = (self.s, self.c);
        let (ccx, ccy) = (0.0, MID_Y);
        if self.t < BURST {
            let mut i = 0.0;
            while i < js_round(3.0 * s) {
                let a = self.r.range(0.0, TAU);
                let r = self.r.range(34.0, 64.0) * libm::sqrt(s);
                let sp = self.r.range(260.0, 420.0);
                let (ca, sa) = (libm::cos(a), libm::sin(a));
                let col = if self.r.chance(0.3) { c[1] } else { c[2] };
                self.motes.push(Mote {
                    k: Kind::In,
                    x: ccx + ca * r,
                    y: ccy + sa * r * 0.8,
                    vx: -ca * sp,
                    vy: -sa * sp * 0.8,
                    life: (r * PX) / sp,
                    max: (r * PX) / sp,
                    c: col,
                    s: 1.0,
                    front: true,
                });
                i += 1.0;
            }
        }
        if !self.burst && self.t >= BURST {
            self.burst = true;
            let dust = mix(DUSTC, c[4], 0.25);
            let mut i = 0.0;
            while i < 18.0 * s {
                let side = self.r.sign();
                let x = ccx + side * self.r.range(2.0, 10.0);
                let y = -self.r.range(0.0, 3.0);
                let vx = side * self.r.range(60.0, 170.0);
                let vy = -self.r.range(2.0, 16.0);
                let life = self.r.range(0.45, 0.8);
                let size = self.r.range(1.5, 3.0);
                self.motes.push(Mote {
                    k: Kind::Dust,
                    x,
                    y,
                    vx,
                    vy,
                    life,
                    max: 0.8,
                    c: dust,
                    s: size,
                    front: false,
                });
                i += 1.0;
            }
            let mut i = 0.0;
            while i < 22.0 * s {
                let (x, y) = self.on_body();
                let vx = self.r.range(-40.0, 40.0);
                let vy = -self.r.range(30.0, 110.0);
                let life = self.r.range(0.6, 1.3);
                let col = self.top();
                let size = if self.r.chance(0.35) {
                    2.0
                } else if self.r.chance(0.5) {
                    1.0
                } else {
                    0.0
                };
                let front = self.r.chance(0.6);
                self.motes.push(Mote {
                    k: Kind::Star,
                    x,
                    y,
                    vx,
                    vy,
                    life,
                    max: 1.3,
                    c: col,
                    s: size,
                    front,
                });
                i += 1.0;
            }
        }
        let a = self.aura();
        if self.burst && a > 0.05 {
            let streaks = (5.0 + 5.0 * self.flare()) * a * s * dt * 10.0;
            let mut n = 0.0;
            while n < streaks {
                let (x, _) = self.on_body();
                let side = self.r.sign();
                let x = x + side * self.r.range(5.0, 16.0) * libm::sqrt(s);
                let y = ccy + self.r.range(-16.0, 26.0);
                let vy = -self.r.range(220.0, 420.0);
                let life = self.r.range(0.14, 0.3);
                let col = if self.r.chance(0.5) { c[2] } else { c[1] };
                let len = self.r.range(6.0, 16.0);
                let front = self.r.chance(0.5);
                self.motes.push(Mote {
                    k: Kind::Streak,
                    x,
                    y,
                    vx: 0.0,
                    vy,
                    life,
                    max: 0.3,
                    c: col,
                    s: len,
                    front,
                });
                n += 1.0;
            }
            let mut n = 0.0;
            while n < 9.0 * a * s * dt {
                n += 1.0;
                let (x, y) = self.on_body();
                let up = clamp((MID_Y - y) / 16.0, 0.0, 1.0);
                if self.r.chance(1.0 - up * 0.8) {
                    continue;
                }
                let x = x + self.r.range(-4.0, 4.0);
                let y = y - self.r.range(6.0, 12.0) * s;
                let vx = self.r.range(-15.0, 15.0);
                let vy = -self.r.range(70.0, 130.0);
                let life = self.r.range(0.3, 0.55);
                let size = self.r.range(1.4, 2.4) * libm::sqrt(s);
                self.motes.push(Mote {
                    k: Kind::Wisp,
                    x,
                    y,
                    vx,
                    vy,
                    life,
                    max: 0.55,
                    c: c[2],
                    s: size,
                    front: false,
                });
            }
            if self.r.chance(0.5 * a) {
                let (x, y) = self.on_body();
                let x = x + self.r.range(-8.0, 8.0);
                let vx = self.r.range(-10.0, 10.0);
                let vy = -self.r.range(25.0, 70.0);
                let life = self.r.range(0.6, 1.1);
                let col = self.top();
                let size = if self.r.chance(0.25) { 1.0 } else { 0.0 };
                let front = self.r.chance(0.5);
                self.motes.push(Mote {
                    k: Kind::Star,
                    x,
                    y,
                    vx,
                    vy,
                    life,
                    max: 1.1,
                    c: col,
                    s: size,
                    front,
                });
            }
        }
        self.motes.retain_mut(|m| {
            m.life -= dt;
            if m.life <= 0.0 {
                return false;
            }
            let drag = match m.k {
                Kind::Dust => 3.5,
                Kind::Star => 1.4,
                _ => 0.0,
            };
            m.vx -= m.vx * drag * dt;
            m.vy -= m.vy * drag * dt;
            if m.k == Kind::Star {
                m.vy -= 18.0 * dt;
            }
            m.x += (m.vx * dt) / PX;
            m.y += (m.vy * dt) / PX;
            true
        });
    }

    /// The aura: bands of the palette out from the body, reaching further where noise and height say.
    fn draw_aura(&self, p: &mut Painter, a: f64, fl: f64) {
        let (c, s, t) = (&self.c, self.s, self.t);
        for (px, py, d) in self.body.around(24.0) {
            let n = libm::pow(
                noise2(px * 0.5, (py + t * 34.0) * 0.26, 7.0) * 0.5 + 0.5,
                1.4,
            );
            let up = clamp((MID_Y - py) / 16.0, 0.0, 1.0);
            let reach = a * s * (3.4 + 14.0 * n * (0.3 + 1.15 * up)) * (1.0 + 0.3 * fl);
            if d > reach {
                continue;
            }
            let band = d / reach;
            if band < 0.22 {
                p.px(px, py, if fl > 0.35 && band < 0.14 { c[0] } else { c[1] });
            } else if band < 0.45 {
                p.px(px, py, c[2]);
            } else if band < 0.7 {
                p.px(px, py, c[3]);
            } else if bayer(px, py) < (1.0 - band) * 2.6 {
                p.px(px, py, if band < 0.86 { c[4] } else { c[5] });
            }
        }
    }

    /// A flash at the feet, a wave racing both ways along the floor with a crest of light, sparks behind it.
    fn shock(&self, p: &mut Painter, t: f64, z: f64) {
        let l = &self.c;
        let (cx, fy) = (0.0, 0.0);
        let big_r = 72.0 * z;
        let q = t / 0.55;
        if !(0.0..1.8).contains(&q) {
            return;
        }
        if q < 0.5 {
            let mut dx = -js_round(14.0 * z);
            while dx <= 14.0 * z {
                let u = dx.abs() / (14.0 * z);
                let a = (1.0 - q / 0.5) * (1.0 - u) * 2.2;
                let c0 = if u < 0.3 {
                    WHITE
                } else if u < 0.6 {
                    l[1]
                } else {
                    l[2]
                };
                dot(p, cx + dx, fy, c0, a);
                dot(
                    p,
                    cx + dx,
                    fy - 1.0,
                    if u < 0.4 { l[1] } else { l[2] },
                    a * 0.7,
                );
                dot(p, cx + dx, fy + 1.0, l[2], a * 0.5);
                dx += 1.0;
            }
        }
        let reach = 4.0 + big_r * out_expo(js_min(1.0, q));
        let after = 1.0 - clamp((q - 0.25) / 1.4, 0.0, 1.0);
        let f = libm::floor(t * 20.0);
        let mut dx = -js_round(reach);
        while dx <= reach {
            let u = dx.abs() / reach;
            let g = hash(js_round(cx + dx) * 7.0 + f * 613.0, 0.0) < 0.04 * after;
            let c0 = if g {
                WHITE
            } else if u < 0.5 {
                l[2]
            } else {
                l[3]
            };
            dot(
                p,
                cx + dx,
                fy,
                c0,
                if g {
                    1.0
                } else {
                    after * 0.8 * (1.0 - u * 0.5)
                },
            );
            dot(p, cx + dx, fy - 1.0, l[3], after * 0.3 * (1.0 - u));
            dx += 1.0;
        }
        if q < 1.0 {
            let head = 4.0 + big_r * out_expo(q);
            let len = (6.0 + 22.0 * (1.0 - q)) * z;
            let crest = 6.0 * z * libm::pow(1.0 - q, 0.7);
            for sd in [-1.0, 1.0] {
                let mut j = 0.0;
                while j < len {
                    let x = cx + sd * (head - j);
                    let a = (1.0 - j / len) * (1.0 - q) * 2.2;
                    let lift = js_round(crest * js_max(0.0, 1.0 - j / (len * 0.5)));
                    let c0 = if j < 2.0 {
                        WHITE
                    } else if j < len * 0.35 {
                        l[1]
                    } else if j < len * 0.7 {
                        l[2]
                    } else {
                        l[3]
                    };
                    dot(p, x, fy, c0, a);
                    let mut r = 1.0;
                    while r <= lift {
                        let c1 = if j < 2.0 && r > lift - 2.0 {
                            WHITE
                        } else if r == lift {
                            l[1]
                        } else {
                            l[2]
                        };
                        dot(p, x, fy - r, c1, a * (1.0 - r / (lift + 2.0)) * 1.4);
                        r += 1.0;
                    }
                    j += 1.0;
                }
                let mut r = 0.0;
                while r < crest + 3.0 {
                    dot(
                        p,
                        cx + sd * head,
                        fy - r,
                        WHITE,
                        (1.0 - q) * 1.6 * (1.0 - r / (crest + 3.0)),
                    );
                    r += 1.0;
                }
                glow(p, cx + sd * head, fy - 3.0, 12.0 * z, l[2], 0.8 * (1.0 - q));
            }
            glow(p, cx, fy, reach, l[3], 0.25 * (1.0 - q));
        }
        let mut i = 0.0;
        while i < js_round(24.0 * z) {
            let t0 = hash(i, 1.0) * 0.32;
            let age = t - t0;
            let life = 0.3 + hash(i, 4.0) * 0.3;
            if age < 0.0 || age > life {
                i += 1.0;
                continue;
            }
            let sd = if i % 2.0 != 0.0 { 1.0 } else { -1.0 };
            let x = cx
                + sd * (4.0 + big_r * out_expo(t0 / 0.55))
                + sd * (15.0 + hash(i, 2.0) * 70.0) * age;
            let y = fy - 1.0 - ((60.0 + hash(i, 3.0) * 90.0) * age - 110.0 * age * age);
            let k = 1.0 - age / life;
            if y > fy - 1.0 {
                i += 1.0;
                continue;
            }
            dot(
                p,
                x,
                y,
                if age < life * 0.3 { WHITE } else { l[1] },
                1.4 * k,
            );
            if hash(i, 5.0) < 0.4 && k > 0.4 {
                for (ox, oy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    dot(p, x + ox, y + oy, l[2], k * 1.2);
                }
            }
            glow(p, x, y, 4.0, l[2], 0.45 * k);
            i += 1.0;
        }
    }

    /// The body flashing on each beat, in the palette.
    fn tint(&self, p: &mut Painter, e: f64) {
        let c = &self.c;
        let colour = if e > 0.75 { c[0] } else { c[1] };
        let (mut n, mut sx, mut top, mut bot) = (0.0, 0.0, f64::INFINITY, -1.0);
        for (x, y) in self.body.cells() {
            n += 1.0;
            sx += x;
            top = js_min(top, y);
            bot = y;
            dot(p, x, y, colour, e * 1.5);
        }
        if n > 0.0 {
            glow(p, sx / n, (top + bot) / 2.0, 26.0, self.c[3], 0.45 * e);
        }
    }
}

fn draw_mote(p: &mut Painter, m: &Mote) {
    let k = m.life / m.max;
    match m.k {
        Kind::In => {
            let sp = libm::hypot(m.vx, m.vy);
            let sp = if sp == 0.0 || sp.is_nan() { 1.0 } else { sp };
            let (ux, uy) = (m.vx / sp, m.vy / sp);
            line(
                p,
                m.x - ux * 7.0,
                m.y - uy * 7.0,
                m.x - ux * 3.0,
                m.y - uy * 3.0,
                m.c,
            );
            line(p, m.x - ux * 2.5, m.y - uy * 2.5, m.x, m.y, WHITE);
        }
        Kind::Streak => {
            let l = m.s * (0.5 + 0.5 * k);
            let mut j = 0.0;
            while j <= l {
                let c = if j < 1.5 { WHITE } else { m.c };
                let a = if j < l * 0.5 {
                    1.0
                } else {
                    (1.0 - j / (l + 1.0)) * 1.6
                };
                dot(p, m.x, m.y + j, c, a);
                j += 1.0;
            }
            glow(p, m.x, m.y + l * 0.3, 4.0, m.c, 0.3 * k);
        }
        Kind::Wisp => {
            let r = m.s * (0.35 + 0.65 * k);
            disc(
                p,
                m.x,
                m.y,
                r + 0.7,
                if k > 0.5 { m.c } else { scale(m.c, 0.7) },
            );
            if k > 0.35 {
                disc(
                    p,
                    m.x - 0.3,
                    m.y - 0.4,
                    r * 0.55,
                    if k > 0.7 { WHITE } else { m.c },
                );
            }
            glow(p, m.x, m.y, 5.0 + r * 2.0, m.c, 0.3 * k);
        }
        Kind::Star => {
            let (px, py) = (libm::floor(m.x), libm::floor(m.y));
            let tw = 0.6 + 0.4 * libm::sin(m.life * 26.0 + m.x);
            if tw * k < 0.2 {
                return;
            }
            let arm = js_round(m.s * tw * js_min(1.0, k * 2.5));
            let mut d = 1.0;
            while d <= arm {
                let c = if d == arm && arm > 1.0 {
                    scale(m.c, 0.75)
                } else {
                    m.c
                };
                p.px(px + d, py, c);
                p.px(px - d, py, c);
                p.px(px, py + d, c);
                p.px(px, py - d, c);
                d += 1.0;
            }
            p.px(px, py, if tw > 0.75 { WHITE } else { m.c });
            glow(p, m.x, m.y, 3.0 + arm * 2.0, m.c, 0.45 * k * tw);
        }
        Kind::Dust => {}
    }
}

impl Show for Power {
    fn update(&mut self, dt: f64) {
        self.step(dt);
    }

    fn draw(&self, p: &mut Painter, _t: f64, layer: Layer) {
        let (a, fl, s, t) = (self.aura(), self.flare(), self.s, self.t);
        let c = &self.c;
        if layer == Layer::Back {
            for m in self.motes.iter().filter(|m| m.k == Kind::Dust) {
                let k = m.life / m.max;
                alpha_disc(p, m.x, m.y, m.s * (1.6 - k * 0.6), m.c, k * 0.6);
            }
            if a > 0.02 {
                self.draw_aura(p, a, fl);
            }
            for m in self.motes.iter().filter(|m| !m.front) {
                draw_mote(p, m);
            }
            glow(
                p,
                0.0,
                MID_Y,
                (34.0 + 20.0 * fl) * s,
                c[3],
                (0.35 * a + 0.3 * fl) * js_min(1.4, s),
            );
            glow(p, 0.0, -2.0, 30.0 * s, c[2], 0.25 * a);
            return;
        }
        if self.burst {
            self.shock(p, t - BURST, 0.45 * s);
        }
        let e = if self.burst {
            [BURST, PULSE1, PULSE2]
                .iter()
                .map(|&q| pulse(t, q, 0.07))
                .fold(f64::NEG_INFINITY, js_max)
        } else {
            0.0
        };
        if e > 0.03 {
            self.tint(p, e);
        }
        for m in self.motes.iter().filter(|m| m.front) {
            draw_mote(p, m);
        }
    }
}
