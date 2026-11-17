//! A small particle kit the recipes build on: colours, deterministic dice and noise, a painter that draws hard-edged
//! pixels and adds light on the world's pictures, closed-form and simulated particles, and the frame clock every
//! effect plays on.
//!
//! The pixfx engine's effects (where the duel's spells were first drawn) are pixel-art code against its painter; this
//! kit speaks the same language (`Painter::px`, `dot`, `circle`, `ring`, `glow`, `shade`, ...; `Rng`, `rand`,
//! `noise`; `Particles` of sparks, smoke, fire), so a recipe here reads like the effect it reproduces. All in `f64`,
//! in world pixels, y down, the effect's own origin at (0, 0) mirrored when it faces left.
//!
//! Only what is drawn uses this: the rules never see it, so its float maths need not be bit-equal across targets (the
//! `libm` here is std's, under the name the engine's code uses).

#![allow(clippy::too_many_arguments)]

pub mod blob;
pub mod halo;
pub mod shapes;
pub mod sweep;
pub mod sim;

use super::{Effect, Instance, Paint, Spawn};
use crate::raster::Canvas;

pub use core::f64::consts::{PI, TAU};

// ---------------------------------------------------------------------------------------------------------------
// maths

/// Float maths under the names the engine's code uses (std's, for drawing only).
#[allow(clippy::disallowed_methods)]
pub mod libm {
    pub fn sin(x: f64) -> f64 {
        x.sin()
    }
    pub fn cos(x: f64) -> f64 {
        x.cos()
    }
    pub fn tan(x: f64) -> f64 {
        x.tan()
    }
    pub fn asin(x: f64) -> f64 {
        x.asin()
    }
    pub fn acos(x: f64) -> f64 {
        x.acos()
    }
    pub fn atan(x: f64) -> f64 {
        x.atan()
    }
    pub fn atan2(y: f64, x: f64) -> f64 {
        y.atan2(x)
    }
    pub fn sqrt(x: f64) -> f64 {
        x.sqrt()
    }
    pub fn hypot(x: f64, y: f64) -> f64 {
        x.hypot(y)
    }
    pub fn floor(x: f64) -> f64 {
        x.floor()
    }
    pub fn ceil(x: f64) -> f64 {
        x.ceil()
    }
    pub fn round(x: f64) -> f64 {
        x.round()
    }
    pub fn trunc(x: f64) -> f64 {
        x.trunc()
    }
    pub fn pow(x: f64, y: f64) -> f64 {
        x.powf(y)
    }
    pub fn exp(x: f64) -> f64 {
        x.exp()
    }
    pub fn log(x: f64) -> f64 {
        x.ln()
    }
    pub fn fabs(x: f64) -> f64 {
        x.abs()
    }
    pub fn fmod(x: f64, y: f64) -> f64 {
        x % y
    }
    pub fn tanh(x: f64) -> f64 {
        x.tanh()
    }
    pub fn sincos(x: f64) -> (f64, f64) {
        x.sin_cos()
    }
}

/// JS `Math.round`: halves go up.
pub fn js_round(x: f64) -> f64 {
    libm::floor(x + 0.5)
}

/// JS `Math.sign`.
pub fn js_sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// JS `x || 1`: 0 and NaN become 1.
pub fn or_one(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() { 1.0 } else { x }
}

/// JS `Math.min` (NaN wins).
pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b {
        a
    } else {
        b
    }
}

/// JS `Math.max` (NaN wins).
pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b {
        a
    } else {
        b
    }
}

/// JS `x | 0`.
pub fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let t = libm::trunc(x) % 4_294_967_296.0;
    let t = if t < 0.0 { t + 4_294_967_296.0 } else { t };
    (t as u64 as u32) as i32
}

pub fn clamp(x: f64, a: f64, b: f64) -> f64 {
    if x < a {
        a
    } else if x > b {
        b
    } else {
        x
    }
}

pub fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn inv_lerp(a: f64, b: f64, x: f64) -> f64 {
    (x - a) / (b - a)
}

pub fn remap(x: f64, a0: f64, a1: f64, b0: f64, b1: f64) -> f64 {
    lerp(b0, b1, inv_lerp(a0, a1, x))
}

pub fn fract(x: f64) -> f64 {
    x - libm::floor(x)
}

/// `((x % m) + m) % m`.
pub fn wrap(x: f64, m: f64) -> f64 {
    ((x % m) + m) % m
}

/// Progress 0..1 of t through [t0, t1]: 0 before, 1 after.
pub fn seg(t: f64, t0: f64, t1: f64) -> f64 {
    clamp((t - t0) / (t1 - t0), 0.0, 1.0)
}

/// Hermite smoothstep of x between e0 and e1.
pub fn smooth(e0: f64, e1: f64, x: f64) -> f64 {
    let t = seg(x, e0, e1);
    t * t * (3.0 - 2.0 * t)
}

/// 0 -> 1 -> 0 over [t0, t1] (a sine bump).
pub fn bump(t: f64, t0: f64, t1: f64) -> f64 {
    if t <= t0 || t >= t1 {
        0.0
    } else {
        libm::sin(PI * seg(t, t0, t1))
    }
}

pub fn pingpong(x: f64) -> f64 {
    1.0 - (wrap(x, 2.0) - 1.0).abs()
}

pub fn dist(x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    libm::hypot(x1 - x0, y1 - y0)
}

pub fn angle_to(x0: f64, y0: f64, x1: f64, y1: f64) -> f64 {
    libm::atan2(y1 - y0, x1 - x0)
}

/// A 32-bit hash of any number of integers (each taken as `n | 0`).
pub fn hash(ns: &[f64]) -> u32 {
    let mut h: u32 = 0x9e37_79b9;
    for &n in ns {
        h = (h ^ to_int32(n) as u32).wrapping_mul(0x85eb_ca6b);
        h ^= h >> 13;
        h = h.wrapping_mul(0xc2b2_ae35);
        h ^= h >> 16;
    }
    h
}

/// A deterministic number in [0, 1) from integers (seed, particle, which value...).
pub fn rand(ns: &[f64]) -> f64 {
    f64::from(hash(ns)) / 4_294_967_296.0
}

/// Seeded dice (mulberry32, as `raster::Dice`, in `f64`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    s: u32,
}

impl Rng {
    pub fn new(seed: u32) -> Self {
        Self {
            s: if seed == 0 { 1 } else { seed },
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> f64 {
        self.s = self.s.wrapping_add(0x6d2b_79f5);
        let mut t = self.s;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.next()
    }

    /// An integer in [a, b], both ends included.
    pub fn int(&mut self, a: f64, b: f64) -> f64 {
        libm::floor(self.range(a, b + 1.0))
    }

    pub fn chance(&mut self, p: f64) -> bool {
        self.next() < p
    }

    pub fn sign(&mut self) -> f64 {
        if self.next() < 0.5 { -1.0 } else { 1.0 }
    }

    /// One item of `list` (`None` only when it is empty).
    pub fn pick<'l, T>(&mut self, list: &'l [T]) -> Option<&'l T> {
        let i = libm::floor(self.next() * list.len() as f64) as usize;
        list.get(i)
    }

