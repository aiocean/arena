//! The little float math the scene, the light and the feel draw with, written out so native and wasm builds give the
//! same bits (the workspace keeps `f64::sin` and its kin out for that reason, and this crate has no `libm` of its
//! own): floor, square root, sine, JS `Math.round`, and the engine's hash and Perlin noise (`pixfx-draw::math`).

pub use arena_duel::js::{js_max, js_min, js_round};

/// The largest whole number not above `x` (for the sizes a picture has; `x` well inside `i64`).
pub fn floor(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let t = x as i64 as f64;
    if t > x { t - 1.0 } else { t }
}

/// The square root of `x` (0 for anything not above 0): Newton's steps from a guess halving the exponent.
pub fn sqrt(x: f64) -> f64 {
    if x <= 0.0 || !x.is_finite() {
        return if x == f64::INFINITY { x } else { 0.0 };
    }
    let mut r = f64::from_bits((x.to_bits() >> 1) + (1023u64 << 51));
    for _ in 0..6 {
        r = 0.5 * (r + x / r);
    }
    r
}

/// `sqrt(a * a + b * b)`.
pub fn hypot(a: f64, b: f64) -> f64 {
    sqrt(a * a + b * b)
}

/// The sine of `x` (radians): brought to [-pi/2, pi/2], then its Taylor series.
pub fn sin(x: f64) -> f64 {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    if !x.is_finite() {
        return f64::NAN;
    }
    let mut a = x - TAU * floor(x / TAU + 0.5);
    if a > FRAC_PI_2 {
        a = PI - a;
    } else if a < -FRAC_PI_2 {
        a = -PI - a;
    }
    let a2 = a * a;
    let mut term = a;
    let mut sum = a;
    for n in 1..10 {
        let k = (2 * n) as f64;
        term *= -a2 / (k * (k + 1.0));
        sum += term;
    }
    sum
}

/// JS `x | 0`.
pub fn to_int32(x: f64) -> i32 {
    if !x.is_finite() {
        return 0;
    }
    let t = x.trunc();
    (t.rem_euclid(4_294_967_296.0) as u64 as u32) as i32
}

/// 32-bit hash of integers (each taken as JS `n | 0`): the engine's `hash`.
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

/// A number in [0, 1) from integers: the engine's `rand`.
pub fn rand(ns: &[f64]) -> f64 {
    f64::from(hash(ns)) / 4_294_967_296.0
}

/// Smooth 1D Perlin noise in [-1, 1], 0 at every whole `x`: the engine's `perlin`.
pub fn perlin(x: f64, seed: f64) -> f64 {
    let i = floor(x);
    let f = x - i;
    let slope = |at: f64| rand(&[at, seed]) * 2.0 - 1.0;
    let fade = f * f * f * (f * (f * 6.0 - 15.0) + 10.0);
    let (a, b) = (slope(i) * f, slope(i + 1.0) * (f - 1.0));
    ((a + (b - a) * fade) * 2.0).clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn the_written_out_math_matches_the_library_closely() {
        for i in -400..400 {
            let x = i as f64 * 0.173;
            assert!((sin(x) - x.sin()).abs() < 1e-12, "sin {x}");
            assert_eq!(floor(x), x.floor(), "floor {x}");
            let y = x.abs() * 37.0;
            assert!(
                (sqrt(y) - y.sqrt()).abs() <= 1e-12 * y.sqrt().max(1.0),
                "sqrt {y}"
            );
        }
        assert_eq!(to_int32(-3.7), -3);
        assert_eq!(to_int32(4_294_967_297.0), 1);
    }
}
