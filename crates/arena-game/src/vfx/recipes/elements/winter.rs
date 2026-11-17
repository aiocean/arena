//! `shadow/winter-ordeal`, after the engine's: over the foe the night sky darkens and cracks like glass: cracks run
//! out from a keystone in its middle, where an eye opens and looks down, and rings of cracks cut it into slabs. Then
//! the sky falls: ring by ring the slabs tremble, drop, turn and smash on the ground into shards that crumble to dust,
//! and last the keystone with the open eye falls onto the foe's head at 1.78 s (the rules' impact). A black drift is
//! left round the feet and melts into shadow. Played on the floor under the foe.
//!
//! The engine bakes its sky once (`winter_ordeal_sky.rs`, ~2000 lines of slab pictures and crack times); here the
//! same kind of sky is laid out at spawn from a few rules instead: an oval 200 by 80 px centred 112 px up, a keystone
//! and three rings cut into 5, 7 and 10 slabs along wavering lines, each slab split into shards round its middle, the
//! cracks the one-pixel seams between slabs, each cracking open at a time that runs outward from the keystone. The
//! foe it falls on is this game's fighter (61 px tall), not the engine's ruler figure.

use crate::vfx::kit::halo::Halo;
use crate::vfx::kit::{
    Layer, Opts, Painter, Recipe, Rgba, Show, TAU, clamp, fbm, libm, rand, smooth, wrap,
};

pub const RECIPE: Recipe = Recipe::new("shadow/winter-ordeal", false, 2.8, make);

const W: usize = 224;
const H: usize = 176;
const OX: f64 = 112.0;
const OY: f64 = 168.0;

/// The sky's middle (the eye) and half size.
const SKY: (f64, f64) = (0.0, -112.0);
const HALF: (f64, f64) = (100.0, 40.0);

/// The sky darkens in.
const GROW: (f64, f64) = (0.0, 0.45);
/// The eye opens in the keystone, and shuts as it strikes.
const OPEN: (f64, f64) = (0.8, 1.15);
/// The slabs of each ring start to fall at these times (inner, middle, outer), a little apart within a ring; each
/// trembles for TREMBLE before.
const FALL_AT: [f64; 3] = [1.22, 1.34, 1.46];
const FALL_SPREAD: f64 = 0.1;
const TREMBLE: f64 = 0.12;
/// Gravity, px/s².
const GRAVITY: f64 = 1100.0;
/// The keystone strikes the foe at BLOW.
const BLOW: f64 = 1.78;
/// A smashed slab's shards fly this long, the last SHATTER_DUST of it crumbling to dust.
const SHATTER: f64 = 0.5;
const SHATTER_DUST: f64 = 0.2;
/// The drift lies, then melts away by the end.
const MELT: (f64, f64) = (2.1, 2.75);

/// The foe: this game's fighter, about 61 px tall and 30 wide.
const FOE_H: f64 = 61.0;
const FOE_W: f64 = 30.0;
/// The eye's half width (2.4 of the engine's 7 px faces).
const EYE_W: f64 = 17.0;
const DRIFT_W: f64 = FOE_W + 12.0;
/// The foe's body, feet to head.
const BODY: (f64, f64) = (-FOE_H + 4.0, -3.0);
/// How far a halo's heat falls a pixel.
const FALL: f32 = 1.0;

/// The rings' outer edges (as a share of the oval): the keystone, then rings 1 to 3; and how many slabs each ring has.
const RINGS: [f64; 4] = [0.32, 0.56, 0.79, 1.0];
const SECTORS: [usize; 4] = [1, 5, 7, 10];
/// The cracks open between these times (ms), from the keystone outward.
const CRACK_MS: (f64, f64) = (450.0, 1150.0);

/// Dark magic, dark to bright: void black, deep violet, a white-lilac edge.
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

/// One pixel of a slab: the shard it smashes into, its shade of sky (an index into VOID), and whether it is a star
/// (0 none, 1 dim, 2 bright).
#[derive(Clone, Copy)]
struct Cell {
    shard: u8,
    tone: u8,
    star: u8,
}