    /// Roughly normal, mean 0, sd 1.
    pub fn gauss(&mut self) -> f64 {
        (self.next() + self.next() + self.next() + self.next() - 2.0) * 1.732
    }
}

/// Dice for one particle: `rng(&[seed, i])` rolls the same numbers every frame.
pub fn rng(ns: &[f64]) -> Rng {
    Rng::new(hash(ns))
}

/// Smooth 1D value noise in [-1, 1].
pub fn noise(x: f64, seed: f64) -> f64 {
    let i = libm::floor(x);
    let f = x - i;
    let u = f * f * (3.0 - 2.0 * f);
    lerp(rand(&[i, seed]), rand(&[i + 1.0, seed]), u) * 2.0 - 1.0
}

/// Smooth 1D gradient noise in [-1, 1] that never rests.
pub fn perlin(x: f64, seed: f64) -> f64 {
    let i = libm::floor(x);
    let f = x - i;
    let slope = |at: f64| rand(&[at, seed]) * 2.0 - 1.0;
    let fade = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let v = lerp(slope(i) * f, slope(i + 1.0) * (f - 1.0), fade) * 2.0;
    clamp(v, -1.0, 1.0)
}

/// Smooth 2D value noise in [-1, 1].
pub fn noise2(x: f64, y: f64, seed: f64) -> f64 {
    let (ix, iy) = (libm::floor(x), libm::floor(y));
    let (fx, fy) = (x - ix, y - iy);
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let a = rand(&[ix, iy, seed]);
    let b = rand(&[ix + 1.0, iy, seed]);
    let c = rand(&[ix, iy + 1.0, seed]);
    let d = rand(&[ix + 1.0, iy + 1.0, seed]);
    lerp(lerp(a, b, ux), lerp(c, d, ux), uy) * 2.0 - 1.0
}

/// Fractal 2D noise in about [-1, 1].
pub fn fbm(mut x: f64, mut y: f64, seed: f64, octaves: u32) -> f64 {
    let (mut s, mut a, mut n) = (0.0, 0.5, 0.0);
    for o in 0..octaves {
        s += noise2(x, y, seed + f64::from(o) * 101.0) * a;
        n += a;
        x *= 2.03;
        y *= 2.03;
        a *= 0.5;
    }
    s / n
}

/// 2D value noise repeating every `px` by `py` cells (0: no repeat).
pub fn tile_noise(x: f64, y: f64, px: f64, py: f64, seed: f64) -> f64 {
    let (ix, iy) = (libm::floor(x), libm::floor(y));
    let (fx, fy) = (x - ix, y - iy);
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uy = fy * fy * (3.0 - 2.0 * fy);
    let cell = |i: f64, period: f64| if period != 0.0 { wrap(i, period) } else { i };
    let (x0, x1) = (cell(ix, px), cell(ix + 1.0, px));
    let (y0, y1) = (cell(iy, py), cell(iy + 1.0, py));
    let a = rand(&[x0, y0, seed]);
    let b = rand(&[x1, y0, seed]);
    let c = rand(&[x0, y1, seed]);
    let d = rand(&[x1, y1, seed]);
    lerp(lerp(a, b, ux), lerp(c, d, ux), uy) * 2.0 - 1.0
}

/// Fractal [`tile_noise`].
pub fn tile_fbm(x: f64, y: f64, px: f64, py: f64, seed: f64, octaves: u32) -> f64 {
    let (mut s, mut a, mut n, mut k) = (0.0, 0.5, 0.0, 1.0);
    for o in 0..octaves {
        s += tile_noise(x * k, y * k, px * k, py * k, seed + f64::from(o) * 101.0) * a;
        n += a;
        a *= 0.5;
        k *= 2.0;
    }
    s / n
}

/// Noise that loops: the same at phase 0 and phase 1.
pub fn loop_noise(phase: f64, radius: f64, seed: f64) -> f64 {
    let a = phase * TAU;
    noise2(
        libm::cos(a) * radius + 50.0,
        libm::sin(a) * radius + 50.0,
        seed,
    )
}

/// Easing curves: 0..1 in, 0..1 out (back and elastic overshoot).
pub mod ease {
    use super::{PI, TAU, libm};

    pub const BACK_S: f64 = 1.70158;

