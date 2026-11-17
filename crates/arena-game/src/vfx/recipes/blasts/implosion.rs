//! Implosion (`explosions/implosion`): streaks and grit rush inward, rings contract and a dark sphere collapses
//! onto a crushed white point; a beat of stillness (the rules pull until 0.8 s), then a sharp pop at 1.02 s.

use super::common::{cull, draw_both, seed_of};
use crate::vfx::kit::sim::{Kind, Lights, Particles, ParticlesOpts, Spawn, sparkle};
use crate::vfx::kit::{
    Layer, Opts, Painter, Rgba, Rng, Show, TAU, WHITE, clamp, ease, js_max, libm, mix, ramp,
};
use core::f64::consts::PI;

pub const DURATION: f64 = 1.65;

const PALETTE: [Rgba; 7] = [
    Rgba::hex24(0x0a0614),
    Rgba::hex24(0x1c1236),
    Rgba::hex24(0x3a2470),
    Rgba::hex24(0x6a44c0),
    Rgba::hex24(0xa080f0),
    Rgba::hex24(0xd8c8ff),
    Rgba::hex24(0xffffff),
];

const SUCK: f64 = 0.8;
const STILL: f64 = 1.02;
const VOID: Rgba = Rgba::hex24(0x05040a);
// the engine's canvas (208 x 184, origin 104, 92), 3 px in
const X0: f64 = -101.0;
const Y0: f64 = -89.0;
const X1: f64 = 101.0;
const Y1: f64 = 89.0;

/// A speck pulled in, spiralling from angle `a` and distance `d` to the middle between `t0` and `t0 + big_t`.
struct Grit {
    a: f64,
    d: f64,
    t0: f64,
    big_t: f64,
    spin: f64,
    c: Rgba,
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    let c: [Rgba; 7] = ramp(&o.palette_or(&PALETTE));
    let mut r = Rng::new(seed_of(o.seed));
    let ps = Particles::new(ParticlesOpts {
        sparks: vec![c[6], c[5], c[4], c[3]],
        ..ParticlesOpts::default()
    });
    let grit = (0..70)
        .map(|_| {
            let t0 = r.range(0.0, 0.4);
            let a = r.range(0.0, TAU);
            let d = r.range(40.0, 86.0);
            let big_t = r.range(0.3, SUCK - 0.04 - t0);
            let spin = r.sign() * r.range(0.6, 1.6);
            let col = r
                .pick(&[
                    c[2],
                    c[3],
                    c[4],
                    Rgba::hex24(0x4a4458),
                    Rgba::hex24(0x6e6880),
                ])
                .copied()
                .unwrap_or(Rgba::NONE);
            Grit {
                a,
                d,
                t0,
                big_t,
                spin,
                c: col,
            }
        })
        .collect();
    let rays = core::array::from_fn(|i| (i as f64 * TAU) / 4.0 + PI / 4.0);
    Box::new(Implosion {
        c,
        r,
        ps,
        lights: Lights::new(),
        grit,
        rays,
        t: 0.0,
        popped: false,
    })
}

struct Implosion {
    c: [Rgba; 7],
    r: Rng,
    ps: Particles,
    lights: Lights,
    grit: Vec<Grit>,
    rays: [f64; 4],
    t: f64,
    popped: bool,
}

impl Show for Implosion {
    fn update(&mut self, dt: f64) {
        let (c, r) = (&self.c, &mut self.r);
        self.t += dt;
        let t = self.t;
        if t < SUCK - 0.26 {
            // streaks of light rushing in from all round, accelerating, more of them as it goes
            let k = t / SUCK;
            let mut n = 0.0;
            while n < 1.0 + libm::floor(k * 4.0) {
                n += 1.0;
                if !r.chance(0.9) {
                    continue;
                }
                let a = r.range(0.0, TAU);
                let d = r.range(34.0, 82.0) * (1.0 - 0.3 * k);
                let big_t = r.range(0.14, 0.24);
                let v = (d * 3.0) / (libm::exp(3.0 * big_t) - 1.0);
                let col = if r.chance(0.5) { c[5] } else { c[4] };
                self.ps.spawn(
                    r,
                    Spawn {
                        col: Some(col),
                        ..Spawn::new(
                            Kind::Streak,
                            libm::cos(a) * d,
                            libm::sin(a) * d,
                            -libm::cos(a) * v,
                            -libm::sin(a) * v,
                            big_t,
                        )
                    },
                );
            }
            // vapour drawn in along the way
            if r.chance(14.0 * dt) {
                let a = r.range(0.0, TAU);
                let d = r.range(50.0, 80.0);
                let life = r.range(0.4, 0.6);
                let size = r.range(3.0, 5.0);
                self.ps.spawn(
                    r,
                    Spawn {
                        col: Some(c[3]),
                        size,
                        ..Spawn::new(
                            Kind::Vapour,
                            libm::cos(a) * d,
                            libm::sin(a) * d,
                            -libm::cos(a) * 90.0,
                            -libm::sin(a) * 90.0,
                            life,
                        )
                    },
                );
            }
        }
        if !self.popped && t >= STILL {
            self.popped = true;
            self.lights.add(0.0, 0.0, 90.0, c[4], 1.0, 0.4);
            self.lights.add(0.0, 0.0, 30.0, WHITE, 0.9, 0.08);
            for _ in 0..34 {
                let a = r.range(0.0, TAU);
                let sp = r.range(120.0, 330.0);
                let kind = if r.chance(0.6) {
                    Kind::Mote
                } else {
                    Kind::Star
                };
                let life = r.range(0.2, 0.45);
                let col = r.pick(&[c[6], c[5], c[4]]).copied();
                let size = r.range(0.8, 1.8);
                self.ps.spawn(
                    r,
                    Spawn {
                        col,
                        size,
                        ..Spawn::new(kind, 0.0, 0.0, libm::cos(a) * sp, libm::sin(a) * sp, life)
                    },
                );
            }
        }
        self.ps.update(r, dt);
        cull(&mut self.ps, X0, Y0, X1, Y1);
        self.lights.update(dt);
    }