/// A slab of sky: its picture round its middle, where its middle is, when it falls and lands.
struct Slab {
    at: (i32, i32),
    w: usize,
    h: usize,
    px: Vec<Option<Cell>>,
    reach: f64,
    home: (f64, f64),
    t0: f64,
    t1: f64,
    spin: f64,
    key: bool,
    /// Each shard's middle (from the slab's middle) and pixel count.
    shards: Vec<(f64, f64, usize)>,
}

impl Slab {
    fn at(&self, x: i32, y: i32) -> Option<Cell> {
        let (dx, dy) = (x - self.at.0, y - self.at.1);
        if dx < 0 || dy < 0 || dx as usize >= self.w || dy as usize >= self.h {
            return None;
        }
        self.px
            .get(dy as usize * self.w + dx as usize)
            .copied()
            .flatten()
    }
}

/// A crack pixel: where, when it opens (s), and the two slabs it lies between.
struct Crack {
    x: f64,
    y: f64,
    t0: f64,
    side: [usize; 2],
}

/// Where a sky pixel is in the oval: (distance as a share of it, angle), and the slab it belongs to, if any.
fn slab_of(x: f64, y: f64) -> Option<usize> {
    let (nx, ny) = ((x + 0.5 - SKY.0) / HALF.0, (y + 0.5 - SKY.1) / HALF.1);
    let d = libm::hypot(nx, ny);
    let a = libm::atan2(ny, nx);
    // a ragged outer edge
    let edge = 1.0
        + 0.05 * libm::sin(5.0 * a + 1.0)
        + 0.035 * libm::sin(9.0 * a + 2.3)
        + 0.025 * libm::sin(14.0 * a + 0.7);
    if d >= edge {
        return None;
    }
    let mut base = 0;
    for (ring, (&r, &n)) in RINGS.iter().zip(SECTORS.iter()).enumerate() {
        let rf = ring as f64;
        let wob = 0.035 * libm::sin(7.0 * a + rf * 1.7) + 0.025 * libm::sin(12.0 * a + rf * 2.9);
        if d < r + wob || ring == RINGS.len() - 1 {
            if n <= 1 {
                return Some(base);
            }
            let off = rf * 0.9 + 0.3;
            let bend = 0.22 * libm::sin(d * 8.0 + rf * 2.0) + 0.08 * libm::sin(d * 21.0 + rf);
            let s = wrap((a + off + bend) / TAU, 1.0) * n as f64;
            return Some(base + (libm::floor(s) as usize).min(n - 1));
        }
        base += n;
    }
    None
}

/// The ring of slab `id` (0 the keystone).
fn ring_of(id: usize) -> usize {
    let mut base = 0;
    for (ring, &n) in SECTORS.iter().enumerate() {
        if id < base + n {
            return ring;
        }
        base += n;
    }
    SECTORS.len() - 1
}

/// The sky laid out: its slabs (with their times) and the cracks between them.
fn build() -> (Vec<Slab>, Vec<Crack>) {
    let count: usize = SECTORS.iter().sum();
    let (x0, x1) = (
        libm::floor(SKY.0 - HALF.0 * 1.15) as i32,
        libm::ceil(SKY.0 + HALF.0 * 1.15) as i32,
    );
    let (y0, y1) = (
        libm::floor(SKY.1 - HALF.1 * 1.15) as i32,
        libm::ceil(SKY.1 + HALF.1 * 1.15) as i32,
    );
    let mut cells: Vec<Vec<(i32, i32)>> = vec![Vec::new(); count];
    let mut cracks = Vec::new();
    for y in y0..=y1 {
        for x in x0..=x1 {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let Some(id) = slab_of(fx, fy) else {
                continue;
            };
            // a seam: the pixel whose right or lower neighbour is another slab
            let other = [slab_of(fx + 1.0, fy), slab_of(fx, fy + 1.0)]
                .into_iter()
                .flatten()
                .find(|&o| o != id);
            if let Some(o) = other {
                let d = libm::hypot((fx - SKY.0) / HALF.0, (fy - SKY.1) / HALF.1);
                let u = clamp((d - 0.2) / 0.85, 0.0, 1.0);
                let ms = CRACK_MS.0
                    + (CRACK_MS.1 - CRACK_MS.0) * libm::pow(u, 0.8)
                    + 50.0 * rand(&[fx, fy, 61.0]);
                cracks.push(Crack {
                    x: fx,
                    y: fy,
                    t0: ms / 1000.0,
                    side: [id, o],
                });
            } else if let Some(list) = cells.get_mut(id) {
                list.push((x, y));
            }
        }
    }
    let slabs = cells
        .iter()
        .enumerate()
        .map(|(i, list)| slab(i, list))
        .collect();
    (slabs, cracks)
}