    pub fn linear(t: f64) -> f64 {
        t
    }
    pub fn in_quad(t: f64) -> f64 {
        t * t
    }
    pub fn out_quad(t: f64) -> f64 {
        1.0 - (1.0 - t) * (1.0 - t)
    }
    pub fn in_out_quad(t: f64) -> f64 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - libm::pow(-2.0 * t + 2.0, 2.0) / 2.0
        }
    }
    pub fn in_cubic(t: f64) -> f64 {
        t * t * t
    }
    pub fn out_cubic(t: f64) -> f64 {
        1.0 - libm::pow(1.0 - t, 3.0)
    }
    pub fn in_out_cubic(t: f64) -> f64 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - libm::pow(-2.0 * t + 2.0, 3.0) / 2.0
        }
    }
    pub fn in_quart(t: f64) -> f64 {
        t * t * t * t
    }
    pub fn out_quart(t: f64) -> f64 {
        1.0 - libm::pow(1.0 - t, 4.0)
    }
    pub fn in_expo(t: f64) -> f64 {
        if t == 0.0 {
            0.0
        } else {
            libm::pow(2.0, 10.0 * t - 10.0)
        }
    }
    pub fn out_expo(t: f64) -> f64 {
        if t == 1.0 {
            1.0
        } else {
            1.0 - libm::pow(2.0, -10.0 * t)
        }
    }
    pub fn in_sine(t: f64) -> f64 {
        1.0 - libm::cos((t * PI) / 2.0)
    }
    pub fn out_sine(t: f64) -> f64 {
        libm::sin((t * PI) / 2.0)
    }
    pub fn in_out_sine(t: f64) -> f64 {
        -(libm::cos(PI * t) - 1.0) / 2.0
    }
    pub fn in_back(t: f64, s: f64) -> f64 {
        (s + 1.0) * t * t * t - s * t * t
    }
    pub fn out_back(t: f64, s: f64) -> f64 {
        1.0 + (s + 1.0) * libm::pow(t - 1.0, 3.0) + s * libm::pow(t - 1.0, 2.0)
    }
    pub fn out_elastic(t: f64) -> f64 {
        if t == 0.0 || t == 1.0 {
            t
        } else {
            libm::pow(2.0, -10.0 * t) * libm::sin((t * 10.0 - 0.75) * (TAU / 3.0)) + 1.0
        }
    }
    pub fn out_bounce(t: f64) -> f64 {
        let (n, d) = (7.5625, 2.75);
        if t < 1.0 / d {
            n * t * t
        } else if t < 2.0 / d {
            let t = t - 1.5 / d;
            n * t * t + 0.75
        } else if t < 2.5 / d {
            let t = t - 2.25 / d;
            n * t * t + 0.9375
        } else {
            let t = t - 2.625 / d;
            n * t * t + 0.984375
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// colours

/// An RGBA8 colour (straight alpha). `Rgba::NONE` (all zero) means "nothing": the painter draws nothing for it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rgba(pub u8, pub u8, pub u8, pub u8);

impl Rgba {
    pub const NONE: Rgba = Rgba(0, 0, 0, 0);

    /// `0xrrggbb`, opaque.
    pub const fn hex24(rgb: u32) -> Rgba {
        Rgba((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8, 255)
    }

    /// From the raster's `[r, g, b, a]`.
    pub const fn from_raw(c: [u8; 4]) -> Rgba {
        Rgba(c[0], c[1], c[2], c[3])
    }

    pub const fn raw(self) -> [u8; 4] {
        [self.0, self.1, self.2, self.3]
    }
}

pub const WHITE: Rgba = Rgba(255, 255, 255, 255);
pub const BLACK: Rgba = Rgba(0, 0, 0, 255);

/// `#rrggbb` (magenta when malformed).
pub fn hex(s: &str) -> Rgba {
    let c = crate::raster::hex(s);
    Rgba(c[0], c[1], c[2], 255)
}

/// Each channel taken as `v | 0`, alpha 255.
pub fn rgb(r: f64, g: f64, b: f64) -> Rgba {
    Rgba(to_int32(r) as u8, to_int32(g) as u8, to_int32(b) as u8, 255)
}

/// A mix of two colours (t clamped), alpha 255.
pub fn mix(a: Rgba, b: Rgba, t: f64) -> Rgba {
    let t = clamp(t, 0.0, 1.0);
    let ch = |x: u8, y: u8| f64::from(x) + (f64::from(y) - f64::from(x)) * t;
    rgb(ch(a.0, b.0), ch(a.1, b.1), ch(a.2, b.2))
}

/// Brightness times k (capped), alpha 255.
pub fn scale(c: Rgba, k: f64) -> Rgba {
    let ch = |x: u8| js_min(255.0, f64::from(x) * k);
    rgb(ch(c.0), ch(c.1), ch(c.2))
}

/// Perceived brightness 0..1.
pub fn luma(c: Rgba) -> f64 {
    (0.299 * f64::from(c.0) + 0.587 * f64::from(c.1) + 0.114 * f64::from(c.2)) / 255.0
}

/// Step `i` of a list; `NONE` past its ends.
pub fn step_at(list: &[Rgba], i: f64) -> Rgba {
    if i >= 0.0 {
        list.get(i as usize).copied().unwrap_or(Rgba::NONE)
    } else {
        Rgba::NONE
    }
}

/// A ramp resampled to exactly n steps, keeping its ends.
pub fn fit(ramp: &[Rgba], n: usize) -> Vec<Rgba> {
    if ramp.len() == n {
        return ramp.to_vec();
    }
    let last = ramp.len() as f64 - 1.0;
    let span = (n as f64 - 1.0).max(1.0);
    (0..n)
        .map(|i| step_at(ramp, js_round((i as f64 * last) / span)))
        .collect()
}

/// [`fit`] into exactly `N` steps.
pub fn ramp<const N: usize>(r: &[Rgba]) -> [Rgba; N] {
    let v = fit(r, N);
    core::array::from_fn(|i| v.get(i).copied().unwrap_or(Rgba::NONE))
}

/// The ramp step for t in 0..1, unblended.
pub fn at(ramp: &[Rgba], t: f64) -> Rgba {
    let i = clamp(
        libm::floor(t * ramp.len() as f64),
        0.0,
        ramp.len() as f64 - 1.0,
    );
    step_at(ramp, i)
}

/// The colour t along a ramp, blended (for light).
pub fn blend(ramp: &[Rgba], t: f64) -> Rgba {
    let x = clamp(t, 0.0, 1.0) * (ramp.len() as f64 - 1.0);
    let i = js_min(ramp.len() as f64 - 2.0, libm::floor(x));
    mix(step_at(ramp, i), step_at(ramp, i + 1.0), x - i)
}

/// Hue-shifted ramps, dark to bright (the engine's `pal`).
pub mod pal {
    use super::Rgba;
    const fn h(v: u32) -> Rgba {
        Rgba::hex24(v)
    }
    pub const FIRE: [Rgba; 8] = [
        h(0x1f0b10),
        h(0x4a1118),
        h(0x8a1f17),
        h(0xc9381f),
        h(0xf2722d),
        h(0xffb443),
        h(0xffe27a),
        h(0xfff8d6),
    ];
}

// ---------------------------------------------------------------------------------------------------------------
// the painter

const BAYER: [f64; 16] = {
    const V: [u8; 16] = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];
    let mut out = [0.0; 16];
    let mut i = 0;
    while i < 16 {
        out[i] = (V[i] as f64 + 0.5) / 16.0;
        i += 1;
    }
    out
};

/// A value given once or as a function of one shape coordinate.
pub trait Param1<T> {
    fn at(&self, u: f64) -> T;
}

/// A value given once or as a function of two shape coordinates.
pub trait Param2<T> {
    fn at(&self, u: f64, v: f64) -> T;
}

impl Param1<Rgba> for Rgba {
    fn at(&self, _: f64) -> Rgba {
        *self
    }
}

impl Param1<f64> for f64 {
    fn at(&self, _: f64) -> f64 {
        *self
    }
}

impl<T, F: Fn(f64) -> T> Param1<T> for F {
    fn at(&self, u: f64) -> T {
        self(u)
    }
}

impl Param2<Rgba> for Rgba {
    fn at(&self, _: f64, _: f64) -> Rgba {
        *self
    }
}

impl Param2<f64> for f64 {
    fn at(&self, _: f64, _: f64) -> f64 {
        *self
    }
}

impl<T, F: Fn(f64, f64) -> T> Param2<T> for F {
    fn at(&self, u: f64, v: f64) -> T {
        self(u, v)
    }
}

/// What a [`Painter::shade`] function sets for its pixel: colour `c`, alpha `a`, light colour `l` (the pixel's colour
/// when NONE) and light amount `k`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadeOut {
    pub c: Rgba,
    pub a: f64,
    pub l: Rgba,
    pub k: f64,
}

/// Which picture an effect draws on: behind the fighters or over them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Layer {
    Back,
    Front,
}

/// Draws on one of the world's pictures in an effect's own space: its origin placed, mirrored and turned; colour as
/// hard pixels (alpha by ordered dither, or blended for soft volumes) and light gathered apart and added when the
/// painter is done (dropped), so light brightens whatever colour is under it.
pub struct Painter<'a> {
    canvas: &'a mut Canvas,
    light: Vec<[f32; 3]>,
    /// The box of pixels holding light: x0, y0, x1, y1 (inclusive).
    lit: [i32; 4],
    w: f64,
    h: f64,
    tx: f64,
    ty: f64,
    fx: f64,
    cs: f64,
    sn: f64,
    /// Opacity of everything drawn (a stopped effect fades out).
    pub fade: f64,
    /// Strength of the light it adds.
    pub light_k: f64,
    clip: Option<[f64; 4]>,
}

