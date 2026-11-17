//! `holy/lotus`: a pad opens under the feet and five pink-white petals of light rise from it and fold into a tall
//! bud round the fighter; it breathes with specks rising inside; a blow on its right makes that side shiver and
//! flash and throws out pollen; then the bud opens and its petals drift away, fading, the pad last (the engine's
//! `holy/lotus`). The back petals go behind the fighter, the front ones over it.

use crate::kit::{Layer, Opts, PI, Painter, Rgba, Show, clamp, libm, rand, smooth};

pub const D: f64 = 4.2;

/// Outline, shade, body, pale, cream, rim, pad gold.
const PINK: [Rgba; 7] = [
    Rgba::hex24(0xf84cba),
    Rgba::hex24(0xfb82c9),
    Rgba::hex24(0xfdaad4),
    Rgba::hex24(0xfdc9e0),
    Rgba::hex24(0xfde2d3),
    Rgba::hex24(0xfdf6f7),
    Rgba::hex24(0xfdbaa4),
];
const OUTLINE: usize = 0;
const SHADE: usize = 1;
const BODY: usize = 2;
const PALE: usize = 3;
const CREAM: usize = 4;
const RIM: usize = 5;
const GOLD: usize = 6;
const GLOW: Rgba = Rgba::hex24(0xfff3cc);
const PINK_GLOW: Rgba = Rgba::hex24(0xff8fd0);

/// Seconds: the bud is closed, the blow, the bud starts to open, gone.
const CLOSED: f64 = 0.9;
const HIT: f64 = 1.6;
const OPEN: f64 = 2.7;
const GONE: f64 = 4.0;
const HIT_AT: (f64, f64) = (28.0, -36.0);
const FLOATERS: usize = 10;

/// A petal of the bud: base x, lean closed and open (0 up, positive right), curl when closed, length, half width,
/// behind the fighter?, order in rising.
struct Leaf {
    x: f64,
    closed: f64,
    open: f64,
    curl: f64,
    len: f64,
    half: f64,
    back: bool,
    order: f64,
}

const LEAVES: [Leaf; 5] = [
    Leaf {
        x: 0.0,
        closed: 0.0,
        open: 0.0,
        curl: 0.0,
        len: 66.0,
        half: 12.0,
        back: true,
        order: 1.0,
    },
    Leaf {
        x: -10.0,
        closed: -0.6,
        open: -1.2,
        curl: 0.5,
        len: 46.0,
        half: 12.0,
        back: true,
        order: 0.0,
    },
    Leaf {
        x: 10.0,
        closed: 0.6,
        open: 1.2,
        curl: -0.5,
        len: 46.0,
        half: 12.0,
        back: true,
        order: 0.0,
    },
    Leaf {
        x: -7.0,
        closed: -0.38,
        open: -0.9,
        curl: 0.6,
        len: 50.0,
        half: 19.0,
        back: false,
        order: 2.0,
    },
    Leaf {
        x: 7.0,
        closed: 0.38,
        open: 0.9,
        curl: -0.6,
        len: 50.0,
        half: 19.0,
        back: false,
        order: 2.0,
    },
];

/// A petal placed at `t`.
struct Placed {
    bx: f64,
    by: f64,
    lean: f64,
    curl: f64,
    len: f64,
    half: f64,
    alpha: f64,
    flash: f64,
}

const WIDEST: f64 = 0.5;

/// The petal's half width along it: a round base, widest at [`WIDEST`], a long taper to a point.
fn lens(v: f64) -> f64 {
    if v <= 0.0 || v >= 1.0 {
        0.0
    } else if v < WIDEST {
        let k = (WIDEST - v) / WIDEST;
        libm::sqrt(1.0 - k * k)
    } else {
        libm::pow((1.0 - v) / (1.0 - WIDEST), 0.6)
    }
}

/// The petal's tones, white to deep pink, and the peach that warms its base.
const PETAL: [Rgba; 6] = [
    Rgba::hex24(0xfdf7f6),
    Rgba::hex24(0xfdd4ef),
    Rgba::hex24(0xfdc6eb),
    Rgba::hex24(0xfcaae6),
    Rgba::hex24(0xfc8fe7),
    Rgba::hex24(0xfc6ce1),
];
const PEACH: Rgba = Rgba::hex24(0xfde8d3);
const HALO: f64 = 3.0;

/// A 4 x 4 ordered threshold (0..1) at screen pixel (`sx`, `sy`).
fn bayer(sx: f64, sy: f64) -> f64 {
    const M: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    let i =
        (libm::floor(sy).rem_euclid(4.0) as usize) * 4 + libm::floor(sx).rem_euclid(4.0) as usize;
    (f64::from(M.get(i).copied().unwrap_or(0)) + 0.5) / 16.0
}