/// Slab `i` from its pixels (sky coordinates).
fn slab(i: usize, list: &[(i32, i32)]) -> Slab {
    let n = list.len().max(1) as f64;
    let (mx, my) = list.iter().fold((0.0, 0.0), |a, c| {
        (a.0 + f64::from(c.0) / n, a.1 + f64::from(c.1) / n)
    });
    let (mx, my) = (libm::floor(mx + 0.5) as i32, libm::floor(my + 0.5) as i32);
    let (lx0, ly0) = list
        .iter()
        .fold((i32::MAX, i32::MAX), |a, c| (a.0.min(c.0), a.1.min(c.1)));
    let (lx1, ly1) = list
        .iter()
        .fold((i32::MIN, i32::MIN), |a, c| (a.0.max(c.0), a.1.max(c.1)));
    let (w, h) = if list.is_empty() {
        (0, 0)
    } else {
        ((lx1 - lx0 + 1) as usize, (ly1 - ly0 + 1) as usize)
    };
    let ring = ring_of(i);
    let key = ring == 0;
    // shards: wedges round the slab's middle, more for a bigger slab
    let pieces = clamp(libm::floor(n / 70.0), 1.0, 6.0);
    let turn = rand(&[i as f64, 47.0]) * TAU;
    let tint = (i % 3) as f64;
    let mut px = vec![None; w * h];
    let (mut reach, mut bottom): (f64, f64) = (0.0, 0.0);
    let mut shards: Vec<(f64, f64, usize)> = vec![(0.0, 0.0, 0); pieces as usize];
    for &(x, y) in list {
        let (dx, dy) = (f64::from(x - mx), f64::from(y - my));
        let k = libm::floor(wrap((libm::atan2(dy, dx * 0.6) + turn) / TAU, 1.0) * pieces) as usize;
        let k = k.min(shards.len().saturating_sub(1));
        // its shade: deep violet, a different shade a slab, darker toward the sky's bottom, a grain; and stars
        let (gx, gy) = (f64::from(x), f64::from(y));
        let star = rand(&[gx, gy, 5.0]);
        let low = (gy - SKY.1) / HALF.1;
        let shade = tint * 0.25 + fbm(gx * 0.15, gy * 0.15, 3.0, 2) + 0.5 * low;
        let tone = if shade > 1.3 {
            4
        } else if shade > 0.6 {
            3
        } else {
            2
        };
        let cell = Cell {
            shard: k as u8,
            tone,
            star: if star < 0.006 {
                2
            } else if star < 0.022 {
                1
            } else {
                0
            },
        };
        if let Some(slot) = px.get_mut((y - ly0) as usize * w + (x - lx0) as usize) {
            *slot = Some(cell);
        }
        reach = reach.max(libm::hypot(dx, dy));
        bottom = bottom.max(dy);
        if let Some(sh) = shards.get_mut(k) {
            *sh = (sh.0 + dx, sh.1 + dy, sh.2 + 1);
        }
    }
    for sh in &mut shards {
        let c = sh.2.max(1) as f64;
        *sh = (sh.0 / c, sh.1 / c, sh.2);
    }
    let home = (f64::from(mx), f64::from(my));
    let jitter = rand(&[i as f64, 41.0]);
    let (t0, t1) = if key {
        // it lands on the foe's head at BLOW
        let drop = (BODY.0 - 2.0) - (home.1 + bottom);
        (BLOW - libm::sqrt(2.0 * drop.max(1.0) / GRAVITY), BLOW)
    } else {
        let t0 = FALL_AT.get(ring - 1).copied().unwrap_or(1.4) + FALL_SPREAD * jitter;
        let drop = -(home.1 + bottom);
        (t0, t0 + libm::sqrt(2.0 * drop.max(1.0) / GRAVITY))
    };
    Slab {
        at: (lx0 - mx, ly0 - my),
        w,
        h,
        px,
        reach: reach + 1.0,
        home,
        t0,
        t1,
        spin: (rand(&[i as f64, 43.0]) - 0.5) * if key { 0.3 } else { 1.4 },
        key,
        shards,
    }
}

