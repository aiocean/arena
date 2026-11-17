//! `ice/icicle-fall`, after the engine's: an icicle hangs from a frosted crust 80 px over the floor, drips, trembles
//! (from 0.3 s, when a fighter notices it), snaps off at 0.6 s, drops, stabs the floor at 0.944 s (the rules' impact)
//! and bursts into skidding panes of ice, splinters, frost dust and a shock band rolling along the floor. Played on
//! the floor where it falls; the origin is the floor.

use core::f64::consts::{FRAC_PI_2, PI};

use crate::kit::sim::{
    Bit, BitKind, Bits, GRAVITY, Kind, Lights, Particles, ParticlesOpts, Spawn, band, sparkle,
};
use crate::kit::{
    Layer, Opts, Painter, Recipe, Rgba, Rng, Show, TAU, WHITE, clamp, ease, js_max, js_min,
    js_round, js_sign, libm, ramp, rand, seg, smooth, step_at,
};

pub const RECIPE: Recipe = Recipe::new("ice/icicle-fall", false, D, make);

const D: f64 = 2.6;

const PALETTE: [Rgba; 8] = [
    Rgba::hex24(0x151a3d),
    Rgba::hex24(0x212f6b),
    Rgba::hex24(0x2c58aa),
    Rgba::hex24(0x3a8bd8),
    Rgba::hex24(0x62c8f4),
    Rgba::hex24(0xa6ecff),
    Rgba::hex24(0xdcfbff),
    Rgba::hex24(0xffffff),
];

// where it hangs, how long, how wide at the root
const ROOT: f64 = -80.0;
const LEN: f64 = 36.0;
const W: f64 = 12.0;
const DRIP: f64 = 0.16;
const TREMBLE: f64 = 0.3;
const SNAP: f64 = 0.6;
const FREE: f64 = SNAP + 0.04;
const FLOOR: f64 = 0.0;

/// How long the icicle takes to fall from its root to the floor.
fn fall_t() -> f64 {
    libm::sqrt((2.0 * (-ROOT - LEN)) / GRAVITY)
}

fn impact() -> f64 {
    FREE + fall_t()
}

/// One bit of the rime it hangs from: a frosted crust with stubs of older icicles.
struct Rime {
    x: f64,
    h: f64,
    stub: f64,
}

struct Icicle {
    c: [Rgba; 8],
    r: Rng,
    ps: Particles,
    bits: Bits<()>,
    lights: Lights,
    panes: Panes,
    rime: Vec<Rime>,
    t: f64,
    snapped: bool,
    landed: bool,
    dripped: bool,
}

fn make(o: &Opts) -> Box<dyn Show> {
    let c: [Rgba; 8] = ramp(&o.palette_or(&PALETTE));
    let seed = o.seed;
    let mut rime = Vec::new();
    let mut x = -18.0;
    while x <= 18.0 {
        let h = rand(&[x, seed, 2.0]);
        let d = libm::fabs(x) / 18.0;
        rime.push(Rime {
            x,
            h: 1.0 + js_round((0.6 + h * 1.8) * (1.0 - d * 0.7)),
            stub: if h < 0.3 && libm::fabs(x) > 7.0 {
                2.0 + js_round(rand(&[x, 5.0]) * 5.0 * (1.0 - d * 0.5))
            } else {
                0.0
            },
        });
        x += 1.0;
    }
    Box::new(Icicle {
        c,
        r: Rng::new(seed as u32),
        ps: Particles::new(ParticlesOpts {
            ground: FLOOR,
            sparks: vec![c[7], c[6], c[5], c[4]],
            haze: Some(c[5]),
        }),
        bits: Bits::new(FLOOR),
        lights: Lights::new(),
        panes: Panes::new(FLOOR),
        rime,
        t: 0.0,
        snapped: false,
        landed: false,
        dripped: false,
    })
}