fn sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        0.0
    }
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    Box::new(Lotus {
        ramp: o.palette_or(&PINK),
        seed: o.seed,
    })
}

struct Lotus {
    ramp: Vec<Rgba>,
    seed: f64,
}

impl Show for Lotus {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        let back = layer == Layer::Back;
        if back {
            self.pad(p, t);
        }
        for leaf in &LEAVES {
            if leaf.back == back
                && let Some(at) = place(leaf, t)
            {
                self.petal(p, &at);
            }
        }
        if back {
            return;
        }
        self.light(p, t);
        self.floaters(p, t);
        self.specks(p, t);
        self.burst(p, t);
    }
}

/// Where a petal of the bud is at `t`, or nothing before it rises or once it has come loose.
fn place(leaf: &Leaf, t: f64) -> Option<Placed> {
    let start = 0.12 + 0.17 * leaf.order;
    let rise = smooth(start, start + 0.45, t);
    let loose = smooth(OPEN + 0.35, OPEN + 0.6, t);
    if rise <= 0.0 || loose >= 1.0 {
        return None;
    }
    let side = sign(leaf.closed);
    let mut lean = leaf.open + (leaf.closed - leaf.open) * rise;
    let mut curl = leaf.curl * rise;
    let len = leaf.len * (0.3 + 0.7 * rise);
    lean += 0.03 * side * libm::sin(t * 3.0 + leaf.order) * smooth(CLOSED - 0.2, CLOSED, t);
    let since = t - HIT;
    let mut flash = 0.0;
    if since >= 0.0 && side >= 0.0 {
        let k = if side > 0.0 { 1.0 } else { 0.5 };
        lean -= k * 0.08 * libm::sin(40.0 * since) * libm::exp(-7.0 * since);
        flash = k * clamp(1.0 - since / 0.2, 0.0, 1.0);
    }
    let opening = smooth(OPEN, OPEN + 0.4, t);
    lean += (leaf.open * 0.85 - leaf.closed) * opening;
    curl += (-0.12 * side - leaf.curl) * opening;
    Some(Placed {
        bx: leaf.x * (1.0 + 0.3 * opening),
        by: -2.0,
        lean,
        curl,
        len,
        half: leaf.half * (0.55 + 0.45 * rise),
        alpha: smooth(0.0, 0.3, rise) * (1.0 - loose),
        flash,
    })
}

impl Lotus {
    fn c(&self, i: usize) -> Rgba {
        self.ramp
            .get(i.min(self.ramp.len().saturating_sub(1)))
            .copied()
            .unwrap_or(Rgba::NONE)
    }

    /// One petal: outline, white rim, body tones from the base up, a pale vein, a glow round it.
    fn petal(&self, p: &mut Painter, at: &Placed) {
        let (dx, dy) = (libm::sin(at.lean), -libm::cos(at.lean));
        let (px, py) = (-dy, dx);
        let bend = at.curl * at.len;
        let (tx, ty) = (
            at.bx + dx * at.len + px * bend,
            at.by + dy * at.len + py * bend,
        );
        let r = at.half + HALO + 1.0;
        let (x0, x1) = (at.bx.min(tx) - r, at.bx.max(tx) + r);
        let (y0, y1) = (at.by.min(ty) - r, at.by.max(ty) + r);
        let field = |x: f64, y: f64| -> (f64, f64, f64) {
            let (rx, ry) = (x - at.bx, y - at.by);
            let along = rx * dx + ry * dy;
            let v = along / at.len;
            let vc = clamp(v, 0.0, 1.0);
            let across = rx * px + ry * py - bend * vc * vc;
            let hw = at.half * lens(vc);
            let over = (-along).max(along - at.len).max(0.0);
            (vc, across / hw.max(0.5), hw - across.abs() - over)
        };
        let inside = |x: f64, y: f64| field(x, y).2 >= 0.0;
        p.each(x0, y0, x1, y1, |p, lx, ly, sx, sy| {
            let (v, u, room) = field(lx, ly);
            if room < 0.0 {
                if room > -HALO {
                    let k = 1.0 + room / HALO;
                    p.add(
                        sx,
                        sy,
                        PINK_GLOW,
                        (0.12 + 0.2 * at.flash) * k * k * at.alpha,
                    );
                }
                return;
            }
            let edge = room < 1.0
                || !inside(lx, ly - 1.0)
                || !inside(lx, ly + 1.0)
                || !inside(lx - 1.0, ly)
                || !inside(lx + 1.0, ly);
            let rim = !edge && room < 2.0;
            let mut tone = 0.8 + 1.8 * libm::pow(v, 1.4) + 1.6 * u * u;
            let vein = (0.1 + 0.25 * (1.0 - v)) - u.abs();
            if vein > 0.0 && v < 0.8 {
                tone -= 1.6 * (vein / 0.35).min(1.0) + 0.4;
            }
            tone += 0.5
                * (rand(&[
                    libm::floor(u * at.half / 2.0),
                    libm::floor(v * at.len / 4.0),
                    at.len,
                ]) - 0.5);
            tone -= 1.5 * at.flash;
            let tone = if edge {
                5.0 - at.flash
            } else if rim {
                0.0
            } else {
                clamp(tone, 0.0, 3.4)
            };
            let base = libm::floor(tone);
            let step = if tone - base > bayer(sx, sy) {
                base + 1.0
            } else {
                base
            };
            let c = PETAL
                .get(clamp(step, 0.0, 5.0) as usize)
                .copied()
                .unwrap_or(Rgba::NONE);
            let warm = !edge && !rim && v < 0.3 && step <= 1.0 && rand(&[sx, sy, at.len]) < 0.35;
            p.blend_px(sx, sy, if warm { PEACH } else { c }, at.alpha);
            p.add(
                sx,
                sy,
                if rim { GLOW } else { PINK_GLOW },
                (if rim { 0.1 } else { 0.0 } + 0.1 * at.flash) * at.alpha,
            );
        });
    }

