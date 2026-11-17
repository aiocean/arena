//! `water/splash`, after the engine's: water bursts up where something struck it, a column shooting up from the middle
//! and a crown opening round it, breaking into round droplets that rain back down, run into a puddle and soak away.
//! Painted as one body with the kit's blob in the water ramp (a lit top, a dark outline, the body, deeper where it is
//! thick) and white foam on fast lone droplets. Drawn over the fighters.
//!
//! The engine moves the water with an SPH liquid (pressure, viscosity, surface tension). Here the drops are plain
//! ballistic particles: they are thrown in clumps (neighbours share most of their throw, so the sheets break into
//! droplets rather than a fine spray), lose their speed against the air a little as the liquid's own drag would, and
//! land on the floor where they slide, settle into a puddle and soak in. The column is thrown a little lower than the
//! engine's first moment, which its liquid (held in its 96 px tall box) never let rise the whole way.
//!
//! Also the water ramp and the droplet sprite the water jet shares (the engine's `water/lib.rs`, after Sprite Fusion's
//! open-source blood, https://www.spritefusion.com/pixel-art-vfx/blood, MIT, Copyright (c) 2026 Hugo Duprez).

use crate::vfx::kit::blob::{Blob, BlobLook};
use crate::vfx::kit::{Layer, Opts, Painter, Rgba, Show, WHITE, js_min, libm, or_one, rand};

/// Water, deep to white: deep colour only in shadow, tinted middles, white crests.
pub const WATER: [Rgba; 8] = [
    Rgba::hex24(0x0a1630),
    Rgba::hex24(0x123063),
    Rgba::hex24(0x1b5596),
    Rgba::hex24(0x2a7fc4),
    Rgba::hex24(0x45a9e3),
    Rgba::hex24(0x86d3f5),
    Rgba::hex24(0xcdf1ff),
    Rgba::hex24(0xffffff),
];

/// A droplet at (x, y) moving at (vx, vy) px/s: s 1 is a pixel, 2 a shaded 2x2, 3 a round bead with a highlight; a
/// tail trails back along its motion, longer the faster it goes. `c`: an 8-step water ramp.
#[allow(clippy::too_many_arguments)]
pub fn bead(p: &mut Painter, x: f64, y: f64, vx: f64, vy: f64, s: f64, c: &[Rgba; 8], a: f64) {
    let sp = or_one(libm::hypot(vx, vy));
    let (ux, uy) = (vx / sp, vy / sp);
    let tail = js_min(5.0, sp * 0.011);
    if tail >= 1.0 {
        let (c3, c4, c5) = (c[3], c[4], c[5]);
        p.line(
            x - ux * (tail + 1.0),
            y - uy * (tail + 1.0),
            x - ux * 1.2,
            y - uy * 1.2,
            move |u: f64| {
                if u < 0.45 {
                    c3
                } else if s > 1.0 {
                    c5
                } else {
                    c4
                }
            },
            a,
        );
    }
    if s >= 3.0 {
        p.circle(x, y, 1.75, c[4], a);
        p.circle(x - 0.25, y - 0.25, 1.15, c[6], a);
        p.dot(x - 0.7, y - 0.7, WHITE, a);
    } else if s >= 2.0 {
        let (bx, by) = (libm::floor(x), libm::floor(y));
        p.dot(bx, by, WHITE, a);
        p.dot(bx + 1.0, by, c[6], a);
        p.dot(bx, by + 1.0, c[5], a);
        p.dot(bx + 1.0, by + 1.0, c[4], a);
    } else {
        p.dot(x, y, if sp > 160.0 { c[6] } else { c[5] }, a);
    }
}

/// Radius of the water thrown up (the engine's `size` default).
const R: f64 = 9.0;
/// The size of a clump of drops thrown as one droplet.
const CLUMP: f64 = 3.0;
/// The drops' spacing in it.
const SPACING: f64 = 1.2;
/// The crown: speed up the middle column, up at the rim, and out sideways at the rim (px/s).
const COLUMN: f64 = 150.0;
const RIM_UP: f64 = 70.0;
const RIM_OUT: f64 = 90.0;
const GRAVITY: f64 = 320.0;
/// The air's drag on a drop (1/s), and the floor's on a landed one.
const DRAG: f64 = 0.6;
const FLOOR_DRAG: f64 = 7.0;
/// The highest the water goes (the engine's liquid box) and how far out.
const CEILING: f64 = -76.0;
const SIDE: f64 = 80.0;
/// When the puddle starts soaking in, and how long the last of it takes.
const SOAK_AT: f64 = 0.9;
const SOAK_FOR: f64 = 0.95;
/// A drop this fast (px/s), flying, carries foam (when it is a lone one).
const FOAM_SPEED: f64 = 110.0;
/// How big a drop is painted.
const SPLAT: f64 = 1.9;