impl<'a> Painter<'a> {
    pub fn new(canvas: &'a mut Canvas) -> Self {
        let (w, h) = (f64::from(canvas.w), f64::from(canvas.h));
        Painter {
            canvas,
            light: Vec::new(),
            lit: [i32::MAX, i32::MAX, i32::MIN, i32::MIN],
            w,
            h,
            tx: 0.0,
            ty: 0.0,
            fx: 1.0,
            cs: 1.0,
            sn: 0.0,
            fade: 1.0,
            light_k: 1.0,
            clip: None,
        }
    }

    /// Put the effect's origin at world (x, y), mirrored if `flip`, turned by `angle` (radians).
    pub fn place(&mut self, x: f64, y: f64, flip: bool, angle: f64) -> &mut Self {
        self.tx = x;
        self.ty = y;
        self.fx = if flip { -1.0 } else { 1.0 };
        self.cs = libm::cos(angle);
        self.sn = libm::sin(angle);
        self
    }

    /// Where the origin is and how it is placed: (x, y, flip, angle).
    pub fn placement(&self) -> (f64, f64, bool, f64) {
        (self.tx, self.ty, self.fx < 0.0, libm::atan2(self.sn, self.cs))
    }

    /// Keep writes inside the local box [x0, x1) x [y0, y1) (`None` to stop clipping).
    pub fn clip_box(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> &mut Self {
        self.clip = Some([x0, y0, x1, y1]);
        self
    }

    pub fn clip_local(&mut self, w: f64, h: f64) -> &mut Self {
        self.clip_box(0.0, 0.0, w, h)
    }

    pub fn unclip(&mut self) -> &mut Self {
        self.clip = None;
        self
    }

    fn clipped(&self, sx: f64, sy: f64) -> bool {
        match self.clip {
            None => false,
            Some([x0, y0, x1, y1]) => {
                let (lx, ly) = (self.from_x(sx + 0.5, sy + 0.5), self.from_y(sx + 0.5, sy + 0.5));
                !(lx >= x0 && lx < x1 && ly >= y0 && ly < y1)
            }
        }
    }

    // ---- transforms: local (effect) space <-> world pixels
    pub fn to_x(&self, x: f64, y: f64) -> f64 {
        self.tx + x * self.fx * self.cs - y * self.sn
    }
    pub fn to_y(&self, x: f64, y: f64) -> f64 {
        self.ty + x * self.fx * self.sn + y * self.cs
    }
    pub fn from_x(&self, sx: f64, sy: f64) -> f64 {
        let (dx, dy) = (sx - self.tx, sy - self.ty);
        (dx * self.cs + dy * self.sn) * self.fx
    }
    pub fn from_y(&self, sx: f64, sy: f64) -> f64 {
        let (dx, dy) = (sx - self.tx, sy - self.ty);
        -dx * self.sn + dy * self.cs
    }

    /// The farthest a local point can be from the origin and still land on the picture.
    pub fn reach(&self) -> f64 {
        let far = |size: f64, at: f64| js_max(at.abs(), (size - at).abs());
        libm::hypot(far(self.w, self.tx), far(self.h, self.ty))
    }

    /// The world pixel of the local cell holding (x, y).
    pub fn cell_x(&self, x: f64, y: f64) -> f64 {
        libm::floor(self.to_x(libm::floor(x) + 0.5, libm::floor(y) + 0.5))
    }
    pub fn cell_y(&self, x: f64, y: f64) -> f64 {
        libm::floor(self.to_y(libm::floor(x) + 0.5, libm::floor(y) + 0.5))
    }

    fn index(&self, sx: f64, sy: f64) -> Option<usize> {
        if !(sx >= 0.0 && sy >= 0.0 && sx < self.w && sy < self.h) {
            return None;
        }
        Some((sy as usize) * (self.w as usize) + sx as usize)
    }

    /// Colour `c` on world pixel (sx, sy) at alpha `a` (by ordered dither: hard pixels).
    pub fn put(&mut self, sx: f64, sy: f64, c: Rgba, a: f64) {
        if c == Rgba::NONE || self.clipped(sx, sy) {
            return;
        }
        let a = a * self.fade;
        let cell = (((to_int32(sy) & 3) << 2) | (to_int32(sx) & 3)) as usize;
        if a < 1.0 && BAYER.get(cell).is_none_or(|&t| a <= t) {
            return;
        }
        if let Some(i) = self.index(sx, sy)
            && let Some(px) = self.canvas.px.get_mut(i)
        {
            over(px, c, f64::from(c.3) / 255.0);
        }
    }

    /// Light `c * k` added to world pixel (sx, sy).
    pub fn add(&mut self, sx: f64, sy: f64, c: Rgba, k: f64) {
        if self.clipped(sx, sy) {
            return;
        }
        let k = k * self.fade * self.light_k;
        if k <= 0.004 || k.is_nan() {
            return;
        }
        let Some(i) = self.index(sx, sy) else {
            return;
        };
        if self.light.is_empty() {
            self.light = vec![[0.0; 3]; (self.w as usize) * (self.h as usize)];
        }
        if let Some(l) = self.light.get_mut(i) {
            let k = k as f32;
            l[0] = (l[0] + f32::from(c.0) * k).min(255.0);
            l[1] = (l[1] + f32::from(c.1) * k).min(255.0);
            l[2] = (l[2] + f32::from(c.2) * k).min(255.0);
            let (x, y) = (sx as i32, sy as i32);
            self.lit = [
                self.lit[0].min(x),
                self.lit[1].min(y),
                self.lit[2].max(x),
                self.lit[3].max(y),
            ];
        }
    }

    /// Colour over what is there at alpha `a`, blended (soft smoke, vapour, mist).
    pub fn blend_px(&mut self, sx: f64, sy: f64, c: Rgba, a: f64) {
        if c == Rgba::NONE || self.clipped(sx, sy) {
            return;
        }
        let a = a * self.fade;
        if a <= 0.004 {
            return;
        }
        if let Some(i) = self.index(sx, sy)
            && let Some(px) = self.canvas.px.get_mut(i)
        {
            over(px, c, clamp(a, 0.0, 1.0));
        }
    }

    /// A translucent pixel at a local point.
    pub fn blend(&mut self, x: f64, y: f64, c: Rgba, a: f64) {
        let (sx, sy) = (self.cell_x(x, y), self.cell_y(x, y));
        self.blend_px(sx, sy, c, a);
    }

    /// One opaque pixel.
    pub fn px(&mut self, x: f64, y: f64, c: Rgba) {
        let (sx, sy) = (self.cell_x(x, y), self.cell_y(x, y));
        self.put(sx, sy, c, 1.0);
    }

    /// One pixel at alpha `a`, faded by ordered dither.
    pub fn dot(&mut self, x: f64, y: f64, c: Rgba, a: f64) {
        if a > 0.0 {
            let (sx, sy) = (self.cell_x(x, y), self.cell_y(x, y));
            self.put(sx, sy, c, a);
        }
    }

    /// Visit every world pixel whose centre falls in the local box [x0, x1] x [y0, y1], with that centre in local
    /// space: `f(painter, lx, ly, sx, sy)`.
    pub fn each<F>(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, mut f: F)
    where
        F: FnMut(&mut Self, f64, f64, f64, f64),
    {
        let (mut ax, mut bx, mut ay, mut by) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for k in 0..4 {
            let x = if k & 1 != 0 { x1 } else { x0 };
            let y = if k & 2 != 0 { y1 } else { y0 };
            let (sx, sy) = (self.to_x(x, y), self.to_y(x, y));
            ax = ax.min(sx);
            bx = bx.max(sx);
            ay = ay.min(sy);
            by = by.max(sy);
        }
        if !(ax.is_finite() && bx.is_finite() && ay.is_finite() && by.is_finite()) {
            return;
        }
        let sx0 = js_max(libm::floor(ax), 0.0);
        let sx1 = js_min(libm::ceil(bx), self.w - 1.0);
        let sy0 = js_max(libm::floor(ay), 0.0);
        let sy1 = js_min(libm::ceil(by), self.h - 1.0);
        let mut sy = sy0;
        while sy <= sy1 {
            let mut sx = sx0;
            while sx <= sx1 {
                let (cx, cy) = (sx + 0.5, sy + 0.5);
                let (lx, ly) = (self.from_x(cx, cy), self.from_y(cx, cy));
                f(self, lx, ly, sx, sy);
                sx += 1.0;
            }
            sy += 1.0;
        }
    }

    /// A pixel shader over a local box: `f(x, y, out)` returns a colour, or sets `out` (colour, alpha, light colour,
    /// light amount); `None` with `out.c` NONE leaves the pixel alone.
    pub fn shade<F>(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, mut f: F)
    where
        F: FnMut(f64, f64, &mut ShadeOut) -> Option<Rgba>,
    {
        self.each(x0, y0, x1, y1, |p, lx, ly, sx, sy| {
            let mut o = ShadeOut {
                c: Rgba::NONE,
                a: 1.0,
                l: Rgba::NONE,
                k: 0.0,
            };
            let c = match f(lx, ly, &mut o) {
                Some(c) if c != Rgba::NONE => c,
                _ => o.c,
            };
            if c != Rgba::NONE {
                p.put(sx, sy, c, o.a);
            }
            if o.k > 0.0 {
                p.add(sx, sy, if o.l != Rgba::NONE { o.l } else { c }, o.k);
            }
        });
    }

    /// A 1 px line; `c` and `a` may be functions of u (0 at the start, 1 at the end).
    pub fn line(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        c: impl Param1<Rgba>,
        a: impl Param1<f64>,
    ) {
        let (mut sx, mut sy) = (self.cell_x(x0, y0), self.cell_y(x0, y0));
        let (ex, ey) = (self.cell_x(x1, y1), self.cell_y(x1, y1));
        if !(sx.is_finite() && sy.is_finite() && ex.is_finite() && ey.is_finite()) {
            return;
        }
        let dx = (ex - sx).abs();
        let dy = -(ey - sy).abs();
        let stepx = if sx < ex { 1.0 } else { -1.0 };
        let stepy = if sy < ey { 1.0 } else { -1.0 };
        let n = js_max(dx, -dy);
        let n = if n == 0.0 { 1.0 } else { n };
        let mut err = dx + dy;
        let mut i = 0.0;
        for _ in 0..8192 {
            let u = i / n;
            i += 1.0;
            self.put(sx, sy, c.at(u), a.at(u));
            if sx == ex && sy == ey {
                break;
            }
            let e2 = 2.0 * err;
            if e2 >= dy {
                err += dy;
                sx += stepx;
            }
            if e2 <= dx {
                err += dx;
                sy += stepy;
            }
        }
    }

    /// An ellipse; `c` and `a` may be functions of (d: 0 centre to 1 rim, angle).
    pub fn ellipse(
        &mut self,
        cx: f64,
        cy: f64,
        rx: f64,
        ry: f64,
        c: impl Param2<Rgba>,
        a: impl Param2<f64>,
    ) {
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        self.shade(cx - rx, cy - ry, cx + rx, cy + ry, |x, y, o| {
            let (dx, dy) = ((x - cx) / rx, (y - cy) / ry);
            let d2 = dx * dx + dy * dy;
            if d2 > 1.0 {
                return None;
            }
            let (d, ang) = (libm::sqrt(d2), libm::atan2(dy, dx));
            o.c = c.at(d, ang);
            o.a = a.at(d, ang);
            None
        });
    }

    /// A translucent disc (smoke, vapour, dust); `c` and `a` may be functions of (d: 0 centre to 1 rim, angle).
    pub fn disc(&mut self, cx: f64, cy: f64, r: f64, c: impl Param2<Rgba>, a: impl Param2<f64>) {
        if r < 0.5 {
            return self.blend(cx, cy, c.at(0.0, 0.0), a.at(0.0, 0.0));
        }
        self.each(cx - r, cy - r, cx + r, cy + r, |p, x, y, sx, sy| {
            let (dx, dy) = (x - cx, y - cy);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                return;
            }
            let (d, ang) = (libm::sqrt(d2) / r, libm::atan2(dy, dx));
            p.blend_px(sx, sy, c.at(d, ang), a.at(d, ang));
        });
    }

