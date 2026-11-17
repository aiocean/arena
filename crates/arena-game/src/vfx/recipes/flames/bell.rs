//! `holy/golden-bell`: a temple bell of golden qi drops over the fighter and lands with a white flash and a puff of
//! dust; a glint sweeps across it while it holds; a blow on its right makes it flash, wobble and ring out light;
//! then it shatters into falling shards and rising motes (the engine's `holy/golden-bell`). The back wall goes
//! behind the fighter, the front over it.

use crate::vfx::kit::{Layer, Opts, Painter, Rgba, Show, clamp, fract, libm, rand, smooth};

pub const D: f64 = 3.6;

/// Outline, shadow, base, light, highlight.
const GOLD: [Rgba; 5] = [
    Rgba::hex24(0x5a2a08),
    Rgba::hex24(0xb5560f),
    Rgba::hex24(0xe88a1e),
    Rgba::hex24(0xffc145),
    Rgba::hex24(0xfff1b0),
];
const OUTLINE: usize = 0;
const SHADOW: usize = 1;
const BASE: usize = 2;
const LIGHT: usize = 3;
const HIGHLIGHT: usize = 4;
const WHITE: Rgba = Rgba::hex24(0xffffff);
const AURA: Rgba = Rgba::hex24(0xffb040);
const DUST: Rgba = Rgba::hex24(0xb8946a);

/// Body: the top at `-BODY`, the rim's bottom on the ground at 0.
const BODY: f64 = 44.0;
/// Top rows of the waist band and the rim band, each 4 rows tall.
const WAIST: f64 = -22.0;
const RIM: f64 = -6.0;
/// Seconds: lands, the blow, shatters.
const LAND: f64 = 0.35;
const HIT: f64 = 1.4;
const SHATTER: f64 = 2.8;
/// Where the blow lands (y), on the right side.
const HIT_Y: f64 = -26.0;
const SHARDS: usize = 14;
const MOTES: usize = 18;

/// Half the body's width at `y`: a rounded top, sides widening down, a flared rim, in half-pixels.
fn half(y: f64) -> f64 {
    let v = clamp((y + BODY) / BODY, 0.0, 1.0);
    let flare = smooth(0.8, 1.0, v);
    let w = 12.0 + 7.0 * v + 4.0 * flare * flare;
    let into = clamp(y + BODY, 0.0, BODY);
    let r = 7.0;
    let w = if into < r {
        w - r + libm::sqrt((r * r - (r - into) * (r - into)).max(0.0))
    } else {
        w
    };
    libm::floor(w * 2.0 + 0.5) / 2.0
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(Bell {
        ramp: o.palette_or(&GOLD),
        seed: o.seed,
    })
}

struct Bell {
    ramp: Vec<Rgba>,
    seed: f64,
}

/// The bell's place and look at `t`.
struct Now {
    t: f64,
    up: f64,
    dx: f64,
    flash: f64,
    glint: f64,
    whole: bool,
}

fn now(t: f64) -> Now {
    let fall = clamp(t / LAND, 0.0, 1.0);
    let since = t - HIT;
    let flash_at = |at: f64, len: f64| {
        if t >= at && t < at + len {
            1.0 - (t - at) / len
        } else {
            0.0
        }
    };
    Now {
        t,
        up: libm::floor(40.0 * (1.0 - fall * fall)),
        dx: if since >= 0.0 { wobble(since) } else { 0.0 },
        flash: flash_at(LAND, 0.15)
            .max(flash_at(HIT, 0.12))
            .max(flash_at(SHATTER - 0.1, 0.1)),
        glint: fract((t - LAND) / 1.1) * 3.0 - 1.5,
        whole: t < SHATTER,
    }
}

/// Sideways wobble after the blow: away from it first, dying out, in whole pixels.
fn wobble(since: f64) -> f64 {
    libm::floor(-2.5 * libm::sin(30.0 * since) * libm::exp(-6.0 * since) + 0.5)
}

impl Show for Bell {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        let n = now(t);
        if layer == Layer::Back {
            if n.whole {
                self.wall(p, &n, true);
            }
            return;
        }
        if n.whole {
            self.glow(p, &n);
            self.wall(p, &n, false);
            self.crown(p, &n);
        }
        self.dust(p, t);
        self.ring(p, &n);
        self.shards(p, t);
    }
}

impl Bell {
    fn c(&self, i: usize) -> Rgba {
        self.ramp
            .get(i.min(self.ramp.len().saturating_sub(1)))
            .copied()
            .unwrap_or(Rgba::NONE)
    }