struct Ordeal {
    slabs: Vec<Slab>,
    cracks: Vec<Crack>,
    /// The night round the sky: each pixel it darkens and how dark.
    night: Vec<(i16, i16, f32)>,
    seed: f64,
}

fn make(o: &Opts) -> Box<dyn Show> {
    let (slabs, cracks) = build();
    Box::new(Ordeal {
        slabs,
        cracks,
        night: night_mask(),
        seed: o.seed,
    })
}

impl Show for Ordeal {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if layer == Layer::Back {
            self.night(p, t);
            return;
        }
        let mut halo = Halo::new(W, H, OX, OY);
        self.sky(p, t);
        self.cracks(p, &mut halo, t);
        for s in &self.slabs {
            if t >= s.t0 && t < s.t1 && !s.key {
                self.draw_slab(p, &mut halo, s, t);
            }
        }
        self.key_eye(p, &mut halo, t);
        self.drift(p, &mut halo, t);
        self.shards(p, t);
        halo.spread(FALL);
        paint_halo(p, &halo);
        self.stare(p, t);
        self.shock(p, t);
    }
}

/// Where a slab is at `t` once falling: its middle, and how far it has turned.
fn fall_at(s: &Slab, t: f64) -> (f64, f64, f64) {
    let k = (t - s.t0).max(0.0);
    let drift = if s.key { 0.0 } else { s.home.0 * 0.12 * k };
    (
        s.home.0 - drift,
        s.home.1 + 0.5 * GRAVITY * k * k,
        s.spin * k,
    )
}

/// The sky grows in from its middle: whether sky pixel (gx, gy) is in yet, its front dithered.
fn grown_in(gx: f64, gy: f64, grown: f64) -> bool {
    if grown >= 1.0 {
        return true;
    }
    let d = libm::hypot((gx - SKY.0) / HALF.0, (gy - SKY.1) / HALF.1);
    d <= grown * 1.2 - 0.2 * bayer4(gx, gy)
}

impl Ordeal {
    /// One pixel of sky: its shade, or a star that twinkles.
    fn sky_px(&self, s: &Slab, cell: Cell, lx: i32, ly: i32, t: f64) -> Rgba {
        if cell.star == 0 {
            return VOID.get(usize::from(cell.tone)).copied().unwrap_or(VOID[2]);
        }
        let (gx, gy) = (s.home.0 + f64::from(lx), s.home.1 + f64::from(ly));
        let on = rand(&[gx, gy, libm::floor(t * 6.0), self.seed]) < 0.75;
        if cell.star == 2 && on {
            VOID[7]
        } else if on {
            VOID[5]
        } else {
            VOID[3]
        }
    }