    /// A filled disc of hard pixels; `c` and `a` may be functions of (d: 0 centre to 1 rim, angle).
    pub fn circle(&mut self, cx: f64, cy: f64, r: f64, c: impl Param2<Rgba>, a: impl Param2<f64>) {
        if r < 0.6 {
            return self.dot(cx, cy, c.at(0.0, 0.0), a.at(0.0, 0.0));
        }
        self.shade(cx - r, cy - r, cx + r, cy + r, |x, y, o| {
            let (dx, dy) = (x - cx, y - cy);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r {
                return None;
            }
            let (d, ang) = (libm::sqrt(d2) / r, libm::atan2(dy, dx));
            o.c = c.at(d, ang);
            o.a = a.at(d, ang);
            None
        });
    }

    /// A ring from radius r - w to r; `c` and `a` may be functions of (v: 0 inner to 1 outer, angle).
    pub fn ring(
        &mut self,
        cx: f64,
        cy: f64,
        r: f64,
        w: f64,
        c: impl Param2<Rgba>,
        a: impl Param2<f64>,
    ) {
        let r0 = js_max(0.0, r - w);
        self.shade(cx - r, cy - r, cx + r, cy + r, |x, y, o| {
            let (dx, dy) = (x - cx, y - cy);
            let d2 = dx * dx + dy * dy;
            if d2 > r * r || d2 < r0 * r0 {
                return None;
            }
            let v = if w > 0.0 {
                (libm::sqrt(d2) - r0) / or_one(r - r0)
            } else {
                1.0
            };
            let ang = libm::atan2(dy, dx);
            o.c = c.at(v, ang);
            o.a = a.at(v, ang);
            None
        });
    }