    /// One wall: the back (dark, see-through) or the front (banded, outlined).
    fn wall(&self, p: &mut Painter, n: &Now, back: bool) {
        let shown = smooth(0.0, 0.12, n.t);
        if shown <= 0.0 {
            return;
        }
        p.each(
            n.dx - 26.0,
            -BODY - n.up - 1.0,
            n.dx + 26.0,
            -n.up + 1.0,
            |p, lx, ly, sx, sy| {
                let y = libm::floor(ly + n.up) + 0.5;
                if !(-BODY..0.0).contains(&y) {
                    return;
                }
                let w = half(y);
                let x = lx - n.dx;
                if x.abs() > w {
                    return;
                }
                if back {
                    p.blend_px(sx, sy, self.c(SHADOW), 0.45 * shown);
                    return;
                }
                let u = x / w;
                let edge =
                    x.abs() > w - 1.0 || y < -BODY + 1.0 || y > -1.0 || x.abs() > half(y - 1.0);
                // flat bands lit from the upper left
                let mut i = if u < -0.72 {
                    BASE
                } else if u < -0.5 {
                    LIGHT
                } else if u < -0.36 {
                    HIGHLIGHT
                } else if u < 0.3 {
                    if y < -BODY + 10.0 { LIGHT } else { BASE }
                } else if u < 0.75 {
                    BASE
                } else {
                    SHADOW
                };
                // the waist band and the rim band: light lines top and bottom, dark between, diamonds on the waist
                let in_band = |y0: f64| y >= y0 && y < y0 + 4.0;
                let band = if in_band(WAIST) {
                    Some(WAIST)
                } else if in_band(RIM) {
                    Some(RIM)
                } else {
                    None
                };
                if let Some(y0) = band {
                    let row = libm::floor(y - y0);
                    i = if row == 0.0 || row == 3.0 {
                        LIGHT
                    } else {
                        SHADOW
                    };
                    if y0 == WAIST && (row == 1.0 || row == 2.0) {
                        let along = libm::asin(clamp(u, -1.0, 1.0)) * w;
                        let col = libm::floor(along + 300.0) % 6.0;
                        if col == 2.0 || col == 3.0 {
                            i = if u > 0.6 { LIGHT } else { HIGHLIGHT };
                        }
                    }
                    if u > 0.6 && i == LIGHT {
                        i = BASE;
                    }
                }
                let mut c = if edge { self.c(OUTLINE) } else { self.c(i) };
                let mut k = if edge { 0.0 } else { 0.06 };
                if !edge {
                    // a glint sweeping across while it holds, slanted
                    let g = u - n.glint - (y + BODY) * 0.02;
                    if n.t > LAND + 0.2 && n.t < SHATTER - 0.2 && g.abs() < 0.07 {
                        c = self.c(HIGHLIGHT);
                        k += 0.15;
                    }
                    // the ring of the blow passing over the surface
                    let since = n.t - HIT;
                    if (0.0..0.4).contains(&since) {
                        let d = libm::sqrt((x - w) * (x - w) + (y - HIT_Y) * (y - HIT_Y));
                        if (d - 90.0 * since).abs() < 1.5 {
                            c = self.c(HIGHLIGHT);
                            k += 0.4;
                        }
                    }
                    k += 0.3 * n.flash;
                }
                p.blend_px(sx, sy, c, if edge { shown } else { 0.85 * shown });
                if n.flash > 0.0 && !edge {
                    p.blend_px(sx, sy, WHITE, 0.7 * n.flash * n.flash * shown);
                }
                if k > 0.0 {
                    p.add(sx, sy, c, k * shown);
                }
            },
        );
    }

    /// The loop on top.
    fn crown(&self, p: &mut Painter, n: &Now) {
        let (cx, cy) = (n.dx, -BODY - n.up - 4.0);
        let shown = smooth(0.0, 0.12, n.t);
        p.each(
            cx - 5.0,
            cy - 5.0,
            cx + 5.0,
            cy + 5.0,
            |p, lx, ly, sx, sy| {
                let (x, y) = (lx - cx, ly - cy);
                let r = libm::sqrt(x * x + y * y);
                let stem = x.abs() < 2.0 && y > 2.5 && y < 5.0;
                if !stem && !(1.5..=4.5).contains(&r) {
                    return;
                }
                let edge = if stem {
                    x.abs() > 1.0
                } else {
                    !(2.5..=3.5).contains(&r)
                };
                let c = if edge {
                    self.c(OUTLINE)
                } else if x < 0.0 && y < 0.0 {
                    self.c(HIGHLIGHT)
                } else {
                    self.c(BASE)
                };
                p.blend_px(sx, sy, c, shown);
            },
        );
    }

