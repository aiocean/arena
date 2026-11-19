//! Summon effects: a brief poof per unit kind, replacing the generic `spawn/poof`.
//!
//! - `spawn/trap-poof`: subtle indigo arcane flash (a trap placed is meant to be missed).
//! - `spawn/mine-poof`: electric blue crackling burst (a mine lands and crackles).
//! - `spawn/totem-poof`: golden mystical rings (a spirit is summoned to stand and cast).
//! - `spawn/minion-poof`: aggressive red-orange burst (a fighter coalesces from raw qi).

use crate::kit::{Layer, Opts, Painter, Rgba, Show, clamp, ease, smooth};

// ── Trap ────────────────────────────────────────────────────────────────────

pub const TRAP_DURATION: f64 = 0.6;

const TRAP_DIM: Rgba = Rgba::hex24(0x3d2d6e);
const TRAP_MID: Rgba = Rgba::hex24(0x7858c8);
const TRAP_LIT: Rgba = Rgba::hex24(0xc8b8f8);

pub fn trap_make(_o: &Opts) -> Box<dyn Show> {
    Box::new(TrapPoof)
}

struct TrapPoof;

impl Show for TrapPoof {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if t >= TRAP_DURATION {
            return;
        }
        if layer == Layer::Back {
            // small floor glow: swells in quickly then fades out
            let a = smooth(0.0, 0.08, t) * smooth(TRAP_DURATION, 0.1, t);
            if a > 0.0 {
                p.glow(0.0, 0.0, 10.0, TRAP_MID, 0.3 * a);
            }
            return;
        }
        // brief disc flash at the centre
        if t < 0.1 {
            let k = 1.0 - t / 0.1;
            p.circle(0.0, 0.0, 6.0 * (1.0 - k * 0.4), TRAP_LIT, 0.9 * k);
            p.circle(0.0, 0.0, 3.5, TRAP_LIT, k);
        }
        // one thin ring expanding outward and fading
        {
            let u = clamp(t / 0.5, 0.0, 1.0);
            let r = ease::out_expo(u) * 24.0;
            let a = (1.0 - u) * smooth(0.0, 0.04, t);
            if a > 0.0 {
                p.ring(0.0, 0.0, r, 1.2, TRAP_MID, a);
                p.glow_ring(0.0, 0.0, r, 3.0, TRAP_DIM, 0.18 * a);
            }
        }
        // a second smaller ring, slightly delayed
        {
            let u = clamp((t - 0.06) / 0.4, 0.0, 1.0);
            if u > 0.0 {
                let r = ease::out_expo(u) * 15.0;
                let a = (1.0 - u) * 0.6;
                p.ring(0.0, 0.0, r, 0.9, TRAP_LIT, a);
            }
        }
    }
}

// ── Mine ────────────────────────────────────────────────────────────────────

pub const MINE_DURATION: f64 = 0.9;

const MINE_DIM: Rgba = Rgba::hex24(0x1a3a7c);
const MINE_MID: Rgba = Rgba::hex24(0x50a8ff);
const MINE_LIT: Rgba = Rgba::hex24(0xffffff);

pub fn mine_make(_o: &Opts) -> Box<dyn Show> {
    Box::new(MinePoof)
}

struct MinePoof;

impl Show for MinePoof {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if t >= MINE_DURATION {
            return;
        }
        if layer == Layer::Back {
            // floor glow fading over 0.6s
            let a = smooth(MINE_DURATION, 0.3, t) * smooth(0.0, 0.03, t);
            if a > 0.0 {
                p.glow(0.0, 0.0, 22.0, MINE_MID, 0.4 * a);
            }
            return;
        }
        // initial flash disc
        if t < 0.08 {
            let k = 1.0 - t / 0.08;
            p.circle(0.0, 0.0, 12.0 * (1.3 - k * 0.3), MINE_LIT, 0.95 * k);
            p.circle(0.0, 0.0, 7.0, MINE_MID, k);
        }
        // ring 1: fast, large, blue
        {
            let u = clamp(t / 0.5, 0.0, 1.0);
            let r = ease::out_cubic(u) * 40.0;
            let a = (1.0 - u).powi(2) * smooth(0.0, 0.04, t);
            if a > 0.0 {
                p.ring(0.0, 0.0, r, 2.0, MINE_MID, a);
                p.glow_ring(0.0, 0.0, r, 5.0, MINE_DIM, 0.25 * a);
            }
        }
        // ring 2: delayed, inner, brighter
        {
            let u = clamp((t - 0.1) / 0.35, 0.0, 1.0);
            if u > 0.0 {
                let r = ease::out_expo(u) * 28.0;
                let a = (1.0 - u).powi(2);
                p.ring(0.0, 0.0, r, 1.5, MINE_LIT, a * 0.8);
                p.glow_ring(0.0, 0.0, r, 4.0, MINE_MID, 0.2 * a);
            }
        }
        // electric crackle: 4 short radial segments as bright dots at cardinal angles
        if t < 0.15 {
            let k = 1.0 - t / 0.15;
            let reach = 8.0 + 14.0 * (1.0 - k);
            for &(cx, cy) in &[(reach, 0.0), (-reach, 0.0), (0.0, -reach)] {
                let dot_a = k * 0.9;
                p.circle(cx, cy, 1.5, MINE_LIT, dot_a);
                p.glow(cx, cy, 4.0, MINE_MID, 0.3 * dot_a);
            }
        }
    }
}

// ── Totem ───────────────────────────────────────────────────────────────────

pub const TOTEM_DURATION: f64 = 1.2;

const TOTEM_DIM: Rgba = Rgba::hex24(0x7a4408);
const TOTEM_MID: Rgba = Rgba::hex24(0xf0b840);
const TOTEM_LIT: Rgba = Rgba::hex24(0xffffff);