    /// Light on one pixel at a local point.
    pub fn light(&mut self, x: f64, y: f64, c: Rgba, k: f64) {
        let (sx, sy) = (self.cell_x(x, y), self.cell_y(x, y));
        self.add(sx, sy, c, k);
    }

    /// A filled rectangle.
    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, c: Rgba, a: f64) {
        if self.sn == 0.0 {
            let (x0, y0, x1, y1) = (js_round(x), js_round(y), js_round(x + w), js_round(y + h));
            let mut yy = y0;
            while yy < y1 {
                let mut xx = x0;
                while xx < x1 {
                    let (sx, sy) = (self.cell_x(xx, yy), self.cell_y(xx, yy));
                    self.put(sx, sy, c, a);
                    xx += 1.0;
                }
                yy += 1.0;
            }
            return;
        }
        self.shade(x, y, x + w, y + h, |lx, ly, o| {
            if lx < x || ly < y || lx >= x + w || ly >= y + h {
                return None;
            }
            o.c = c;
            o.a = a;
            None
        });
    }

    /// A filled polygon (even-odd); `c` and `a` may be functions of (x, y).
    pub fn poly(&mut self, pts: &[(f64, f64)], c: impl Param2<Rgba>, a: impl Param2<f64>) {
        let (mut x0, mut y0, mut x1, mut y1) = (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        );
        for &(x, y) in pts {
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
        }
        self.shade(x0, y0, x1, y1, |x, y, o| {
            let mut inside = false;
            let mut j = pts.len().wrapping_sub(1);
            for (i, &(xi, yi)) in pts.iter().enumerate() {
                if let Some(&(xj, yj)) = pts.get(j)
                    && (yi > y) != (yj > y)
                    && x < ((xj - xi) * (y - yi)) / (yj - yi) + xi
                {
                    inside = !inside;
                }
                j = i;
            }
            if !inside {
                return None;
            }
            o.c = c.at(x, y);
            o.a = a.at(x, y);
            None
        });
    }

    pub fn tri(
        &mut self,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        c: impl Param2<Rgba>,
        a: impl Param2<f64>,
    ) {
        self.poly(&[(x0, y0), (x1, y1), (x2, y2)], c, a);
    }

    /// A radial glow of light: k at the centre, falling as (1 - d)^2 to 0 at radius r.
    pub fn glow(&mut self, x: f64, y: f64, r: f64, c: Rgba, k: f64) {
        if r < 1.0 || k <= 0.004 {
            return;
        }
        let (cx, cy) = (self.to_x(x, y), self.to_y(x, y));
        let kt = k * self.fade * self.light_k;
        if kt.is_nan() || kt <= 0.004 {
            return;
        }
        let reach = js_min(1.0, 1.0 - 0.999_999 * libm::sqrt(0.004 / kt)) * r;
        let (x0, x1) = (
            js_max(libm::floor(cx - reach), 0.0),
            js_min(libm::ceil(cx + reach), self.w - 1.0),
        );
        let (y0, y1) = (
            js_max(libm::floor(cy - reach), 0.0),
            js_min(libm::ceil(cy + reach), self.h - 1.0),
        );
        let mut sy = y0;
        while sy <= y1 {
            let mut sx = x0;
            while sx <= x1 {
                let (dx, dy) = ((sx + 0.5 - cx) / r, (sy + 0.5 - cy) / r);
                let d = dx * dx + dy * dy;
                if d < 1.0 {
                    let f = 1.0 - libm::sqrt(d);
                    self.add(sx, sy, c, f * f * k);
                }
                sx += 1.0;
            }
            sy += 1.0;
        }
    }

    /// A glow around a segment (a soft beam of light).
    pub fn glow_line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64, r: f64, c: Rgba, k: f64) {
        if r < 1.0 || k <= 0.004 {
            return;
        }
        let (ax, ay, bx, by) = (
            self.to_x(x0, y0),
            self.to_y(x0, y0),
            self.to_x(x1, y1),
            self.to_y(x1, y1),
        );
        let (dx, dy) = (bx - ax, by - ay);
        let l2 = dx * dx + dy * dy;
        let l2 = if l2 == 0.0 || l2.is_nan() { 1e-9 } else { l2 };
        let (sx0, sx1) = (
            js_max(libm::floor(js_min(ax, bx) - r), 0.0),
            js_min(libm::ceil(js_max(ax, bx) + r), self.w - 1.0),
        );
        let (sy0, sy1) = (
            js_max(libm::floor(js_min(ay, by) - r), 0.0),
            js_min(libm::ceil(js_max(ay, by) + r), self.h - 1.0),
        );
        let mut sy = sy0;
        while sy <= sy1 {
            let mut sx = sx0;
            while sx <= sx1 {
                let (px, py) = (sx + 0.5, sy + 0.5);
                let u = clamp(((px - ax) * dx + (py - ay) * dy) / l2, 0.0, 1.0);
                let (ex, ey) = (ax + dx * u - px, ay + dy * u - py);
                let d = libm::sqrt(ex * ex + ey * ey) / r;
                if d < 1.0 {
                    let f = 1.0 - d;
                    self.add(sx, sy, c, f * f * k);
                }
                sx += 1.0;
            }
            sy += 1.0;
        }
    }

    /// A glow hugging a circle of radius `big_r` (a shockwave's light), falling off over width r.
    pub fn glow_ring(&mut self, x: f64, y: f64, big_r: f64, r: f64, c: Rgba, k: f64) {
        if r < 1.0 || k <= 0.004 {
            return;
        }
        let (cx, cy) = (self.to_x(x, y), self.to_y(x, y));
        let e = big_r + r;
        let mut sy = js_max(libm::floor(cy - e), 0.0);
        while sy <= js_min(libm::ceil(cy + e), self.h - 1.0) {
            let mut sx = js_max(libm::floor(cx - e), 0.0);
            while sx <= js_min(libm::ceil(cx + e), self.w - 1.0) {
                let d = (libm::hypot(sx + 0.5 - cx, sy + 0.5 - cy) - big_r).abs() / r;
                if d < 1.0 {
                    let f = 1.0 - d;
                    self.add(sx, sy, c, f * f * k);
                }
                sx += 1.0;
            }
            sy += 1.0;
        }
    }

    /// A pool of light on the floor: an ellipse of glow, k at its middle falling to 0 at its rim.
    pub fn floor_light(&mut self, x: f64, y: f64, rx: f64, ry: f64, c: Rgba, k: f64) {
        if k <= 0.004 || rx < 1.0 || ry < 0.5 {
            return;
        }
        self.shade(x - rx, y - ry, x + rx, y + ry, |px, py, out| {
            let (dx, dy) = ((px - x) / rx, (py - y) / ry);
            let d = dx * dx + dy * dy;
            if d >= 1.0 {
                return None;
            }
            let f = 1.0 - libm::sqrt(d);
            out.l = c;
            out.k = f * f * k;
            None
        });
    }
}