    /// A soft warm glow round the bell and a pool of light under it.
    fn glow(&self, p: &mut Painter, n: &Now) {
        let k = smooth(LAND - 0.1, LAND + 0.2, n.t) * (0.8 + 0.2 * libm::sin(n.t * 4.0))
            + 1.5 * n.flash;
        p.glow(n.dx, -BODY * 0.55 - n.up, 34.0, AURA, 0.18 * k);
        p.glow(n.dx, -2.0, 20.0, AURA, 0.25 * k);
    }

    /// Dust puffing out from the rim where it lands.
    fn dust(&self, p: &mut Painter, t: f64) {
        let age = t - LAND;
        if !(0.0..0.45).contains(&age) {
            return;
        }
        for i in 0..12 {
            let fi = f64::from(i);
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            let speed = 40.0 + 40.0 * rand(&[fi, self.seed, 3.0]);
            let x = side * (half(-1.0) + 1.0 + speed * age * (1.0 - age));
            let y = -1.0 - libm::floor(16.0 * rand(&[fi, self.seed, 5.0]) * age);
            p.dot(libm::floor(x), y, DUST, 1.0 - age / 0.45);
        }
    }

    /// The ring of light thrown out where the blow lands.
    fn ring(&self, p: &mut Painter, n: &Now) {
        let since = n.t - HIT;
        if !(0.0..0.5).contains(&since) {
            return;
        }
        let x = half(HIT_Y) + n.dx;
        let k = 1.0 - since * 2.0;
        let r = 3.0 + 40.0 * since;
        p.glow(x, HIT_Y, 8.0, WHITE, k);
        p.ring(x, HIT_Y, r, 1.0, self.c(HIGHLIGHT), k);
        p.glow_ring(x, HIT_Y, r, 2.0, AURA, 0.5 * k);
    }

    /// The shatter: flat shards flying out and falling, and motes rising.
    fn shards(&self, p: &mut Painter, t: f64) {
        let age = t - SHATTER;
        if age < 0.0 {
            return;
        }
        for i in 0..SHARDS {
            let fi = i as f64;
            let y0 = -BODY * (0.1 + 0.85 * rand(&[fi, self.seed, 41.0]));
            let side = rand(&[fi, self.seed, 43.0]) * 2.0 - 1.0;
            let x0 = side * half(y0) * 0.8;
            let vx = side * (20.0 + 25.0 * rand(&[fi, self.seed, 45.0]));
            let vy = -25.0 - 20.0 * rand(&[fi, self.seed, 47.0]);
            let (x, y) = (x0 + vx * age, y0 + vy * age + 90.0 * age * age);
            let life = 0.6 + 0.2 * rand(&[fi, self.seed, 49.0]);
            if age > life || y > 0.0 {
                continue;
            }
            let a = 1.0 - age / life;
            let tone = if i % 3 == 0 { LIGHT } else { BASE };
            let s = 1.0 + libm::floor(2.0 * rand(&[fi, self.seed, 51.0]));
            let (x, y) = (libm::floor(x), libm::floor(y));
            p.rect(x - 1.0, y - 1.0, s + 3.0, s + 2.0, self.c(OUTLINE), a);
            p.rect(x, y, s + 1.0, s, self.c(tone), a);
            p.dot(x, y, self.c(HIGHLIGHT), a);
        }
        for i in 0..MOTES {
            let fi = i as f64;
            let m = age - 0.3 * rand(&[fi, self.seed, 61.0]);
            let life = 0.6 + 0.3 * rand(&[fi, self.seed, 63.0]);
            if m < 0.0 || m > life {
                continue;
            }
            let x =
                (rand(&[fi, self.seed, 65.0]) * 2.0 - 1.0) * 20.0 + 2.0 * libm::sin(m * 7.0 + fi);
            let y = -BODY * rand(&[fi, self.seed, 67.0]) - 35.0 * m;
            let k = 1.0 - m / life;
            p.dot(libm::floor(x), libm::floor(y), self.c(HIGHLIGHT), k);
            p.light(x, y, AURA, 0.6 * k);
        }
    }
}
