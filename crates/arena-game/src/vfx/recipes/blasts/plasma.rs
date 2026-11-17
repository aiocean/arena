//! Plasma explosion (`explosions/plasma-explosion`): a white flash, a ball of plasma swelling into a thinning shell,
//! shock rings in and out of depth, lightning arcs crackling off it. The duel's most played blast: many schools'
//! seals and blasts, each through its own palette (7 steps, dark to white) and accent (5).

use super::common::{cull, draw_both, seed_of};
use crate::vfx::kit::shapes::{bolt, forks};
use crate::vfx::kit::sim::{Kind, Lights, Particles, ParticlesOpts, Spawn, band, flare};
use crate::vfx::kit::{
    Layer, Opts, Painter, Rgba, Rng, Show, TAU, WHITE, clamp, ease, js_max, js_min, js_round,
    libm, noise, ramp, rng, smooth,
};

pub const DURATION: f64 = 1.7;

const PALETTE: [Rgba; 7] = [
    Rgba::hex24(0x0a0828),
    Rgba::hex24(0x1a1c6e),
    Rgba::hex24(0x2c3cc0),
    Rgba::hex24(0x4a7cf0),
    Rgba::hex24(0x7cc0ff),
    Rgba::hex24(0xc8ecff),
    Rgba::hex24(0xffffff),
];
const ACCENT: [Rgba; 5] = [
    Rgba::hex24(0x1c0a30),
    Rgba::hex24(0x4a1a8c),
    Rgba::hex24(0x8a3ce0),
    Rgba::hex24(0xc48cff),
    Rgba::hex24(0xf0d8ff),
];

const BALL: f64 = 0.55;
// the engine's canvas (224 x 200, origin 112, 100), 3 px in
const X0: f64 = -109.0;
const Y0: f64 = -97.0;
const X1: f64 = 109.0;
const Y1: f64 = 97.0;

pub fn make(o: &Opts) -> Box<dyn Show> {
    let c: [Rgba; 7] = ramp(&o.palette_or(&PALETTE));
    // a palette given with no accent rims in its own colours, not the default violet
    let own = [c[0], c[2], c[3], c[5], c[6]];
    let v: [Rgba; 5] = ramp(&o.accent_or(if o.has_palette() { &own } else { &ACCENT }));
    let mut r = Rng::new(seed_of(o.seed));
    let mut ps = Particles::new(ParticlesOpts {
        sparks: vec![c[6], c[5], c[4], v[3]],
        ..ParticlesOpts::default()
    });
    let mut lights = Lights::new();
    lights.add(0.0, 0.0, 96.0, c[4], 1.0, 0.6);
    lights.add(0.0, 0.0, 40.0, WHITE, 0.9, 0.1);
    lights.add(0.0, 0.0, 70.0, v[2], 0.5, 1.1);
    for _ in 0..46 {
        let a = r.range(0.0, TAU);
        let sp = r.range(140.0, 360.0);
        let life = r.range(0.2, 0.42);
        let col = r.pick(&[c[6], c[5], c[4], v[3]]).copied();
        let size = r.range(0.7, 1.1);
        ps.spawn(
            &mut r,
            Spawn {
                col,
                size,
                ..Spawn::new(
                    Kind::Mote,
                    libm::cos(a) * 4.0,
                    libm::sin(a) * 4.0,
                    libm::cos(a) * sp,
                    libm::sin(a) * sp,
                    life,
                )
            },
        );
    }
    for _ in 0..26 {
        let a = r.range(0.0, TAU);
        let sp = r.range(40.0, 140.0);
        let life = r.range(0.8, 1.5);
        let col = r.pick(&[c[5], v[3], v[4]]).copied();
        let size = r.range(0.6, 1.6);
        ps.spawn(
            &mut r,
            Spawn {
                col,
                size,
                ..Spawn::new(
                    Kind::Flake,
                    libm::cos(a) * 10.0,
                    libm::sin(a) * 10.0,
                    libm::cos(a) * sp,
                    libm::sin(a) * sp,
                    life,
                )
            },
        );
    }
    Box::new(Plasma {
        c,
        v,
        seed: o.seed,
        r,
        ps,
        lights,
        t: 0.0,
    })
}