/// `c` over pixel `p` at alpha `a`.
fn over(p: &mut [u8; 4], c: Rgba, a: f64) {
    if a >= 1.0 && c.3 == 255 {
        *p = c.raw();
        return;
    }
    let a = a * f64::from(c.3) / 255.0;
    let da = f64::from(p[3]) / 255.0;
    let oa = a + da * (1.0 - a);
    if oa <= 0.0 {
        return;
    }
    let k = a / oa;
    let ch = |s: u8, d: u8| (f64::from(s) * k + f64::from(d) * (1.0 - k)) as u8;
    *p = [
        ch(c.0, p[0]),
        ch(c.1, p[1]),
        ch(c.2, p[2]),
        (oa * 255.0) as u8,
    ];
}

impl Drop for Painter<'_> {
    /// The light gathered, added on the picture: on an opaque pixel straight; over a clear one it becomes colour
    /// whose alpha is its brightest channel, so it reads as light added over what is under the picture.
    fn drop(&mut self) {
        if self.light.is_empty() {
            return;
        }
        let w = self.canvas.w as usize;
        let [x0, y0, x1, y1] = self.lit;
        for y in y0.max(0)..=y1 {
            for x in x0.max(0)..=x1 {
                let i = y as usize * w + x as usize;
                let (Some(l), Some(p)) = (self.light.get(i), self.canvas.px.get_mut(i)) else {
                    continue;
                };
                if l[0] <= 0.0 && l[1] <= 0.0 && l[2] <= 0.0 {
                    continue;
                }
                let a = f32::from(p[3]) / 255.0;
                if a >= 1.0 {
                    for c in 0..3 {
                        p[c] = (f32::from(p[c]) + l[c]).min(255.0) as u8;
                    }
                } else {
                    let pre = [
                        f32::from(p[0]) * a + l[0],
                        f32::from(p[1]) * a + l[1],
                        f32::from(p[2]) * a + l[2],
                    ];
                    let na = a.max(l[0].max(l[1]).max(l[2]) / 255.0).min(1.0);
                    if na > 0.0 {
                        for c in 0..3 {
                            p[c] = (pre[c] / na).min(255.0) as u8;
                        }
                        p[3] = (na * 255.0) as u8;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// closed-form particles

/// Where a particle is `t` seconds after leaving (x, y) at (vx, vy) px/s, under gravity `g` (px/s², down) and linear
/// drag (1/s).
pub fn fly(x: f64, y: f64, vx: f64, vy: f64, t: f64, g: f64, drag: f64) -> (f64, f64) {
    if drag <= 0.0 {
        return (x + vx * t + 0.0, y + vy * t + 0.5 * g * t * t);
    }
    let f = (1.0 - libm::exp(-drag * t)) / drag;
    let vt = g / drag;
    (x + vx * f, y + vt * t + (vy - vt) * f)
}

/// Particles that repeat every `period` seconds, for loops with no seam: particle `i` is born at its own phase and
/// lives `life(g)`; `f(age, life, r, i)` is called for every live copy with `r = rng(&[seed, i])`.
pub fn cycle(
    t: f64,
    period: f64,
    count: usize,
    seed: f64,
    life: impl Fn(&mut Rng) -> f64,
    mut f: impl FnMut(f64, f64, &mut Rng, usize),
) {
    for i in 0..count {
        let mut g = rng(&[seed, i as f64, 911.0]);
        let born = g.next() * period;
        let l = life(&mut g);
        let mut age = wrap(t - born, period);
        while age < l {
            f(age, l, &mut rng(&[seed, i as f64]), i);
            age += period;
        }
    }
}

/// A particle drawn by [`looped`]: how long each copy lives.
pub trait Life {
    fn life(&self) -> f64;
}

/// Seamless particles for a loop `d` seconds long: `make(r, i)` builds particle i from its own dice (the same every
/// frame), `draw(q, age, i)` is called for every live copy.
pub fn looped<Q: Life>(
    t: f64,
    d: f64,
    count: usize,
    seed: f64,
    mut make: impl FnMut(&mut Rng, usize) -> Q,
    mut draw: impl FnMut(&Q, f64, usize),
) {
    for i in 0..count {
        let mut r = rng(&[seed, i as f64]);
        let born = r.next() * d;
        let q = make(&mut r, i);
        let mut age = wrap(t - born, d);
        while age < q.life() {
            draw(&q, age, i);
            age += d;
        }
    }
}

/// Where a body is t seconds after leaving (x, y) at (vx, vy), under acceleration (ax, ay) and linear drag kx, ky:
/// [x, y, vx, vy].
pub fn drift(x: f64, y: f64, vx: f64, vy: f64, t: f64, ax: f64, ay: f64, kx: f64, ky: f64) -> [f64; 4] {
    let along = |x: f64, v: f64, a: f64, k: f64| {
        if k <= 0.0 {
            return (x + v * t + 0.5 * a * t * t, v + a * t);
        }
        let (e, vt) = (libm::exp(-k * t), a / k);
        (x + vt * t + ((v - vt) * (1.0 - e)) / k, vt + (v - vt) * e)
    };
    let (xx, xv) = along(x, vx, ax, kx);
    let (yy, yv) = along(y, vy, ay, ky);
    [xx, yy, xv, yv]
}

// ---------------------------------------------------------------------------------------------------------------
// recipes and their clock

/// Seconds a stopped effect takes to fade out.
pub const FADE: f64 = 0.3;

/// What an effect was played with, read in its own terms: colours, size, seed, the floor.
#[derive(Clone, Debug)]
pub struct Opts {
    /// Where it was played, world pixels.
    pub x: f64,
    pub y: f64,
    /// The floor in the effect's own space (the ground's row less `y`): what falls stops there.
    pub ground: f64,
    pub flip: bool,
    pub seed: f64,
    pub spawn: Spawn,
}

fn colors(c: &Option<Vec<[u8; 4]>>) -> Option<Vec<Rgba>> {
    c.as_ref()
        .filter(|v| !v.is_empty())
        .map(|v| v.iter().map(|&c| Rgba::from_raw(c)).collect())
}

impl Opts {
    pub fn new(at: &Spawn) -> Opts {
        Opts {
            x: f64::from(at.x),
            y: f64::from(at.y),
            ground: f64::from(at.ground - at.y),
            flip: at.flip,
            seed: f64::from(at.seed),
            spawn: at.clone(),
        }
    }

    /// The palette the rules gave (dark to bright), or `default`.
    pub fn palette_or(&self, default: &[Rgba]) -> Vec<Rgba> {
        colors(&self.spawn.palette).unwrap_or_else(|| default.to_vec())
    }

    pub fn accent_or(&self, default: &[Rgba]) -> Vec<Rgba> {
        colors(&self.spawn.accent).unwrap_or_else(|| default.to_vec())
    }

    pub fn stars_or(&self, default: &[Rgba]) -> Vec<Rgba> {
        colors(&self.spawn.stars).unwrap_or_else(|| default.to_vec())
    }

    pub fn fire_or(&self, default: &[Rgba]) -> Vec<Rgba> {
        colors(&self.spawn.fire).unwrap_or_else(|| default.to_vec())
    }

    pub fn glow_or(&self, default: Rgba) -> Rgba {
        self.spawn.glow.map(Rgba::from_raw).unwrap_or(default)
    }

    pub fn width_or(&self, default: f64) -> f64 {
        self.spawn.width.map(f64::from).unwrap_or(default)
    }

    pub fn height_or(&self, default: f64) -> f64 {
        self.spawn.height.map(f64::from).unwrap_or(default)
    }

    /// Whether the rules gave a palette.
    pub fn has_palette(&self) -> bool {
        colors(&self.spawn.palette).is_some()
    }
}

/// One playing of an effect, as its recipe draws it.
pub trait Show: Send + Sync {
    /// One sub-step of `dt` seconds (at most 1/120 s) for an effect that simulates; drawn-from-time ones skip it.
    fn update(&mut self, _dt: f64) {}
    /// The rules moved it: its origin is now world (x, y). (The painter is placed there; this is for what it leaves
    /// behind in the world, a trail.)
    fn moved(&mut self, _x: f64, _y: f64) {}
    /// Draw at `t` seconds into it (into the loop, for a loop) on `layer`, the painter placed at its origin.
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer);
}

/// An effect: its id, whether it loops, how long (one loop, or the whole of a one-shot), the frames a second it shows
/// (its clock steps by whole frames, as pixel art does), and how a playing of it starts.
#[derive(Clone, Copy)]
pub struct Recipe {
    pub id: &'static str,
    pub looping: bool,
    pub duration: f64,
    pub fps: u32,
    /// For a loop that opens once first: the seconds its opening takes (its frames from then on repeat).
    pub intro: Option<f64>,
    pub make: fn(&Opts) -> Box<dyn Show>,
}

impl Recipe {
    /// A recipe at 30 frames a second with no opening.
    pub const fn new(
        id: &'static str,
        looping: bool,
        duration: f64,
        make: fn(&Opts) -> Box<dyn Show>,
    ) -> Recipe {
        Recipe {
            id,
            looping,
            duration,
            fps: 30,
            intro: None,
            make,
        }
    }

    pub const fn fps(self, fps: u32) -> Recipe {
        Recipe { fps, ..self }
    }

    pub const fn intro(self, secs: f64) -> Recipe {
        Recipe {
            intro: Some(secs),
            ..self
        }
    }

    fn frame_dt(&self) -> f64 {
        1.0 / f64::from(self.fps.max(1))
    }

    /// Frames in one loop or one-shot.
    fn frames(&self) -> u64 {
        let last = if self.looping { 0.0 } else { 1.0 };
        js_max(1.0, js_round(self.duration * f64::from(self.fps)) + last) as u64
    }
}

impl Effect for Recipe {
    fn id(&self) -> &'static str {
        self.id
    }

    fn looping(&self) -> bool {
        self.looping
    }

    fn spawn(&self, at: &Spawn) -> Box<dyn Instance> {
        let opts = Opts::new(at);
        let show = (self.make)(&opts);
        Box::new(Play {
            recipe: *self,
            opts,
            show,
            t: 0.0,
            tick: 0,
            tick_t: 0.0,
            stop_at: None,
        })
    }
}

/// A playing effect on its clock: the time, the frame its show is at (sub-stepped as it simulates), the fade after
/// a stop.
struct Play {
    recipe: Recipe,
    opts: Opts,
    show: Box<dyn Show>,
    t: f64,
    tick: u64,
    tick_t: f64,
    stop_at: Option<f64>,
}

const FRAME_SLACK: f64 = 1e-6;

impl Play {
    fn clock_frame(&self) -> u64 {
        libm::floor(self.t * f64::from(self.recipe.fps) + FRAME_SLACK).max(0.0) as u64
    }

    /// The frame to show: a loop's comes round (after its opening), a one-shot's stops at its last.
    fn frame(&self) -> u64 {
        let (k, n) = (self.clock_frame(), self.recipe.frames());
        if self.recipe.looping {
            match self.recipe.intro {
                Some(intro) => {
                    let ki = (js_round(intro * f64::from(self.recipe.fps)) as u64).min(n - 1);
                    if k < n { k } else { ki + (k - ki) % (n - ki).max(1) }
                }
                None => k % n,
            }
        } else {
            k.min(n - 1)
        }
    }

    fn seek(&mut self, k: u64) {
        if k < self.tick {
            self.show = (self.recipe.make)(&self.opts);
            self.show.moved(self.opts.x, self.opts.y);
            self.tick = 0;
            self.tick_t = 0.0;
        }
        let dt = self.recipe.frame_dt();
        let n = js_max(1.0, libm::ceil(dt * 120.0 - 1e-9)) as u64;
        while self.tick < k {
            self.tick_t += dt;
            self.tick += 1;
            for _ in 0..n {
                self.show.update(dt / n as f64);
            }
        }
    }

    fn fade(&self) -> f64 {
        match self.stop_at {
            None => 1.0,
            Some(s) => {
                let shown = self.clock_frame() as f64 / f64::from(self.recipe.fps);
                clamp(1.0 - (shown - s) / FADE, 0.0, 1.0)
            }
        }
    }
}

impl Instance for Play {
    fn update(&mut self, dt: f32) -> bool {
        let dt = f64::from(dt);
        if dt.is_finite() && dt > 0.0 {
            self.t += dt.min(0.25);
            let k = self.frame();
            self.seek(k);
        }
        let stopped = self.stop_at.is_some_and(|s| self.t - s >= FADE);
        let ended = !self.recipe.looping && self.t >= self.recipe.duration;
        !(stopped || ended)
    }

    fn set_position(&mut self, x: f32, y: f32) {
        self.opts.x = f64::from(x);
        self.opts.y = f64::from(y);
        self.show.moved(self.opts.x, self.opts.y);
    }

    fn stop(&mut self) {
        if self.stop_at.is_none() {
            self.stop_at = Some(self.t);
        }
    }

    fn paint(&self, paint: &mut Paint) {
        let fade = self.fade();
        if fade <= 0.0 {
            return;
        }
        let d = self.recipe.duration;
        let t = if self.recipe.looping && d > 0.0 {
            self.tick_t % d
        } else {
            js_min(self.tick_t, d)
        };
        let (x, y, flip) = (self.opts.x, self.opts.y, self.opts.flip);
        for (layer, canvas) in [(Layer::Back, &mut *paint.back), (Layer::Front, &mut *paint.front)] {
            let mut p = Painter::new(canvas);
            p.place(x, y, flip, 0.0);
            p.fade = fade;
            self.show.draw(&mut p, t, layer);
        }
    }
}

impl Recipe {
    /// A playing of it at `at` (as `Effect::spawn`), for tools that hold the recipe itself.
    pub fn spawn_instance(&self, at: &Spawn) -> Box<dyn Instance> {
        Effect::spawn(self, at)
    }
}