    /// The pad under the feet: a flat scalloped rosette with a pale inner ring and a warm core.
    fn pad(&self, p: &mut Painter, t: f64) {
        let grow = smooth(0.0, 0.15, t);
        let fade = 1.0 - smooth(GONE, GONE + 0.2, t);
        if grow <= 0.0 || fade <= 0.0 {
            return;
        }
        let (rx, ry) = (42.0 * (0.3 + 0.7 * grow), 10.0 * (0.3 + 0.7 * grow));
        let shape = |x: f64, y: f64| -> f64 {
            let a = libm::atan2(y / ry, x / rx);
            let scallop = 1.0 - 0.2 * libm::cos(4.5 * a).abs();
            libm::sqrt((x / rx) * (x / rx) + (y / ry) * (y / ry)) / scallop
        };
        p.each(
            -rx - 1.0,
            -ry - 1.0,
            rx + 1.0,
            ry + 1.0,
            |p, lx, ly, sx, sy| {
                let (x, y) = (lx, ly + 1.0);
                let d = shape(x, y);
                if d > 1.0 {
                    return;
                }
                let edge = shape(x + 1.0, y) > 1.0
                    || shape(x - 1.0, y) > 1.0
                    || shape(x, y + 1.0) > 1.0
                    || shape(x, y - 1.0) > 1.0;
                let i = if edge {
                    OUTLINE
                } else if d > 0.82 {
                    SHADE
                } else if d > 0.66 {
                    BODY
                } else if d > 0.56 {
                    RIM
                } else if d > 0.32 {
                    PALE
                } else {
                    GOLD
                };
                p.blend_px(sx, sy, self.c(i), fade * if edge { 1.0 } else { 0.92 });
                if d < 0.6 {
                    p.add(sx, sy, GLOW, 0.15 * fade);
                }
            },
        );
    }

    /// The warm light inside the bud, a flare at the blow, and a column of it rising as the bud opens.
    fn light(&self, p: &mut Painter, t: f64) {
        let held = smooth(0.3, CLOSED, t) * (1.0 - smooth(OPEN + 0.8, GONE, t));
        let flare = if t >= HIT {
            0.25 * clamp(1.0 - (t - HIT) / 0.25, 0.0, 1.0)
        } else {
            0.0
        };
        let k = held * (0.85 + 0.15 * libm::sin(t * 3.0)) + flare;
        p.glow(0.0, -26.0, 30.0, GLOW, 0.15 * k);
        p.glow(0.0, -12.0, 14.0, GLOW, 0.4 * k);
        let column = smooth(OPEN + 0.1, OPEN + 0.5, t) * (1.0 - smooth(OPEN + 0.9, GONE - 0.1, t));
        if column > 0.0 {
            p.glow_line(0.0, -4.0, 0.0, -56.0, 9.0, GLOW, 0.55 * column);
        }
    }