struct Plasma {
    c: [Rgba; 7],
    v: [Rgba; 5],
    seed: f64,
    r: Rng,
    ps: Particles,
    lights: Lights,
    t: f64,
}

impl Show for Plasma {
    fn update(&mut self, dt: f64) {
        let (c, v, r) = (&self.c, &self.v, &mut self.r);
        self.t += dt;
        let t = self.t;
        // the shell sheds sparkles as it thins
        if t > 0.12 && t < 0.9 && r.chance(30.0 * dt) {
            let a = r.range(0.0, TAU);
            let rs = shell(t);
            let vx = libm::cos(a) * r.range(10.0, 40.0);
            let vy = libm::sin(a) * r.range(10.0, 40.0);
            let life = r.range(0.3, 0.6);
            let col = if r.chance(0.5) { c[5] } else { v[4] };
            let size = r.range(0.5, 1.8);
            self.ps.spawn(
                r,
                Spawn {
                    col: Some(col),
                    size,
                    ..Spawn::new(
                        Kind::Star,
                        libm::cos(a) * rs,
                        libm::sin(a) * rs,
                        vx,
                        vy,
                        life,
                    )
                },
            );
        }
        self.ps.update(r, dt);
        cull(&mut self.ps, X0, Y0, X1, Y1);
        self.lights.update(dt);
    }

    fn draw(&self, p: &mut Painter, _t: f64, layer: Layer) {
        let (c, v, t) = (&self.c, &self.v, self.t);
        if layer == Layer::Back {
            // the tilted ring's far half, behind the ball (and the fighter in it)
            tilted(p, t, c, v, true);
            return;
        }
        // the ball: a white heart, bright and mid bands, an accent rim boiling at the edge; it swells, then hollows
        if t < BALL {
            let u = t / BALL;
            let r = shell(t);
            let hollow = smooth(0.35, 1.0, u);
            let boil = libm::floor(t * 20.0);
            p.shade(-r - 3.0, -r - 3.0, r + 3.0, r + 3.0, |x, y, out| {
                let d = libm::hypot(x, y);
                let a = libm::atan2(y, x);
                let edge = r * (1.0 + 0.1 * noise(a * 3.0 + boil * 0.7, 3.0));
                if d > edge {
                    return None;
                }
                let vv = d / edge;
                if vv < hollow * 0.92 {
                    return None;
                }
                let n = noise(a * 4.0 + d * 0.25 + boil * 0.5, 5.0);
                let w = vv + n * 0.14;
                let fil = libm::fabs(libm::sin(a * 5.0 + d * 0.32 - t * 14.0 + n * 2.5)) < 0.1;
                out.k = 0.25 * (1.0 - u);
                if vv > 0.9 {
                    return Some(v[2]);
                }
                if fil && vv > 0.3 {
                    return Some(if u < 0.5 { WHITE } else { c[5] });
                }
                Some(if w < 0.32 - u * 0.25 {
                    WHITE
                } else if w < 0.52 {
                    c[5]
                } else if w < 0.72 {
                    c[4]
                } else if w < 0.86 {
                    if u < 0.5 { c[3] } else { v[3] }
                } else {
                    v[2]
                })
            });
            p.glow(0.0, 0.0, r * 1.6 + 6.0, c[4], 0.6 * (1.0 - u));
        }
        if t < 0.07 {
            p.circle(0.0, 0.0, 9.0 - t * 60.0, WHITE, 1.0);
        }
        if t < 0.25 {
            flare(
                p,
                0.0,
                0.0,
                52.0 * libm::pow(1.0 - t / 0.25, 1.3),
                1.0 - t / 0.25,
                c,
            );
        }
        // shock rings: a fast bright one, a slower accent one
        let u1 = t / 0.42;
        if u1 < 1.0 {
            let rr = 8.0 + 84.0 * ease::out_cubic(u1);
            p.ring(
                0.0,
                0.0,
                rr,
                js_max(1.0, 3.0 * (1.0 - u1)),
                if u1 < 0.25 { WHITE } else { c[5] },
                1.15 - u1,
            );
            p.glow_ring(0.0, 0.0, rr, 5.0, c[4], 0.35 * (1.0 - u1));
        }
        let u2 = (t - 0.06) / 0.6;
        if u2 > 0.0 && u2 < 1.0 {
            p.ring(
                0.0,
                0.0,
                6.0 + 60.0 * ease::out_cubic(u2),
                js_max(1.0, 2.0 * (1.0 - u2)),
                v[3],
                1.1 - u2,
            );
        }
        // arcs: lightning out of the heart, re-struck every few frames, shorter as the charge runs out
        let left = 1.0 - smooth(0.15, 0.85, t);
        let strike = libm::floor(t * 18.0);
        if left > 0.0 {
            let mut r = rng(&[self.seed, strike, 17.0]);
            let n = 2.0 + js_round(4.0 * left);
            let mut i = 0.0;
            while i < n {
                let a = r.range(0.0, TAU);
                let l = r.range(26.0, 58.0) * (0.45 + 0.55 * left);
                let path = bolt(
                    &mut r,
                    0.0,
                    0.0,
                    libm::cos(a) * l,
                    libm::sin(a) * l,
                    0.28,
                    4,
                );
                arc(p, &path, c, v, 0.8 * left);
                if r.chance(0.5) {
                    for f in forks(&mut r, &path, 1, 14.0, 0.35, 0.9) {
                        arc(p, &f, c, v, 0.5 * left);
                    }
                }
                i += 1.0;
            }
            // and little arcs crawling over the shell
            let rs = shell(t);
            for _ in 0..3 {
                let a = r.range(0.0, TAU);
                let b = a + r.range(0.3, 0.6);
                let path = bolt(
                    &mut r,
                    libm::cos(a) * rs,
                    libm::sin(a) * rs,
                    libm::cos(b) * rs,
                    libm::sin(b) * rs,
                    0.4,
                    3,
                );
                arc(p, &path, c, v, 0.6 * left);
            }
        }
        tilted(p, t, c, v, false);
        draw_both(p, &self.ps);
        self.lights.draw(p);
    }
}