    /// The slabs still in the sky; one about to fall trembles.
    fn sky(&self, p: &mut Painter, t: f64) {
        let grown = smooth(GROW.0, GROW.1, t);
        if grown <= 0.0 {
            return;
        }
        let shake_on = libm::floor(t * 20.0) % 2.0 == 0.0;
        for (i, s) in self.slabs.iter().enumerate() {
            if t >= s.t0 {
                continue;
            }
            let shake = if t > s.t0 - TREMBLE && (shake_on ^ (i % 2 == 0)) {
                1.0
            } else {
                0.0
            };
            for ly in 0..s.h as i32 {
                for lx in 0..s.w as i32 {
                    let (x, y) = (lx + s.at.0, ly + s.at.1);
                    let Some(cell) = s.at(x, y) else {
                        continue;
                    };
                    let (gx, gy) = (s.home.0 + f64::from(x), s.home.1 + f64::from(y));
                    if !grown_in(gx, gy, grown) {
                        continue;
                    }
                    p.px(gx + 0.5 + shake, gy + 0.5, self.sky_px(s, cell, x, y, t));
                }
            }
        }
    }

    /// The cracks: each cell white as the crack reaches it, settling to cold violet, while a slab beside it is up.
    fn cracks(&self, p: &mut Painter, halo: &mut Halo, t: f64) {
        let grown = smooth(GROW.0, GROW.1, t);
        for k in &self.cracks {
            let up = |i: usize| self.slabs.get(i).is_some_and(|s| t < s.t0);
            if !(up(k.side[0]) || up(k.side[1])) {
                continue;
            }
            let (fx, fy) = (k.x + 0.5, k.y + 0.5);
            if t < k.t0 {
                // not cracked yet: plain sky
                if grown > 0.0 && grown_in(k.x, k.y, grown) {
                    p.px(fx, fy, VOID[2]);
                }
                continue;
            }
            let fresh = 1.0 - clamp((t - k.t0) / 0.15, 0.0, 1.0);
            p.px(fx, fy, if fresh > 0.4 { VOID[7] } else { VOID[6] });
            halo.stamp(fx, fy, (1.8 + 2.4 * fresh) as f32, 1);
        }
    }

    /// A falling slab: it turns a little as it drops and shows its thickness, a lit top edge and a dark underside two
    /// pixels deep. Each screen pixel looks back to the slab's pixel it shows, so a turned slab has no holes.
    fn draw_slab(&self, p: &mut Painter, halo: &mut Halo, s: &Slab, t: f64) {
        let (x, y, a) = fall_at(s, t);
        let (c, n) = (libm::cos(a), libm::sin(a));
        let r = s.reach + 3.0;
        let (x0, y0) = (libm::floor(x - r), libm::floor(y - r));
        let (x1, y1) = (libm::ceil(x + r), libm::ceil(y + r));
        let mut sy = y0;
        while sy <= y1 {
            let mut sx = x0;
            while sx <= x1 {
                let (dx, dy) = (sx + 0.5 - x, sy + 0.5 - y);
                let back = |oy: f64| {
                    (
                        libm::floor(dx * c + (dy - oy) * n + 0.5) as i32,
                        libm::floor(-dx * n + (dy - oy) * c + 0.5) as i32,
                    )
                };
                let (lx, ly) = back(0.0);
                if let Some(cell) = s.at(lx, ly) {
                    let top = s.at(lx, ly - 1).is_none();
                    let edge = s.at(lx - 1, ly).is_none()
                        || s.at(lx + 1, ly).is_none()
                        || s.at(lx, ly + 1).is_none();
                    let col = if top {
                        VOID[6]
                    } else if edge {
                        VOID[4]
                    } else {
                        self.sky_px(s, cell, lx, ly, t)
                    };
                    p.px(sx + 0.5, sy + 0.5, col);
                    if top {
                        halo.stamp(sx + 0.5, sy + 0.5, 1.0, 1);
                    }
                } else {
                    let (ux, uy) = back(1.0);
                    let (vx, vy) = back(2.0);
                    if s.at(ux, uy).is_some() || s.at(vx, vy).is_some() {
                        p.px(sx + 0.5, sy + 0.5, VOID[0]);
                    }
                }
                sx += 1.0;
            }
            sy += 1.0;
        }
    }

