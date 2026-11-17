//! `holy/feather-burst`: a white puff pops and throws feathers that brake hard, then rock from side to side as they
//! sink and fade, with down drifting after them (the engine's `holy/feather-burst`, with the holy kit's feathers).

use crate::vfx::kit::sim::{Kind, Lights, Particles, ParticlesOpts, Spawn, sparkle};
use crate::vfx::kit::{
    Layer, Opts, PI, Painter, Rgba, Rng, Show, TAU, WHITE, clamp, ease, js_max, js_min, js_round,
    libm, mix, ramp, smooth,
};

pub const D: f64 = 3.0;

const PALETTE: [Rgba; 6] = [
    Rgba::hex24(0x4a4560),
    Rgba::hex24(0x7a7494),
    Rgba::hex24(0xaaa5c2),
    Rgba::hex24(0xd6d3e6),
    Rgba::hex24(0xf4f3fa),
    Rgba::hex24(0xffffff),
];
const GLOW: Rgba = Rgba::hex24(0xfff0c0);

/// A feather of light from its quill at (x, y) pointing along `ang`, `len` long, its trailing vane `wid` wide on
/// `side`: a white shaft, a lit leading rim, a dark trailing edge, a body cooling from cream to gold, a tip thinning
/// into light. `hot` over 0.5 whitens it.
#[allow(clippy::too_many_arguments)]
fn feather(
    p: &mut Painter,
    x: f64,
    y: f64,
    ang: f64,
    len: f64,
    wid: f64,
    ramp: &[Rgba],
    a: f64,
    side: f64,
    hot: f64,
    quill: f64,
) {
    if len < 1.5 || a <= 0.02 {
        return;
    }
    let n = ramp.len();
    let at = |k: usize| {
        n.checked_sub(k)
            .and_then(|i| ramp.get(i))
            .copied()
            .unwrap_or(Rgba::NONE)
    };
    let (ca, sa) = (libm::cos(ang), libm::sin(ang));
    let (x1, y1) = (x + ca * len, y + sa * len);
    let pad = wid + 1.2;
    let h = hot > 0.5;
    let edge = if h { at(3) } else { at(5) };
    let lead = if h { WHITE } else { at(2) };
    let b = if h {
        [WHITE, WHITE, at(2)]
    } else {
        [at(2), at(3), at(4)]
    };
    p.each(
        js_min(x, x1) - pad,
        js_min(y, y1) - pad,
        js_max(x, x1) + pad,
        js_max(y, y1) + pad,
        |p, px, py, sx, sy| {
            let (dx, dy) = (px - x, py - y);
            let u = (dx * ca + dy * sa) / len;
            if !(0.0..=1.05).contains(&u) {
                return;
            }
            let v = (-dx * sa + dy * ca) * side;
            let av = v.abs();
            let t = js_max(0.0, (u - 0.55) / 0.47);
            let w = wid
                * libm::sqrt(clamp((u - quill) / 0.22, 0.0, 1.0))
                * libm::sqrt(js_max(0.0, 1.0 - t * t));
            let lim = if v < 0.0 { w * 0.42 } else { w };
            let fade = if u < 0.72 {
                1.0
            } else {
                1.0 - (u - 0.72) * 1.6
            };
            if av < 0.5 && u < 0.9 {
                return p.put(sx, sy, if u < 0.5 { WHITE } else { b[0] }, a);
            }
            if av > lim + 0.05 || w < 0.5 {
                return;
            }
            if lim - av < 1.0 {
                return if v > 0.0 {
                    p.put(sx, sy, if u > 0.85 { b[2] } else { edge }, a * fade)
                } else {
                    p.put(sx, sy, lead, a)
                };
            }
            if u < 0.72 {
                return p.put(sx, sy, if u < 0.38 { b[0] } else { b[1] }, a);
            }
            p.blend_px(sx, sy, b[2], a * fade);
        },
    );
}

/// One loose feather: where it is, how it moves and how it rocks.
struct Loose {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    a: f64,
    len: f64,
    wid: f64,
    life: f64,
    delay: f64,
    back: bool,
    side: f64,
    ph: f64,
    w: f64,
    amp: f64,
    sink: f64,
    va: f64,
    tilt: f64,
    max: f64,
    age: f64,
}