/// The icicle: a rippled taper hanging from (x, top), lit down its left side with a bright core streak, rings of light
/// at the ripples, deep on the right, a dark rim. `white` flashes it; `glint` (0..1, or -1) is a highlight across it.
fn icicle(p: &mut Painter, c: &[Rgba; 8], x: f64, top: f64, white: bool, glint: f64) {
    let (len, w) = (LEN, W);
    let hw = |u: f64| {
        (w / 2.0)
            * libm::pow(1.0 - u, 0.75)
            * (1.0 + 0.14 * libm::sin(u * 13.0 + 0.5))
            * if u < 0.08 { 1.15 } else { 1.0 }
            + 0.3
    };
    p.shade(
        x - w / 2.0 - 4.0,
        top - 1.0,
        x + w / 2.0 + 4.0,
        top + len + 1.0,
        |px, py, out| {
            let u = (py - top) / len;
            let cx = x + 0.7 * libm::sin(u * 6.0 + 1.0);
            let s = px - cx;
            if !(0.0..=1.0).contains(&u) {
                return None;
            }
            let h = hw(u);
            if libm::fabs(s) > h + 1.0 {
                return None;
            }
            if libm::fabs(s) > h {
                out.c = c[1];
                return None;
            }
            if white {
                return Some(WHITE);
            }
            let q = s / h;
            let ring = libm::sin(u * 13.0 + 0.5) > 0.7;
            let mut col = if u > 0.88 {
                WHITE
            } else if q < -0.72 {
                c[6]
            } else if q < -0.2 {
                if ring { c[7] } else { c[5] }
            } else if q < 0.18 {
                if u < 0.72 { c[7] } else { c[6] }
            } else if q > 0.8 {
                c[2]
            } else if libm::fabs(q - 0.45) < 0.1 && u > 0.15 && u < 0.6 {
                c[5]
            } else if q < 0.6 {
                c[4]
            } else {
                c[3]
            };
            if glint >= 0.0 && q < 0.25 && libm::fabs(u - glint) < 0.06 {
                col = WHITE;
            }
            out.c = col;
            None
        },
    );
}

fn pick(c: &[Rgba; 8], r: &mut Rng, a: f64, b: f64) -> Rgba {
    c.get(r.int(a, b) as usize).copied().unwrap_or(WHITE)
}

/// A shard bit with the engine's usual knobs.
#[allow(clippy::too_many_arguments)]
fn shard(kind: BitKind, x: f64, y: f64, vx: f64, vy: f64, a: f64, va: f64, s: f64, c: Rgba) -> Bit {
    Bit {
        a,
        va,
        s,
        c,
        vx,
        vy,
        ..Bit::new(kind, x, y)
    }
}

