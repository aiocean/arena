//! JS number semantics, where they change a fight's results: `Math.round` (half up toward +inf), `Math.sign`,
//! `Math.min`/`Math.max` (NaN wins) and `x || 1`. Taken from the pixfx engine's `pixfx-draw::math` at rev ad1612b.

/// JS `Math.round`: nearest integer, halves toward +inf (`-0.5` gives `-0`).
pub fn js_round(x: f64) -> f64 {
    let r = libm::floor(x);
    let up = if x - r >= 0.5 { r + 1.0 } else { r };
    if up == 0.0 && x < 0.0 { -0.0 } else { up }
}

/// JS `Math.sign`: -1, 0 or 1; NaN and a signed zero come back as they are.
pub fn js_sign(x: f64) -> f64 {
    if x > 0.0 {
        1.0
    } else if x < 0.0 {
        -1.0
    } else {
        x
    }
}

/// JS `x || 1`: 0 and NaN are falsy, so they give 1.
pub fn or_one(x: f64) -> f64 {
    if x == 0.0 || x.is_nan() { 1.0 } else { x }
}

/// JS `Math.min(a, b)`: NaN if either is NaN, and -0 is less than +0.
pub fn js_min(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a < b || (a == b && a.is_sign_negative()) {
        a
    } else {
        b
    }
}

/// JS `Math.max(a, b)`: NaN if either is NaN, and +0 is greater than -0.
pub fn js_max(a: f64, b: f64) -> f64 {
    if a.is_nan() || b.is_nan() {
        f64::NAN
    } else if a > b || (a == b && a.is_sign_positive()) {
        a
    } else {
        b
    }
}