    /// The keystone: it holds the eye; once it falls it carries the eye down onto the foe, still open.
    fn key_eye(&self, p: &mut Painter, halo: &mut Halo, t: f64) {
        let Some(s) = self.slabs.iter().find(|s| s.key) else {
            return;
        };
        if t >= s.t1 {
            return;
        }
        let (cx, cy) = if t >= s.t0 {
            self.draw_slab(p, halo, s, t);
            let (x, y, _) = fall_at(s, t);
            (x, y)
        } else {
            s.home
        };
        self.eye(p, halo, t, cx + SKY.0 - s.home.0, cy + SKY.1 - s.home.1);
    }

    /// The eye: it opens in the keystone and stares down; it stays open as it falls and shuts as it strikes.
    fn eye(&self, p: &mut Painter, halo: &mut Halo, t: f64, ex: f64, ey: f64) {
        let o = smooth(OPEN.0, OPEN.1, t) * (1.0 - smooth(BLOW - 0.04, BLOW, t));
        if o <= 0.02 {
            return;
        }
        let lid = 7.0 * o;
        let mut x = -EYE_W;
        while x <= EYE_W {
            let u = x / EYE_W;
            let h = lid * (1.0 - u * u);
            let (top, bot) = (libm::floor(ey - h), libm::floor(ey + h * 0.8));
            for y in [top, bot] {
                p.px(ex + x + 0.5, y + 0.5, VOID[7]);
                halo.stamp(ex + x + 0.5, y + 0.5, 3.5, 2);
            }
            let mut y = top + 1.0;
            while y < bot {
                p.px(ex + x + 0.5, y + 0.5, VOID[0]);
                y += 1.0;
            }
            x += 1.0;
        }
        // the pupil, looking down at the foe
        if o > 0.45 {
            let r = 4.5 * o;
            p.circle(ex, ey + 0.8, r + 1.0, VOID[4], 1.0);
            p.circle(ex, ey + 0.8, r, VOID[0], 1.0);
            p.light(ex - 1.0, ey - 0.5, VOID[7], 0.8);
        }
    }

    /// A landed slab's shards: each flies out and up from where the slab hit, falls back, stays above the ground, and
    /// crumbles to dust at the end.
    fn shards(&self, p: &mut Painter, t: f64) {
        for (i, s) in self.slabs.iter().enumerate() {
            let k = t - s.t1;
            if !(0.0..SHATTER).contains(&k) {
                continue;
            }
            let (hx, hy, a) = fall_at(s, s.t1);
            let crumble = clamp((k - (SHATTER - SHATTER_DUST)) / SHATTER_DUST, 0.0, 1.0);
            for (j, &(sx, sy, count)) in s.shards.iter().enumerate() {
                if count == 0 {
                    continue;
                }
                let r = |salt: f64| rand(&[self.seed, i as f64, j as f64, salt]);
                let (vx, vy) = (sx * 3.0 + 50.0 * (r(1.0) - 0.5), -40.0 - 70.0 * r(2.0));
                let spin = (r(3.0) - 0.5) * 8.0;
                let (ox, mut oy) = (vx * k, vy * k + 0.5 * GRAVITY * k * k);
                let ground = -(hy + sy + 1.0);
                if oy > ground {
                    oy = ground;
                }
                let turn = a + spin * k;
                let (c, n) = (libm::cos(turn), libm::sin(turn));
                for ly in 0..s.h as i32 {
                    for lx in 0..s.w as i32 {
                        let (x, y) = (lx + s.at.0, ly + s.at.1);
                        let Some(cell) = s.at(x, y) else {
                            continue;
                        };
                        if usize::from(cell.shard) != j {
                            continue;
                        }
                        if crumble > 0.0 && rand(&[f64::from(x), f64::from(y), 9.0]) < crumble {
                            continue;
                        }
                        let (dx, dy) = (f64::from(x) - sx, f64::from(y) - sy);
                        let px = hx + sx + ox + dx * c - dy * n;
                        let py = (hy + sy + oy + dx * n + dy * c).min(-0.5);
                        let col = if crumble > 0.5 {
                            VOID[3]
                        } else {
                            self.sky_px(s, cell, x, y, t)
                        };
                        p.px(px + 0.5, py + 0.5, col);
                    }
                }
            }
            // the dust where it hit
            if k < 0.25 && !s.key {
                p.glow(
                    hx,
                    -2.0,
                    6.0 + s.reach * 0.4,
                    VOID[5],
                    0.5 * (1.0 - k / 0.25),
                );
            }
        }
    }