#[derive(Clone, Copy)]
struct Drop {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    /// When it soaks in.
    gone: f64,
    /// How far above the floor it settles (the puddle has some depth).
    lift: f64,
    /// Whether it is spray (a lone fleck that carries foam).
    lone: bool,
}

pub fn make(o: &Opts) -> Box<dyn Show> {
    let s = o.seed;
    let mut drops = Vec::new();
    // the water at the moment of the blow: a disc sitting on the ground, thrown up fastest in the middle (a column)
    // and out and up at its sides (the crown's walls); drops in the same clump share most of their throw
    let n = libm::ceil(R / SPACING) as i32;
    let mut i = 0.0;
    for gy in -n..=n {
        for gx in -n..=n {
            let (dx, dy) = (f64::from(gx) * SPACING, f64::from(gy) * SPACING);
            if dx * dx + dy * dy > R * R {
                continue;
            }
            let (x, y) = (dx, dy - R);
            // drops in the same clump fly as one droplet (the throw of its middle); a few lone flecks keep their own
            // throw: the spray
            let lone = rand(&[s, i, 5.0]) < 0.1;
            let clump = [libm::floor(x / CLUMP), libm::floor(y / CLUMP)];
            let (mx, my) = if lone {
                (x, y)
            } else {
                ((clump[0] + 0.5) * CLUMP, (clump[1] + 0.5) * CLUMP)
            };
            let dice = |k: f64| {
                if lone {
                    rand(&[s, i, k])
                } else {
                    rand(&[s, clump[0], clump[1], k])
                }
            };
            let a = (mx / R).clamp(-1.0, 1.0);
            let k = 1.0 - a.abs();
            let jit = 0.8 + 0.4 * dice(1.0);
            let vx = a * RIM_OUT * jit + (dice(4.0) - 0.5) * 16.0;
            let vy = -(RIM_UP + COLUMN * k * k) * (0.85 + 0.3 * dice(2.0)) * (0.6 + 0.4 * (-my / (2.0 * R)));
            drops.push(Drop {
                x,
                y,
                vx,
                vy,
                gone: SOAK_AT + SOAK_FOR * rand(&[s, i, 3.0]),
                lift: rand(&[s, i, 6.0]) * 1.6,
                lone,
            });
            i += 1.0;
        }
    }
    let ramp = o.palette_or(&WATER);
    let at = |i: usize| {
        ramp.get(i.min(ramp.len().saturating_sub(1)))
            .copied()
            .unwrap_or(WATER[3])
    };
    Box::new(State {
        drops,
        t: 0.0,
        look: BlobLook {
            edge: at(1),
            body: at(3),
            deep: at(2),
            rim: at(6),
            iso: 0.5,
            deep_at: 2.4,
            shine: 0.25,
        },
        foam: at(7),
    })
}

struct State {
    drops: Vec<Drop>,
    t: f64,
    look: BlobLook,
    foam: Rgba,
}

impl Show for State {
    fn update(&mut self, dt: f64) {
        if dt.is_nan() || dt <= 0.0 {
            return;
        }
        self.t += dt;
        let t = self.t;
        let air = libm::exp(-DRAG * dt);
        let floor = libm::exp(-FLOOR_DRAG * dt);
        for d in &mut self.drops {
            d.vy += GRAVITY * dt;
            d.vx *= air;
            d.x += d.vx * dt;
            d.y += d.vy * dt;
            if d.y >= -d.lift {
                // on the floor: a little bounce, then it slides to a stop in the puddle
                d.y = -d.lift;
                d.vy = if d.vy > 40.0 { -d.vy * 0.1 } else { 0.0 };
                d.vx *= floor;
            }
            if d.y < CEILING {
                d.y = CEILING;
                d.vy = d.vy.max(0.0);
            }
            if d.x.abs() > SIDE {
                d.x = d.x.clamp(-SIDE, SIDE);
                d.vx = 0.0;
            }
        }
        self.drops.retain(|d| t < d.gone);
    }

    fn draw(&self, p: &mut Painter, _t: f64, layer: Layer) {
        if layer != Layer::Front {
            return;
        }
        let Some(first) = self.drops.first() else {
            return;
        };
        let (mut x0, mut y0, mut x1, mut y1) = (first.x, first.y, first.x, first.y);
        for d in &self.drops {
            x0 = x0.min(d.x);
            y0 = y0.min(d.y);
            x1 = x1.max(d.x);
            y1 = y1.max(d.y);
        }
        let mut blob = Blob::new(x0 - SPLAT, y0 - SPLAT, x1 + SPLAT + 1.0, y1 + SPLAT + 1.0);
        for d in &self.drops {
            blob.splat(d.x, d.y, SPLAT, 1.0);
        }
        blob.paint(p, &self.look);
        for d in &self.drops {
            if d.lone && d.y < -1.0 && libm::hypot(d.vx, d.vy) > FOAM_SPEED {
                p.px(d.x, d.y, self.foam);
            }
        }
    }
}