    /// Small petals drifting up and out once the bud has opened, turning slowly, fading near the top.
    fn floaters(&self, p: &mut Painter, t: f64) {
        let age = t - (OPEN + 0.35);
        if age < 0.0 {
            return;
        }
        for i in 0..FLOATERS {
            let fi = i as f64;
            let m = age - 0.06 * fi;
            let life = 0.9 + 0.3 * rand(&[fi, self.seed, 31.0]);
            if m < 0.0 || m > life {
                continue;
            }
            let side = if i % 2 == 0 { -1.0 } else { 1.0 };
            let spread = 0.3 + 0.7 * rand(&[fi, self.seed, 33.0]);
            let x =
                side * (8.0 + 34.0 * spread * smooth(0.0, 0.6, m)) + 3.0 * libm::sin(m * 4.0 + fi);
            let y = -14.0 - 22.0 * rand(&[fi, self.seed, 35.0]) - 40.0 * m;
            self.petal(
                p,
                &Placed {
                    bx: x,
                    by: y,
                    lean: side * (0.6 + 0.8 * spread) + 1.5 * side * m,
                    curl: 0.0,
                    len: 16.0 - 4.0 * m,
                    half: 4.6,
                    alpha: smooth(0.0, 0.08, m) * (1.0 - smooth(life * 0.6, life, m)),
                    flash: 0.0,
                },
            );
        }
    }

    /// Specks of light rising round and inside the bud, every third one a sparkle.
    fn specks(&self, p: &mut Painter, t: f64) {
        let on = smooth(0.0, 0.15, t) * (1.0 - smooth(GONE - 0.2, GONE + 0.15, t));
        if on <= 0.0 {
            return;
        }
        for i in 0..14 {
            let fi = f64::from(i);
            let period = 1.4 + 0.8 * rand(&[fi, self.seed, 7.0]);
            let phase = libm::fmod(t / period + rand(&[fi, self.seed, 9.0]), 1.0);
            let x = libm::floor(
                (rand(&[fi, self.seed, 11.0]) * 2.0 - 1.0) * 36.0 + 1.5 * libm::sin(t * 2.0 + fi),
            );
            let y = libm::floor(-4.0 - 66.0 * phase);
            let k = on * libm::sin(PI * phase);
            p.blend(x, y, self.c(RIM), k);
            if i % 3 == 0 {
                for (ox, oy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                    p.blend(x + ox, y + oy, self.c(CREAM), 0.8 * k);
                }
                for (ox, oy) in [(-2.0, 0.0), (2.0, 0.0), (0.0, -2.0), (0.0, 2.0)] {
                    p.blend(x + ox, y + oy, self.c(PALE), 0.5 * k);
                }
            }
            p.light(x, y, GLOW, 0.6 * k);
        }
    }

    /// The blow: streaks of light flung out to the right, glowing pollen and small petals with them.
    fn burst(&self, p: &mut Painter, t: f64) {
        let age = t - HIT;
        if !(0.0..0.8).contains(&age) {
            return;
        }
        let (hx, hy) = HIT_AT;
        let k = 1.0 - age / 0.8;
        p.glow(hx, hy, 10.0 * (1.0 - 0.5 * age), GLOW, 0.9 * k);
        if age < 0.3 {
            for i in 0..6 {
                let a = (f64::from(i) - 2.5) * 0.32;
                let (r0, r1) = (4.0 + 30.0 * age, 10.0 + 60.0 * age);
                let (c, s) = (libm::cos(a), libm::sin(a));
                p.glow_line(
                    hx + c * r0,
                    hy + s * r0,
                    hx + c * r1,
                    hy + s * r1,
                    1.5,
                    GLOW,
                    1.0 - age / 0.3,
                );
            }
        }
        for i in 0..14 {
            let fi = f64::from(i);
            let a = (rand(&[fi, self.seed, 21.0]) - 0.5) * 1.8;
            let speed = 35.0 + 55.0 * rand(&[fi, self.seed, 23.0]);
            let d = speed * age * (1.0 - 0.45 * age);
            let (x, y) = (
                hx + libm::cos(a) * d,
                hy + libm::sin(a) * d + 12.0 * age * age,
            );
            if i % 3 == 0 {
                self.petal(
                    p,
                    &Placed {
                        bx: x,
                        by: y,
                        lean: 1.2 + a + 3.0 * age,
                        curl: 0.0,
                        len: 8.0,
                        half: 2.6,
                        alpha: k,
                        flash: 0.0,
                    },
                );
            } else {
                let (x, y) = (libm::floor(x), libm::floor(y));
                p.blend(x, y, self.c(RIM), k);
                p.blend(x + 1.0, y, self.c(CREAM), 0.6 * k);
                p.light(x, y, GLOW, 0.7 * k);
            }
        }
    }
}