    /// The night round the sky, darkening in as the sky grows and lifting as it falls.
    fn night(&self, p: &mut Painter, t: f64) {
        let s = smooth(GROW.0, GROW.1 + 0.25, t) * (1.0 - smooth(FALL_AT[0], BLOW + 0.3, t));
        if s <= 0.0 {
            return;
        }
        for &(x, y, a) in &self.night {
            p.blend(
                f64::from(x) + 0.5,
                f64::from(y) + 0.5,
                VOID[0],
                f64::from(a) * s,
            );
        }
    }

    /// The eye's stare: a dithered cone of violet light from the pupil down onto the foe, and the foe lit cold.
    fn stare(&self, p: &mut Painter, t: f64) {
        let o = smooth(OPEN.0, OPEN.1, t) * (1.0 - smooth(BLOW - 0.4, BLOW, t));
        if o <= 0.02 {
            return;
        }
        let top = SKY.1 + 8.0;
        let mut y = libm::floor(top);
        while y < 0.0 {
            let u = (y - top) / -top;
            let half = 3.0 + 13.0 * u;
            let mut x = -libm::ceil(half);
            while x <= half {
                let keep = 0.7 - 0.5 * libm::fabs(x) / half;
                if rand(&[x, y, self.seed, 3.0]) < keep {
                    p.light(x + 0.5, y + 0.5, VOID[4], 0.16 * o * (1.0 - 0.4 * u));
                }
                x += 1.0;
            }
            y += 1.0;
        }
        p.glow(0.0, -FOE_H * 0.7, 16.0, VOID[5], 0.45 * o);
    }

    /// The blow: a white flash, a ring of cold light thrown out from the foe and one along the ground.
    fn shock(&self, p: &mut Painter, t: f64) {
        let k = t - BLOW;
        if !(0.0..0.32).contains(&k) {
            return;
        }
        let mid = (BODY.0 + BODY.1) / 2.0;
        if k < 0.07 {
            p.glow(0.0, mid, 46.0, VOID[7], 0.9 * (1.0 - k / 0.07));
        }
        let u = k / 0.32;
        let r = 8.0 + 50.0 * libm::sqrt(u);
        let c = if u < 0.35 {
            VOID[7]
        } else if u < 0.7 {
            VOID[6]
        } else {
            VOID[5]
        };
        p.ring(0.0, mid, r, 2.0, c, 1.0 - 0.6 * u);
        p.glow_ring(0.0, mid, r, 4.0, VOID[5], 0.5 * (1.0 - u));
        for i in 0..120 {
            let a = TAU * f64::from(i) / 120.0;
            p.px(libm::cos(a) * r * 1.3, -0.5 + libm::sin(a) * r * 0.2, c);
        }
        if k < 0.12 {
            for i in 0..10 {
                let a = TAU * f64::from(i) / 10.0 + 0.3;
                let (c0, s0) = (libm::cos(a), libm::sin(a));
                let (r0, r1) = (10.0 + 60.0 * k, 22.0 + 140.0 * k);
                p.line(c0 * r0, mid + s0 * r0, c0 * r1, mid + s0 * r1, VOID[7], 1.0);
            }
        }
    }