/// What a new feather is given; `sink` and `amp` replace the random ones when set (drawn either way).
struct NewFeather {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    a: f64,
    len: f64,
    wid: f64,
    life: f64,
    delay: f64,
    back: bool,
    sink: f64,
    amp: Option<f64>,
}

/// Loose feathers thrown out, braking hard, then rocking like a pendulum as they sink (no floor here: the burst
/// is high in the air and they fade first).
struct Feathers {
    list: Vec<Loose>,
}

impl Feathers {
    fn add(&mut self, r: &mut Rng, f: NewFeather) {
        let side = r.sign();
        let ph = r.range(0.0, TAU);
        let w = r.range(3.2, 4.6);
        let amp = r.range(6.0, 11.0);
        let _sink = r.range(14.0, 24.0);
        let va = r.range(-9.0, 9.0);
        let tilt = r.range(-0.35, 0.35);
        self.list.push(Loose {
            x: f.x,
            y: f.y,
            vx: f.vx,
            vy: f.vy,
            a: f.a,
            len: f.len,
            wid: f.wid,
            life: f.life,
            delay: f.delay,
            back: f.back,
            side,
            ph,
            w,
            amp: f.amp.unwrap_or(amp),
            sink: f.sink,
            va,
            tilt,
            max: f.life,
            age: 0.0,
        });
    }

    fn update(&mut self, dt: f64) {
        self.list.retain_mut(|f| {
            if f.delay > 0.0 {
                f.delay -= dt;
                return true;
            }
            f.life -= dt;
            if f.life <= 0.0 {
                return false;
            }
            f.age += dt;
            // the throw: drag kills it fast while it spins
            f.vx -= f.vx * 3.4 * dt;
            f.vy -= f.vy * 3.4 * dt;
            let s = smooth(0.1, 0.6, f.age);
            let ph = f.age * f.w + f.ph;
            let c = libm::cos(ph);
            // the flutter: a swing across, a sink that's fastest through the bottom of each swing
            f.x += (f.vx + libm::sin(ph) * f.amp * s * 0.9) * dt;
            f.y += (f.vy + f.sink * s * (c * c * 1.5 - 0.2)) * dt;
            // it lies along its swing, tipping up at each end
            let rest = js_round(f.a / PI) * PI + f.tilt + libm::sin(ph) * 0.6;
            f.a += f.va * dt * (1.0 - s) + (rest - f.a) * js_min(1.0, dt * 5.0 * s);
            true
        });
    }

    /// The feathers of one layer: the small ones behind, the rest in front.
    fn draw(&self, p: &mut Painter, ramp: &[Rgba], layer: Layer) {
        let glow_col = ramp
            .len()
            .checked_sub(3)
            .and_then(|i| ramp.get(i))
            .copied()
            .unwrap_or(Rgba::NONE);
        for f in &self.list {
            let this = if f.back { Layer::Back } else { Layer::Front };
            if f.delay > 0.0 || this != layer {
                continue;
            }
            let k = f.life / f.max;
            let a = js_min(js_min(1.0, f.life / 0.45), f.age / 0.06);
            let (ca, sa) = (libm::cos(f.a), libm::sin(f.a));
            feather(
                p,
                f.x - (ca * f.len) / 2.0,
                f.y - (sa * f.len) / 2.0,
                f.a,
                f.len,
                f.wid,
                ramp,
                a,
                f.side,
                if f.age < 0.06 { 1.0 } else { 0.0 },
                0.2,
            );
            p.glow(
                f.x,
                f.y,
                4.0 + f.len * 0.5,
                glow_col,
                0.22 * a * (0.4 + 0.6 * k),
            );
        }
    }

    fn pick(&self, r: &mut Rng) -> Option<(f64, f64, f64)> {
        r.pick(&self.list).map(|f| (f.x, f.y, f.delay))
    }
}