pub fn totem_make(_o: &Opts) -> Box<dyn Show> {
    Box::new(TotemPoof)
}

struct TotemPoof;

impl Show for TotemPoof {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if t >= TOTEM_DURATION {
            return;
        }
        if layer == Layer::Back {
            // warm golden floor glow, fades over 1.0s
            let a = smooth(TOTEM_DURATION, 0.2, t) * smooth(0.0, 0.05, t);
            if a > 0.0 {
                p.glow(0.0, 0.0, 30.0, TOTEM_MID, 0.35 * a);
                p.glow(0.0, 0.0, 14.0, TOTEM_LIT, 0.2 * a);
            }
            return;
        }
        // central golden flash
        if t < 0.1 {
            let k = 1.0 - t / 0.1;
            p.circle(0.0, 0.0, 10.0, TOTEM_LIT, 0.9 * k);
            p.circle(0.0, 0.0, 6.5, TOTEM_LIT, k);
        }
        // ring 1: innermost, quick
        {
            let u = clamp(t / 0.45, 0.0, 1.0);
            let r = ease::out_cubic(u) * 20.0;
            let a = (1.0 - u).powi(2) * smooth(0.0, 0.04, t);
            if a > 0.0 {
                p.ring(0.0, 0.0, r, 1.5, TOTEM_LIT, a);
                p.glow_ring(0.0, 0.0, r, 5.0, TOTEM_MID, 0.3 * a);
            }
        }
        // ring 2: mid, golden
        {
            let u = clamp((t - 0.05) / 0.6, 0.0, 1.0);
            if u > 0.0 {
                let r = ease::out_expo(u) * 30.0;
                let a = (1.0 - u).powi(2) * 0.9;
                p.ring(0.0, 0.0, r, 2.0, TOTEM_MID, a);
                p.glow_ring(0.0, 0.0, r, 6.0, TOTEM_DIM, 0.25 * a);
            }
        }
        // ring 3: outermost, slow, dim
        {
            let u = clamp((t - 0.1) / 0.9, 0.0, 1.0);
            if u > 0.0 {
                let r = ease::out_expo(u) * 40.0;
                let a = (1.0 - u).powi(2) * 0.6;
                p.ring(0.0, 0.0, r, 1.5, TOTEM_MID, a * 0.7);
            }
        }
        // upward rising glow (spirit ascending)
        {
            let rise = t * 30.0; // moves upward
            let a = smooth(TOTEM_DURATION, 0.4, t) * smooth(0.0, 0.1, t);
            if a > 0.0 {
                p.glow(0.0, -rise, 8.0, TOTEM_LIT, 0.25 * a);
                p.glow(0.0, -rise * 0.6, 14.0, TOTEM_MID, 0.15 * a);
            }
        }
    }
}

// ── Minion ──────────────────────────────────────────────────────────────────

pub const MINION_DURATION: f64 = 1.0;

const MINION_DIM: Rgba = Rgba::hex24(0x701010);
const MINION_MID: Rgba = Rgba::hex24(0xf05020);
const MINION_LIT: Rgba = Rgba::hex24(0xffffff);

pub fn minion_make(_o: &Opts) -> Box<dyn Show> {
    Box::new(MinionPoof)
}

struct MinionPoof;

impl Show for MinionPoof {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if t >= MINION_DURATION {
            return;
        }
        if layer == Layer::Back {
            // aggressive red floor glow, fades over 0.6s
            let a = smooth(MINION_DURATION, 0.4, t) * smooth(0.0, 0.03, t);
            if a > 0.0 {
                p.glow(0.0, 0.0, 28.0, MINION_MID, 0.4 * a);
                p.glow(0.0, 0.0, 12.0, MINION_LIT, 0.2 * a);
            }
            return;
        }
        // aggressive flash disc
        if t < 0.08 {
            let k = 1.0 - t / 0.08;
            p.circle(0.0, 0.0, 14.0 * (1.2 - k * 0.2), MINION_LIT, 0.95 * k);
            p.circle(0.0, 0.0, 9.0, MINION_MID, k);
            p.circle(0.0, 0.0, 5.0, MINION_LIT, k);
        }
        // one large fast expanding ring
        {
            let u = clamp(t / 0.6, 0.0, 1.0);
            let r = ease::out_cubic(u) * 48.0;
            let a = (1.0 - u).powi(2) * smooth(0.0, 0.03, t);
            if a > 0.0 {
                p.ring(0.0, 0.0, r, 2.5, MINION_MID, a);
                p.glow_ring(0.0, 0.0, r, 7.0, MINION_DIM, 0.3 * a);
            }
        }
        // inner ring fades out sooner
        {
            let u = clamp(t / 0.35, 0.0, 1.0);
            if u < 1.0 {
                let r = ease::out_expo(u) * 22.0;
                let a = (1.0 - u).powi(2) * 0.8;
                p.ring(0.0, 0.0, r, 1.8, MINION_LIT, a);
            }
        }
        // burst petals: 6 small discs flung outward at angles (like qi shrapnel)
        if t < 0.4 {
            let k = t / 0.4;
            let reach = ease::out_cubic(k) * 20.0;
            let shrink = (1.0 - k) * 2.5;
            let a = (1.0 - k).powi(2) * 0.85;
            for i in 0..6usize {
                let angle = (i as f64 / 6.0) * core::f64::consts::TAU;
                let cx = crate::kit::libm::cos(angle) * reach;
                let cy = crate::kit::libm::sin(angle) * reach;
                p.circle(cx, cy, shrink, MINION_MID, a);
                p.glow(cx, cy, shrink * 2.5, MINION_DIM, 0.2 * a);
            }
        }
    }
}