    /// The black drift round the feet: it piles up as the sky strikes, then sinks to a pool of shadow and fades.
    fn drift(&self, p: &mut Painter, halo: &mut Halo, t: f64) {
        let rise = smooth(BLOW - 0.05, BLOW + 0.3, t);
        if rise <= 0.0 {
            return;
        }
        let melt = smooth(MELT.0, MELT.1, t);
        if melt >= 1.0 {
            return;
        }
        let w = DRIFT_W * (1.0 - 0.4 * melt);
        let mut x = -libm::ceil(w);
        while x <= w {
            let u = x / w;
            let bump = 0.6 + 0.4 * fbm(x * 0.3, 0.0, self.seed, 2);
            let h = libm::floor(9.0 * (1.0 - u * u) * bump * rise * (1.0 - melt) + 0.5);
            if 1.0 - u * u > 0.0 {
                p.blend(x + 0.5, -0.5, VOID[0], 0.75 * (1.0 - melt));
            }
            let mut y = 0.0;
            while y < h {
                let top = y + 1.0 >= h;
                let glint = top && rand(&[x, self.seed, 5.0]) < 0.25;
                let col = if glint {
                    VOID[7]
                } else if top {
                    VOID[5]
                } else if y + 2.0 >= h {
                    VOID[3]
                } else if rand(&[x, y, self.seed]) < 0.3 {
                    VOID[2]
                } else {
                    VOID[1]
                };
                p.px(x + 0.5, -y - 0.5, col);
                if top {
                    halo.stamp(x + 0.5, -y - 0.5, if glint { 1.6 } else { 0.9 }, 0);
                }
                y += 1.0;
            }
            x += 1.0;
        }
    }
}

/// The night round the sky: an oval a little past it, darkest in the middle, falling off in four solid steps with
/// each step's edge dithered into the next.
fn night_mask() -> Vec<(i16, i16, f32)> {
    let (cx, cy) = (SKY.0, SKY.1 + HALF.1 * 0.2);
    let (rx, ry) = (HALF.0 * 1.15, HALF.1 * 1.5);
    let mut out = Vec::new();
    let (x0, x1) = (
        libm::floor(cx - rx * 1.2) as i32,
        libm::ceil(cx + rx * 1.2) as i32,
    );
    let (y0, y1) = (
        libm::floor(cy - ry * 1.2) as i32,
        libm::ceil(cy + ry * 1.2) as i32,
    );
    for y in y0.max(-(OY as i32))..=y1.min(-1) {
        for x in x0.max(-(OX as i32))..=x1.min(OX as i32 - 1) {
            let (fx, fy) = (f64::from(x), f64::from(y));
            let d = libm::hypot((fx + 0.5 - cx) / rx, (fy + 0.5 - cy) / ry);
            let q = (1.0 - smooth(0.35, 1.15, d)) * 4.0;
            let step = libm::floor(q);
            let up = if q - step > bayer4(fx, fy) { 1.0 } else { 0.0 };
            let a = 0.75 * (step + up) / 4.0;
            if a > 0.0 {
                out.push((x as i16, y as i16, a as f32));
            }
        }
    }
    out
}

/// A 4 by 4 ordered dither threshold, 0 to 1.
fn bayer4(x: f64, y: f64) -> f64 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    let xi = (libm::floor(x) as i64).rem_euclid(4) as usize;
    let yi = (libm::floor(y) as i64).rem_euclid(4) as usize;
    M.get(yi)
        .and_then(|r| r.get(xi))
        .map_or(0.5, |&v| (f64::from(v) + 0.5) / 16.0)
}

/// The glow round every stroke, painted as steps of cold light: a pale step near the stroke, then violet, then a
/// dithered deep edge. Strokes keep their own colour; only their halo is light.
fn paint_halo(p: &mut Painter, halo: &Halo) {
    for (x, y, h, kind) in halo.each() {
        if kind > 0 && h >= 2.0 {
            continue;
        }
        let (c, k) = if h >= 3.0 {
            (VOID[7], 0.55)
        } else if h >= 2.0 {
            (VOID[6], 0.4)
        } else if h >= 1.0 {
            (VOID[5], 0.28)
        } else if (libm::floor(x) + libm::floor(y)) % 2.0 == 0.0 {
            (VOID[4], 0.18)
        } else {
            continue;
        };
        p.light(x + 0.5, y + 0.5, c, k);
    }
}