/// One lobe of the puff: soft, thrown out from the pop, swelling and thinning.
struct Lobe {
    x: f64,
    y: f64,
    r0: f64,
    r1: f64,
    life: f64,
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    let f: [Rgba; 6] = ramp(&o.palette_or(&PALETTE));
    let g = o.glow_or(GLOW);
    let mut r = Rng::new(o.seed as u32);
    let mut ps = Particles::new(ParticlesOpts {
        ground: f64::INFINITY,
        sparks: vec![WHITE, f[4], f[3], f[2]],
        ..ParticlesOpts::default()
    });
    let mut lights = Lights::new();
    let mut loose = Feathers { list: Vec::new() };
    let mut lobes = Vec::new();
    for i in 0..11 {
        let a = (f64::from(i) / 11.0) * TAU + r.range(-0.25, 0.25);
        let d = r.range(5.0, 14.0);
        let x = libm::cos(a) * d;
        let y = libm::sin(a) * d * 0.8;
        let r0 = r.range(2.5, 4.0);
        let r1 = r.range(6.0, 9.5);
        let life = r.range(0.45, 0.8);
        lobes.push(Lobe { x, y, r0, r1, life });
    }
    for i in 0..20 {
        let a = (f64::from(i) / 20.0) * TAU + r.range(-0.15, 0.15);
        let sp = r.range(110.0, 230.0);
        let up = if libm::sin(a) < 0.0 { 0.7 } else { 0.9 };
        let (x, y) = (libm::cos(a) * 3.0, libm::sin(a) * 3.0);
        let (vx, vy) = (libm::cos(a) * sp, libm::sin(a) * sp * up - 20.0);
        let a = a + r.range(-0.5, 0.5);
        let len = r.range(10.0, 14.0);
        let wid = r.range(2.6, 3.4);
        let life = r.range(1.9, 2.6);
        let delay = r.range(0.0, 0.03);
        let sink = r.range(26.0, 38.0);
        let amp = r.range(8.0, 13.0);
        loose.add(
            &mut r,
            NewFeather {
                x,
                y,
                vx,
                vy,
                a,
                len,
                wid,
                life,
                delay,
                back: false,
                sink,
                amp: Some(amp),
            },
        );
    }
    // small feathers and down, slower
    for _ in 0..10 {
        let a = r.range(0.0, TAU);
        let sp = r.range(50.0, 130.0);
        let (vx, vy) = (libm::cos(a) * sp, libm::sin(a) * sp - 20.0);
        let a = r.range(0.0, TAU);
        let len = r.range(6.0, 8.0);
        let wid = r.range(1.8, 2.2);
        let life = r.range(1.6, 2.4);
        let sink = r.range(18.0, 28.0);
        loose.add(
            &mut r,
            NewFeather {
                x: 0.0,
                y: 0.0,
                vx,
                vy,
                a,
                len,
                wid,
                life,
                delay: 0.0,
                back: true,
                sink,
                amp: None,
            },
        );
    }
    for _ in 0..26 {
        let a = r.range(0.0, TAU);
        let sp = r.range(30.0, 140.0);
        let x = r.range(-3.0, 3.0);
        let y = r.range(-3.0, 3.0);
        let life = r.range(1.4, 2.6);
        let col = if r.chance(0.6) { WHITE } else { f[4] };
        let size = r.range(0.6, 1.6);
        ps.spawn(
            &mut r,
            Spawn {
                col: Some(col),
                size,
                ..Spawn::new(
                    Kind::Flake,
                    x,
                    y,
                    libm::cos(a) * sp,
                    libm::sin(a) * sp - 15.0,
                    life,
                )
            },
        );
    }
    for _ in 0..10 {
        let a = r.range(0.0, TAU);
        let sp = r.range(60.0, 160.0);
        let life = r.range(0.3, 0.6);
        let col = if r.chance(0.5) { WHITE } else { g };
        let size = r.range(0.8, 2.2);
        ps.spawn(
            &mut r,
            Spawn {
                col: Some(col),
                size,
                ..Spawn::new(
                    Kind::Star,
                    0.0,
                    0.0,
                    libm::cos(a) * sp,
                    libm::sin(a) * sp,
                    life,
                )
            },
        );
    }
    lights.add(0.0, 0.0, 50.0, g, 0.75, 0.45);
    lights.add(0.0, 0.0, 20.0, WHITE, 0.7, 0.1);
    Box::new(Burst {
        f,
        g,
        r,
        ps,
        lights,
        loose,
        lobes,
        t: 0.0,
    })
}