    fn draw(&self, p: &mut Painter, _t: f64, layer: Layer) {
        if layer == Layer::Back {
            return;
        }
        let (c, t) = (&self.c, self.t);
        let k = clamp(t / SUCK, 0.0, 1.0);
        if t < STILL {
            // the void: a dark sphere edged with light, collapsing onto the core
            let vr = 64.0 * (1.0 - ease::in_cubic(k)) + 3.0;
            if k < 1.0 {
                p.disc(0.0, 0.0, vr, VOID, 0.35 + 0.35 * k);
                p.ring(
                    0.0,
                    0.0,
                    vr + 1.0,
                    1.0,
                    if k < 0.5 { c[3] } else { c[4] },
                    0.5 + 0.5 * k,
                );
                p.glow_ring(0.0, 0.0, vr, 5.0, c[4], 0.18 + 0.3 * k);
            }
            // rings pulled in after it, one after another, brighter as they go
            for i in 0..4 {
                let fi = f64::from(i);
                let u = (t - fi * 0.17) / 0.34;
                if u <= 0.0 || u >= 1.0 {
                    continue;
                }
                let r = (70.0 - fi * 8.0) * (1.0 - ease::in_quad(u)) + 2.0;
                p.ring(
                    0.0,
                    0.0,
                    r,
                    if u > 0.7 { 2.0 } else { 1.0 },
                    if u > 0.6 { c[5] } else { c[3] },
                    0.4 + 0.6 * u,
                );
            }
            // the grit, spiralling in
            for q in &self.grit {
                let u = (t - q.t0) / q.big_t;
                if u <= 0.0 || u >= 1.0 {
                    continue;
                }
                let d = q.d * (1.0 - ease::in_cubic(u));
                let a = q.a + q.spin * ease::in_quad(u) * 2.0;
                let (x, y) = (libm::cos(a) * d, libm::sin(a) * d);
                let x2 = libm::cos(a - q.spin * 0.1) * (d + 4.0 * u * u + 0.5);
                let y2 = libm::sin(a - q.spin * 0.1) * (d + 4.0 * u * u + 0.5);
                let (tail, col) = (mix(q.c, VOID, 0.4), q.c);
                p.line(x2, y2, x, y, move |v: f64| if v < 0.5 { tail } else { col }, 1.0);
            }
            // the core: hotter as it is crushed, then a white point trembling in the stillness
            let still = t > SUCK;
            let jit = if still {
                if libm::floor(t * 40.0) % 2.0 != 0.0 {
                    0.5
                } else {
                    -0.5
                }
            } else {
                0.0
            };
            let cr = if still {
                1.6
            } else {
                1.0 + 3.5 * (1.0 - k) * k * 4.0
            };
            p.disc(jit, 0.0, cr + 3.5, VOID, 0.85);
            p.circle(jit, 0.0, cr + 1.2, c[4], 1.0);
            p.circle(jit, 0.0, cr, WHITE, 1.0);
            p.glow(0.0, 0.0, 10.0 + 26.0 * k, c[4], 0.3 + 0.5 * k);
            if still {
                p.glow(0.0, 0.0, 8.0, WHITE, 0.4 + 0.2 * libm::sin(t * 60.0));
            }
        } else {
            // the pop: a white point bursting, a cross flare, a fast thin ring and a slower one
            let q = (t - STILL) / 0.5;
            let e = ease::out_cubic(clamp(q, 0.0, 1.0));
            if q < 0.08 {
                p.circle(0.0, 0.0, 6.0 * (1.0 - q / 0.08) + 1.0, WHITE, 1.0);
            }
            if q < 0.35 {
                let l = (1.0 - q / 0.35) * 34.0 + 3.0;
                for &a in &self.rays {
                    let mut d = 0.0;
                    while d <= l {
                        let col = if d < l * 0.4 { WHITE } else { c[5] };
                        let al = if d < l * 0.7 {
                            1.0
                        } else {
                            1.0 - (d / l - 0.7) / 0.3
                        };
                        p.dot(libm::cos(a) * d, libm::sin(a) * d, col, al);
                        d += 0.5;
                    }
                }
                for &a in &self.rays {
                    let mut d = 0.0;
                    while d <= l * 0.45 {
                        p.dot(
                            libm::cos(a + PI / 4.0) * d,
                            libm::sin(a + PI / 4.0) * d,
                            c[5],
                            1.0 - d / (l * 0.45),
                        );
                        d += 0.5;
                    }
                }
            }
            if q < 1.0 {
                p.ring(
                    0.0,
                    0.0,
                    4.0 + 74.0 * e,
                    js_max(1.0, 2.5 * (1.0 - q)),
                    if q < 0.2 { WHITE } else { c[5] },
                    1.15 - q,
                );
                p.ring(
                    0.0,
                    0.0,
                    2.0 + 40.0 * ease::out_cubic(clamp(q * 1.4, 0.0, 1.0)),
                    1.0,
                    c[4],
                    1.0 - q,
                );
            }
            if q < 0.6 {
                sparkle(p, 0.0, 0.0, 5.0 * (1.0 - q / 0.6), WHITE, c[5], 0.6);
            }
        }
        draw_both(p, &self.ps);
        self.lights.draw(p);
    }
}