/// A few splinters flung off at (x, y).
fn splinter(r: &mut Rng, bits: &mut Bits<()>, c: &[Rgba; 8], x: f64, y: f64, vx: f64, n: u32) {
    for _ in 0..n {
        let bx = x + r.range(-2.0, 2.0);
        let vx = vx * 0.5 + r.range(-45.0, 45.0);
        let vy = -r.range(30.0, 100.0);
        let life = r.range(0.4, 0.75);
        let a = r.range(0.0, TAU);
        let va = r.range(-18.0, 18.0);
        let s = r.range(1.2, 2.4);
        let col = pick(c, r, 4.0, 7.0);
        bits.add(Bit {
            g: 0.8,
            drag: 0.6,
            bounce: 0.35,
            life,
            ..shard(BitKind::Shard, bx, y - 1.0, vx, vy, a, va, s, col)
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    ps: &mut Particles,
    r: &mut Rng,
    kind: Kind,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    life: f64,
    col: Rgba,
    size: f64,
) {
    ps.spawn(
        r,
        Spawn {
            col: Some(col),
            size,
            ..Spawn::new(kind, x, y, vx, vy, life)
        },
    );
}

impl Icicle {
    /// Where the icicle's top is: at the root, then falling.
    fn top(&self) -> f64 {
        ROOT + if self.t < FREE {
            0.0
        } else {
            0.5 * GRAVITY * (self.t - FREE) * (self.t - FREE)
        }
    }

    /// It snaps: a crack flashes across the root, chips and a puff burst off.
    fn snap(&mut self) {
        let (r, c) = (&mut self.r, &self.c);
        self.lights.add(0.0, ROOT + 4.0, 24.0, WHITE, 0.6, 0.1);
        self.lights.add(0.0, ROOT + 4.0, 32.0, c[5], 0.4, 0.3);
        for _ in 0..7 {
            let a = r.range(0.0, TAU);
            let sp = r.range(30.0, 110.0);
            let x = r.range(-4.0, 4.0);
            let life = r.range(0.5, 0.9);
            let ba = r.range(0.0, TAU);
            let va = r.range(-20.0, 20.0);
            let s = r.range(1.0, 2.0);
            let col = pick(c, r, 5.0, 7.0);
            self.bits.add(Bit {
                g: 0.8,
                drag: 0.8,
                bounce: 0.3,
                life,
                ..shard(
                    BitKind::Shard,
                    x,
                    ROOT + 2.0,
                    libm::cos(a) * sp,
                    libm::sin(a) * sp - 20.0,
                    ba,
                    va,
                    s,
                    col,
                )
            });
        }
        for _ in 0..3 {
            let x = r.range(-5.0, 5.0);
            let vx = r.range(-25.0, 25.0);
            let vy = r.range(-5.0, 5.0);
            let life = r.range(0.4, 0.7);
            let size = r.range(1.6, 2.6);
            spawn(
                &mut self.ps,
                r,
                Kind::Vapour,
                x,
                ROOT + 2.0,
                vx,
                vy,
                life,
                c[6],
                size,
            );
        }
        for _ in 0..5 {
            let x = r.range(-6.0, 6.0);
            let vx = r.range(-30.0, 30.0);
            let vy = r.range(-20.0, 20.0);
            let life = r.range(0.6, 1.2);
            let col = if r.chance(0.5) { WHITE } else { c[6] };
            let size = r.range(0.5, 1.3);
            spawn(
                &mut self.ps,
                r,
                Kind::Flake,
                x,
                ROOT + 2.0,
                vx,
                vy,
                life,
                col,
                size,
            );
        }
    }

    /// It stabs the floor and bursts.
    fn burst(&mut self) {
        let (r, c) = (&mut self.r, &self.c);
        let tp = -LEN;
        self.lights.add(0.0, -16.0, 46.0, WHITE, 0.85, 0.12);
        self.lights.add(0.0, -16.0, 58.0, c[4], 0.5, 0.5);
        // the icicle in pieces: cut across its length, then down the middle
        let mut parts = vec![vec![(-W / 2.0, tp), (W / 2.0, tp), (0.5, 0.0)]];
        for v in [0.3, 0.58, 0.8] {
            let mut next = Vec::new();
            for poly in &parts {
                let y = tp + LEN * v + r.range(-2.0, 2.0);
                let a = r.range(-0.4, 0.4);
                next.extend(split_poly(poly, 0.0, y, a));
            }
            parts = next;
        }
        let mut next = Vec::new();
        for poly in &parts {
            let m = centroid(poly);
            if m.2 > 22.0 {
                let a = PI / 2.0 + r.range(-0.5, 0.5);
                next.extend(split_poly(poly, m.0, m.1, a));
            } else {
                next.push(poly.clone());
            }
        }
        for part in next {
            let (mx, my, _) = centroid(&part);
            let side = if mx < 0.5 { -1.0 } else { 1.0 };
            let low = clamp(-my / LEN, 0.0, 1.0);
            let mut pane = Pane::new(
                part.iter().map(|&(x, y)| (x - mx, y - my)).collect(),
                mx,
                my,
            );
            pane.vx = side * r.range(30.0, 110.0) + r.range(-15.0, 15.0);
            pane.vy = -r.range(70.0, 200.0) * (0.4 + low);
            pane.va = r.range(-12.0, 12.0);
            pane.f = r.range(-0.7, 0.7);
            pane.vf = r.range(-8.0, 8.0);
            pane.crumble = r.range(0.15, 0.45);
            pane.life = r.range(1.0, 1.4);
            self.panes.add(pane);
        }
        for _ in 0..52 {
            let a = -FRAC_PI_2 + r.range(-1.25, 1.25);
            let sp = r.range(60.0, 230.0);
            let kind = if r.chance(0.15) {
                BitKind::Chunk
            } else {
                BitKind::Shard
            };
            let x = r.range(-3.0, 3.0);
            let y = -r.range(1.0, 10.0);
            let life = r.range(0.6, 1.2);
            let ba = r.range(0.0, TAU);
            let va = r.range(-22.0, 22.0);
            let s = r.range(1.2, 3.0);
            let col = pick(c, r, 4.0, 7.0);
            self.bits.add(Bit {
                ramp: Some(vec![c[1], c[3], c[4], c[6], c[7]]),
                g: 0.85,
                drag: 0.6,
                bounce: 0.32,
                life,
                ..shard(
                    kind,
                    x,
                    y,
                    libm::cos(a) * sp,
                    libm::sin(a) * sp,
                    ba,
                    va,
                    s,
                    col,
                )
            });
        }
        for _ in 0..10 {
            let s = r.sign();
            let x = s * r.range(0.0, 8.0);
            let y = -r.range(1.0, 6.0);
            let vx = s * r.range(25.0, 90.0);
            let vy = -r.range(2.0, 14.0);
            let life = r.range(0.6, 1.1);
            let size = r.range(2.2, 3.8);
            spawn(
                &mut self.ps,
                r,
                Kind::Vapour,
                x,
                y,
                vx,
                vy,
                life,
                c[6],
                size,
            );
        }
        for _ in 0..12 {
            let x = r.range(-6.0, 6.0);
            let y = -r.range(2.0, 16.0);
            let vx = r.range(-70.0, 70.0);
            let vy = -r.range(30.0, 110.0);
            let life = r.range(1.0, 1.5);
            let col = if r.chance(0.6) { WHITE } else { c[6] };
            let size = r.range(0.5, 1.5);
            spawn(&mut self.ps, r, Kind::Flake, x, y, vx, vy, life, col, size);
        }
        for _ in 0..6 {
            let x = r.range(-24.0, 24.0);
            let y = -r.range(4.0, 34.0);
            let vx = r.range(-10.0, 10.0);
            let vy = -r.range(4.0, 14.0);
            let life = r.range(0.4, 0.8);
            let col = if r.chance(0.5) { WHITE } else { c[6] };
            let size = r.range(1.0, 2.2);
            spawn(&mut self.ps, r, Kind::Star, x, y, vx, vy, life, col, size);
        }
    }

    /// The shock band rolling out along the floor where it struck: its far half behind, its near half in front.
    fn shock(&self, p: &mut Painter, far: bool) {
        let c = &self.c;
        let k = seg(self.t, impact(), impact() + 0.35);
        if k >= 1.0 {
            return;
        }
        band(
            p,
            0.0,
            0.5,
            6.0 + 34.0 * ease::out_cubic(k),
            0.18,
            js_max(1.0, 4.0 * (1.0 - k)),
            1.1 - k,
            &[c[3], c[4], c[5], c[6]],
            |a| (libm::sin(a) < 0.0) == far,
            !far && k < 0.15,
        );
    }

    /// Nothing lives past the end.
    fn end_by(&mut self) {
        let left = js_max(0.001, D - 0.05 - self.t);
        for q in self.ps.list_mut() {
            q.life = js_min(q.life, left);
        }
        for q in self.bits.list_mut() {
            q.bit.life = js_min(q.bit.life, left);
        }
        for q in &mut self.panes.list {
            q.life = js_min(q.life, left);
        }
    }
}

impl Show for Icicle {
    fn update(&mut self, dt: f64) {
        self.t += dt;
        let t = self.t;
        let impact = impact();
        // a drop of meltwater swells at the tip and falls
        if !self.dripped && t >= DRIP {
            self.dripped = true;
            self.bits.add(Bit {
                vy: 20.0,
                g: 0.9,
                life: 1.2,
                c: self.c[6],
                ..Bit::new(BitKind::Drip, 0.5, ROOT + LEN + 1.0)
            });
        }
        // it trembles: frost sifts down from the root
        if t > TREMBLE && t < SNAP && self.r.chance(0.25) {
            let r = &mut self.r;
            let x = r.range(-6.0, 6.0);
            let y = ROOT + r.range(0.0, 3.0);
            let vx = r.range(-8.0, 8.0);
            let vy = r.range(5.0, 20.0);
            let life = r.range(0.6, 1.1);
            let col = if r.chance(0.5) { WHITE } else { self.c[6] };
            let size = r.range(0.5, 1.1);
            spawn(&mut self.ps, r, Kind::Flake, x, y, vx, vy, life, col, size);
        }
        if !self.snapped && t >= SNAP {
            self.snapped = true;
            self.snap();
        }
        // falling: it sheds frost motes that hang in the air behind it
        if t > FREE && t < impact && self.r.chance(0.5) {
            let tp = self.top();
            let r = &mut self.r;
            let x = r.range(-3.0, 3.0);
            let y = tp + r.range(0.0, LEN * 0.6);
            let vx = r.range(-10.0, 10.0);
            let vy = r.range(-40.0, -10.0);
            let life = r.range(0.2, 0.35);
            let col = if r.chance(0.5) { self.c[6] } else { self.c[5] };
            spawn(&mut self.ps, r, Kind::Mote, x, y, vx, vy, life, col, 0.6);
        }
        if !self.landed && t >= impact {
            self.landed = true;
            self.burst();
        }
        if self.landed && t < impact + 0.9 && self.r.chance(0.08) && self.bits.count() > 0 {
            let r = &mut self.r;
            if let Some(b) = r.pick(self.bits.list()) {
                let (bx, by) = (b.bit.x, b.bit.y);
                let life = r.range(0.2, 0.4);
                let col = if r.chance(0.5) { WHITE } else { self.c[6] };
                let size = r.range(0.6, 1.4);
                spawn(
                    &mut self.ps,
                    r,
                    Kind::Star,
                    bx,
                    by - 1.0,
                    0.0,
                    -3.0,
                    life,
                    col,
                    size,
                );
            }
        }
        self.ps.update(&mut self.r, dt);
        self.bits.update(dt);
        let (r, ps, bits, c) = (&mut self.r, &mut self.ps, &mut self.bits, &self.c);
        self.panes.update(dt, |ev| match ev {
            PaneEvent::Land { x, y, vx, speed } => {
                if speed < 100.0 {
                    return;
                }
                let pvx = r.range(-15.0, 15.0);
                let pvy = -r.range(2.0, 8.0);
                let life = r.range(0.4, 0.6);
                let size = r.range(1.4, 2.2);
                spawn(ps, r, Kind::Vapour, x, y - 1.0, pvx, pvy, life, c[6], size);
                splinter(r, bits, c, x, y, vx, 2);
            }
            PaneEvent::Crumble { x, vx } => splinter(r, bits, c, x, 0.0, vx, 3),
        });
        self.lights.update(dt);
        self.end_by();
    }

    fn draw(&self, p: &mut Painter, _t: f64, layer: Layer) {
        let (t, c) = (self.t, &self.c);
        let impact = impact();
        if layer == Layer::Back {
            self.draw_back(p);
            return;
        }
        // the rime it hangs from, fading at the end
        let ra = 1.0 - smooth(D - 0.7, D - 0.2, t);
        for r in &self.rime {
            let mut j = 0.0;
            while j < r.h + 1.0 {
                let col = if j == r.h {
                    WHITE
                } else if j == r.h - 1.0 {
                    c[6]
                } else if j == 0.0 {
                    c[3]
                } else {
                    c[5]
                };
                p.dot(r.x, ROOT - 1.0 - j, col, ra);
                j += 1.0;
            }
            let mut j = 0.0;
            while j < r.stub {
                let col = if j == r.stub - 1.0 {
                    WHITE
                } else if j == 0.0 {
                    c[5]
                } else {
                    c[4]
                };
                p.dot(r.x, ROOT + j, col, ra);
                j += 1.0;
            }
            if r.stub > 4.0 {
                p.dot(r.x + 1.0, ROOT, c[3], ra);
            }
        }
        // the snapped root: a jagged stub left behind
        if self.snapped {
            let mut x = -W / 2.0 + 1.0;
            while x <= W / 2.0 - 1.0 {
                let l = 1.0 + js_round(rand(&[x, 77.0]) * 2.5);
                let mut j = 0.0;
                while j < l {
                    let col = if j == l - 1.0 {
                        if t < SNAP + 0.15 { WHITE } else { c[6] }
                    } else if x < 0.0 {
                        c[5]
                    } else {
                        c[3]
                    };
                    p.dot(x, ROOT + j, col, ra);
                    j += 1.0;
                }
                x += 1.0;
            }
        }
        p.glow(0.0, ROOT, 18.0, c[4], 0.3 * ra);
        if t >= impact {
            self.shock(p, false);
            if t < impact + 0.22 {
                sparkle(
                    p,
                    0.5,
                    -3.0,
                    10.0 * ease::out_back(js_min(1.0, (t - impact) / 0.05), ease::BACK_S)
                        * (1.0 - (t - impact) / 0.22),
                    c[6],
                    c[4],
                    0.7,
                );
            }
        }
        if t < impact {
            // the icicle: trembling before it snaps, then falling with a streak of air behind it
            let tp = self.top();
            let jx = if t > TREMBLE && t < SNAP {
                f64::from(libm::floor(t * 36.0) % 2.0 != 0.0)
                    * js_round(seg(t, TREMBLE, SNAP) * 1.4 + 0.4)
            } else {
                0.0
            };
            if t >= FREE {
                let sp = GRAVITY * (t - FREE);
                for (dx, k) in [(-3.0, 0.7), (0.5, 1.0), (4.0, 0.6)] {
                    let l = js_min(30.0, sp * 0.07) * k;
                    if l > 2.0 {
                        p.line(
                            dx,
                            tp - l,
                            dx,
                            tp + 3.0,
                            |u: f64| {
                                if u > 0.75 {
                                    c[6]
                                } else if u > 0.4 {
                                    c[4]
                                } else {
                                    c[3]
                                }
                            },
                            1.0,
                        );
                    }
                }
            }
            let gl = (t - 0.04) / 0.3;
            icicle(
                p,
                c,
                jx + 0.5,
                tp,
                t >= impact - 0.025 || (SNAP..SNAP + 0.03).contains(&t),
                if (0.0..=1.0).contains(&gl) { gl } else { -1.0 },
            );
            if self.snapped && t < SNAP + 0.12 {
                p.line(
                    -W / 2.0 - 1.0,
                    ROOT + 1.0,
                    W / 2.0 + 1.0,
                    ROOT + 1.0,
                    WHITE,
                    1.0,
                );
            }
            p.glow(0.5, tp + LEN * 0.5, 20.0, c[4], 0.28);
            // the drop of meltwater swelling at the tip
            if t < DRIP {
                let s = seg(t, 0.0, DRIP);
                p.dot(0.5, ROOT + LEN + 1.0, c[6], s);
                if s > 0.6 {
                    p.px(0.5, ROOT + LEN + 2.0, WHITE);
                }
            }
        }
        self.panes.draw(p, c);
        self.bits.draw(p);
        self.ps.draw(p, Layer::Front);
        self.lights.draw(p);
    }
}

impl Icicle {
    fn draw_back(&self, p: &mut Painter) {
        let (t, c) = (self.t, &self.c);
        let impact = impact();
        // its shadow gathers on the floor as it comes down
        if t > TREMBLE && t < impact + 0.05 {
            let k = if t < FREE {
                seg(t, TREMBLE, FREE) * 0.35
            } else {
                0.35 + 0.65 * seg(t, FREE, impact)
            };
            let rx = 3.0 + 7.0 * k;
            let mut x = libm::floor(-rx);
            while x <= rx {
                let f = 1.0 - (x / rx) * (x / rx);
                p.blend(x + 0.5, 0.0, c[0], 0.7 * k * f);
                if libm::fabs(x) < rx * 0.7 {
                    p.blend(x + 0.5, 1.0, c[0], 0.45 * k * f);
                }
                x += 1.0;
            }
        }
        // frost splashed across the floor where it struck, melting away
        if t >= impact {
            let g = ease::out_cubic(seg(t, impact, impact + 0.25));
            let out = 1.0 - smooth(impact + 0.8, impact + 1.5, t);
            for xi in -26..=26 {
                let x = f64::from(xi);
                let d = libm::fabs(x + 0.5) / 26.0;
                let h = rand(&[x, 23.0]);
                let a = out * clamp((g * 1.15 - d) / 0.15, 0.0, 1.0);
                if a <= 0.0 {
                    continue;
                }
                let top = if h < 0.35 || d > g - 0.12 {
                    WHITE
                } else {
                    c[6]
                };
                p.dot(x, 0.0, top, a);
                if h < 0.4 {
                    let n = js_round((1.0 + rand(&[x, 3.0]) * 2.5) * (1.0 - d));
                    let mut j = 1.0;
                    while j <= n {
                        let dx = if j > 1.0 { js_sign(x) } else { 0.0 };
                        p.dot(x + dx, -j, if j > 1.0 { WHITE } else { c[5] }, a);
                        j += 1.0;
                    }
                }
                let n = js_round((2.0 + rand(&[x, 7.0]) * 3.0) * (1.0 - d * 0.6));
                let mut j = 1.0;
                while j <= n {
                    p.dot(x, j, if j < 2.0 { c[5] } else { c[4] }, a * (1.1 - j / 6.0));
                    j += 1.0;
                }
            }
            self.shock(p, true);
        }
        self.ps.draw(p, Layer::Back);
    }
}

// ---- panes of ice that tumble and skid (the engine's `ice/lib.rs`, the solid blade-shaded panes icicle-fall uses)

type Poly = Vec<(f64, f64)>;

/// A polygon's centroid and area.
fn centroid(pts: &[(f64, f64)]) -> (f64, f64, f64) {
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    let mut j = pts.len().wrapping_sub(1);
    for (i, &(x1, y1)) in pts.iter().enumerate() {
        if let Some(&(x0, y0)) = pts.get(j) {
            let k = x0 * y1 - x1 * y0;
            a += k;
            cx += (x0 + x1) * k;
            cy += (y0 + y1) * k;
        }
        j = i;
    }
    if libm::fabs(a) < 1e-6 {
        let n = js_max(1.0, pts.len() as f64);
        return (
            pts.iter().fold(0.0, |s, q| s + q.0) / n,
            pts.iter().fold(0.0, |s, q| s + q.1) / n,
            0.0,
        );
    }
    (cx / (3.0 * a), cy / (3.0 * a), libm::fabs(a) / 2.0)
}

/// Cut a polygon by the line through (x, y) at angle `a`: the pieces with three corners or more.
fn split_poly(pts: &[(f64, f64)], x: f64, y: f64, a: f64) -> Vec<Poly> {
    let (nx, ny) = (-libm::sin(a), libm::cos(a));
    let side = |q: (f64, f64)| (q.0 - x) * nx + (q.1 - y) * ny;
    let (mut l, mut rt): (Poly, Poly) = (Vec::new(), Vec::new());
    for (i, &p) in pts.iter().enumerate() {
        let Some(&q) = pts.get((i + 1) % pts.len()) else {
            continue;
        };
        let (sp, sq) = (side(p), side(q));
        if sp >= 0.0 {
            l.push(p);
        } else {
            rt.push(p);
        }
        if (sp >= 0.0) != (sq >= 0.0) {
            let u = sp / (sp - sq);
            let m = (p.0 + (q.0 - p.0) * u, p.1 + (q.1 - p.1) * u);
            l.push(m);
            rt.push(m);
        }
    }
    [l, rt].into_iter().filter(|s| s.len() >= 3).collect()
}

enum PaneEvent {
    Land { x: f64, y: f64, vx: f64, speed: f64 },
    Crumble { x: f64, vx: f64 },
}

/// A pane of ice: its corners round its middle, flying, turning (`a`) and tipping toward us (`f`); it bounces,
/// shatters on its second hard landing, or skids to rest and crumbles.
struct Pane {
    pts: Poly,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    a: f64,
    va: f64,
    f: f64,
    vf: f64,
    life: f64,
    rest: bool,
    crumble: f64,
    hits: u32,
    rest_t: f64,
}

const LIT: f64 = 0.75;
const G: f64 = 0.9;
const DRAG: f64 = 0.6;
const BOUNCE: f64 = 0.35;
const BRITTLE: u32 = 2;
const RIDGE: f64 = FRAC_PI_2;

impl Pane {
    fn new(pts: Poly, x: f64, y: f64) -> Self {
        Pane {
            pts,
            x,
            y,
            vx: 0.0,
            vy: 0.0,
            a: 0.0,
            va: 0.0,
            f: 0.0,
            vf: 0.0,
            life: 1.0,
            rest: false,
            crumble: 0.3,
            hits: 0,
            rest_t: 0.0,
        }
    }

    fn corners(&self) -> Poly {
        let (ca, sa, k) = (libm::cos(self.a), libm::sin(self.a), libm::cos(self.f));
        self.pts
            .iter()
            .map(|&(x, y)| {
                let yy = y * k;
                (self.x + x * ca - yy * sa, self.y + x * sa + yy * ca)
            })
            .collect()
    }

    fn seat(&self) -> f64 {
        self.corners()
            .iter()
            .fold(f64::NEG_INFINITY, |low, c| js_max(low, c.1))
    }
}

struct Panes {
    list: Vec<Pane>,
    ground: f64,
}

impl Panes {
    fn new(ground: f64) -> Self {
        Panes {
            list: Vec::new(),
            ground,
        }
    }

    fn add(&mut self, q: Pane) {
        self.list.push(q);
    }

    fn update(&mut self, dt: f64, mut on: impl FnMut(PaneEvent)) {
        let floor = self.ground - 0.5;
        for i in (0..self.list.len()).rev() {
            let Some(q) = self.list.get_mut(i) else {
                continue;
            };
            q.life -= dt;
            if q.life <= 0.0 {
                self.list.remove(i);
                continue;
            }
            if q.rest {
                q.rest_t += dt;
                if q.rest_t > q.crumble {
                    on(PaneEvent::Crumble { x: q.x, vx: q.vx });
                    self.list.remove(i);
                    continue;
                }
                // skidding: friction slows the slide and the spin, the pane stays seated on the floor
                q.x += q.vx * dt;
                q.vx -= q.vx * 7.0 * dt;
                q.a += q.va * dt;
                q.va -= q.va * 9.0 * dt;
                q.vf -= q.vf * 9.0 * dt;
                q.f += q.vf * dt;
                q.y -= q.seat() - floor;
                continue;
            }
            q.vy += GRAVITY * G * dt;
            q.vx -= q.vx * DRAG * dt;
            q.vy -= q.vy * DRAG * dt;
            q.x += q.vx * dt;
            q.y += q.vy * dt;
            q.a += q.va * dt;
            q.f += q.vf * dt;
            let low = q.seat();
            if low < floor || q.vy <= 0.0 {
                continue;
            }
            q.y -= low - floor;
            if q.vy > 70.0 {
                q.hits += 1;
                on(PaneEvent::Land {
                    x: q.x,
                    y: floor + 0.5,
                    vx: q.vx,
                    speed: q.vy,
                });
                if q.hits >= BRITTLE {
                    self.list.remove(i);
                    continue;
                }
                q.vy = -q.vy * BOUNCE;
                q.vx *= 0.75;
                q.va *= 0.6;
                q.vf *= 0.5;
            } else {
                q.rest = true;
                q.vy = 0.0;
            }
        }
    }

    /// Each pane shaded like the blade it broke from (a lit side, a pale ridge, a deep side), its edges lit where
    /// they face the light (up and left), a glint on its top corner, a little light.
    fn draw(&self, p: &mut Painter, c: &[Rgba; 8]) {
        let nf = c.len() as f64;
        let step = |i: f64| step_at(c, i);
        for q in &self.list {
            let a = js_min(1.0, q.life / 0.35);
            let pts = q.corners();
            let k = libm::cos(q.f);
            let face = libm::fabs(k);
            let lv = LIT * (0.35 + 0.65 * face);
            let flash = face > 0.97 && !q.rest;
            if face > 0.25 {
                let (ca, sa) = (libm::cos(q.a), libm::sin(q.a));
                let (cr, sr) = (libm::cos(RIDGE), libm::sin(RIDGE));
                let rx = cr * ca - sr * k * sa;
                let ry = cr * sa + sr * k * ca;
                let (mut nx, mut ny) = (-ry, rx);
                if nx * -0.55 + ny * -0.84 < 0.0 {
                    nx = -nx;
                    ny = -ny;
                }
                let lit = if flash {
                    step(nf - 1.0)
                } else {
                    step(clamp(js_round(3.0 + lv * 3.0), 3.0, nf - 2.0))
                };
                let deep = step(clamp(js_round(1.0 + lv * 2.0), 1.0, 3.0));
                let pale = step(nf - 2.0);
                let (qx, qy) = (q.x, q.y);
                p.poly(
                    &pts,
                    move |x: f64, y: f64| {
                        let d = (x - qx) * nx + (y - qy) * ny;
                        if d > 0.6 {
                            lit
                        } else if d > -0.6 {
                            pale
                        } else {
                            deep
                        }
                    },
                    a,
                );
            }
            let n = js_max(1.0, pts.len() as f64);
            let cx = pts.iter().fold(0.0, |s, q| s + q.0) / n;
            let cy = pts.iter().fold(0.0, |s, q| s + q.1) / n;
            let mut top: Option<(f64, f64)> = None;
            for (i, &(x0, y0)) in pts.iter().enumerate() {
                let Some(&(x1, y1)) = pts.get((i + 1) % pts.len()) else {
                    continue;
                };
                let (mut nx, mut ny) = (y1 - y0, x0 - x1);
                if nx * ((x0 + x1) / 2.0 - cx) + ny * ((y0 + y1) / 2.0 - cy) < 0.0 {
                    nx = -nx;
                    ny = -ny;
                }
                let l = libm::hypot(nx, ny);
                let l = if l == 0.0 { 1.0 } else { l };
                let lit = (-nx * 0.55 - ny * 0.84) / l;
                let col = if face < 0.25 || lit > 0.35 {
                    step(nf - 1.0)
                } else if lit > -0.25 {
                    step(nf - 2.0)
                } else {
                    step(1.0)
                };
                p.line(x0, y0, x1, y1, col, a);
                if top.is_none_or(|t| y0 < t.1) {
                    top = Some((x0, y0));
                }
            }
            if face > 0.7
                && let Some((tx, ty)) = top
            {
                p.dot(tx, ty, WHITE, a);
            }
            p.glow(
                cx,
                cy,
                5.0,
                step(nf - 3.0),
                (if flash { 0.5 } else { 0.14 }) * a,
            );
        }
    }
}