struct Burst {
    f: [Rgba; 6],
    g: Rgba,
    r: Rng,
    ps: Particles,
    lights: Lights,
    loose: Feathers,
    lobes: Vec<Lobe>,
    t: f64,
}

impl Burst {
    fn puff(&self, p: &mut Painter) {
        let (f, t) = (&self.f, self.t);
        for l in &self.lobes {
            let u = t / l.life;
            if u >= 1.0 {
                continue;
            }
            let e = ease::out_cubic(u);
            let x = l.x * (0.4 + e * 1.3);
            let y = l.y * (0.4 + e * 1.3) - u * 5.0;
            let r = (l.r0 + (l.r1 - l.r0) * ease::out_cubic(js_min(1.0, u * 3.0)))
                * (1.0 - ease::in_quad(u));
            if r < 0.6 {
                continue;
            }
            p.circle(x, y, r + 0.8, f[2], 1.0);
            let (f3, f4) = (f[3], f[4]);
            p.circle(
                x,
                y,
                r,
                move |d: f64, a: f64| {
                    if d > 0.55 && libm::sin(a + 0.8) > 0.3 {
                        f3
                    } else if d < 0.45 && libm::sin(a + 0.8) < -0.2 {
                        WHITE
                    } else {
                        f4
                    }
                },
                1.0,
            );
        }
    }
}

impl Show for Burst {
    fn update(&mut self, dt: f64) {
        self.t += dt;
        // feathers catch the light as they turn
        if self.t > 0.3 && self.t < 2.2 && !self.loose.list.is_empty() && self.r.chance(0.14) {
            let picked = self.loose.pick(&mut self.r);
            if let Some((qx, qy, _)) = picked.filter(|&(_, _, delay)| delay <= 0.0) {
                let x = qx + self.r.range(-2.0, 2.0);
                let y = qy + self.r.range(-1.0, 1.0);
                let life = self.r.range(0.2, 0.4);
                let col = if self.r.chance(0.5) { WHITE } else { self.g };
                let size = self.r.range(1.0, 2.2);
                self.ps.spawn(
                    &mut self.r,
                    Spawn {
                        col: Some(col),
                        size,
                        ..Spawn::new(Kind::Star, x, y, 0.0, 0.0, life)
                    },
                );
            }
        }
        self.ps.update(&mut self.r, dt);
        self.loose.update(dt);
        self.lights.update(dt);
    }

    fn draw(&self, p: &mut Painter, _t: f64, layer: Layer) {
        let (f, g, t) = (&self.f, self.g, self.t);
        if layer == Layer::Back {
            self.loose.draw(p, f, Layer::Back);
            self.ps.draw(p, Layer::Back);
            self.puff(p);
            return;
        }
        // the pop: a white flash and a ring of air
        if t < 0.07 {
            p.circle(0.0, 0.0, 9.0 * (1.0 - t / 0.14), WHITE, 1.0);
        }
        if t < 0.25 {
            let u = t / 0.25;
            let air = mix(f[4], g, 0.4);
            p.ring(
                0.0,
                0.0,
                6.0 + 24.0 * ease::out_cubic(u),
                js_max(1.0, 2.5 * (1.0 - u)),
                move |v: f64, _: f64| if v > 0.5 { WHITE } else { air },
                1.1 - u,
            );
        }
        if t < 0.2 {
            sparkle(p, 0.0, 0.0, 10.0 * (1.0 - t / 0.2), WHITE, g, 0.6);
        }
        // the burst's warmth lingers a moment
        p.glow(0.0, 4.0, 46.0, g, 0.28 * (1.0 - smooth(0.0, 1.3, t)));
        self.loose.draw(p, f, Layer::Front);
        self.ps.draw(p, Layer::Front);
        self.lights.draw(p);
    }
}