/// The ball's radius: punched out fast, then easing.
fn shell(t: f64) -> f64 {
    5.0 + 30.0 * ease::out_cubic(clamp(t / BALL, 0.0, 1.0))
}

/// A ring on a tilted plane racing out round the blast: far half behind, near half in front.
fn tilted(p: &mut Painter, t: f64, c: &[Rgba; 7], v: &[Rgba; 5], back: bool) {
    let u = (t - 0.03) / 0.5;
    if u <= 0.0 || u >= 1.0 {
        return;
    }
    band(
        p,
        0.0,
        4.0,
        12.0 + 86.0 * ease::out_cubic(u),
        0.28,
        js_max(1.0, 4.0 * (1.0 - u)),
        1.15 - u,
        &[v[1], v[2], c[4], c[5]],
        |g| (libm::sin(g) < 0.0) == back,
        u < 0.15,
    );
}

/// One arc: a soft glow along it, a halo line, a white core.
fn arc(p: &mut Painter, path: &[(f64, f64)], c: &[Rgba; 7], v: &[Rgba; 5], k: f64) {
    if k <= 0.05 {
        return;
    }
    let n = path.len() as f64;
    for (i, w) in path.windows(2).enumerate() {
        let &[(ax, ay), (bx, by)] = w else {
            continue;
        };
        p.glow_line(ax, ay, bx, by, 5.0, v[2], 0.18 * k);
        p.line(ax + 1.0, ay, bx + 1.0, by, c[4], k * 0.8);
        let core = if ((i + 1) as f64) < n * 0.6 {
            WHITE
        } else {
            c[5]
        };
        p.line(ax, ay, bx, by, core, js_min(1.0, k * 1.4));
    }
}
