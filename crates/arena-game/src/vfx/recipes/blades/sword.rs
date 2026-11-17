//! The spirit sword (the engine's `arcane/sword`: its sprite, palette and the painter that turns it to any angle) and
//! the effects made of it: `arcane/sword-formation` (eight swords gather behind the origin, fan into a ring and turn
//! there until stopped), `arcane/sword-strike` (where a sword strikes: a shrinking star, sparks, chips) and
//! `arcane/sword-slash` (a big sword winds back and cuts down through the point in one arc, sweeping a crescent).
//!
//! The engine's formation also launches its swords as a volley when the game orders a strike; the duel never orders
//! one (it plays `arcane/sword-strike` itself when the spell ends), so only the gathering and the ring are here.

use crate::vfx::kit::sweep::{Brush, Cell, Look, Pose as SweepPose, Turn, sweep};
use crate::vfx::kit::{Layer, Opts, PI, Painter, Rgba, Show, TAU, clamp, libm, rand, smooth};

/// The sword pointing up, tip first, a row a pixel. The guard's middle (`PIVOT_ROW`, column 2) is where it turns.
const SPRITE: [&[u8; 5]; 18] = [
    b"..W..", b"..Wm.", b".bWm.", b".bWz.", b".bWz.", b".bWb.", b".bWb.", b".bWc.", b".bWc.",
    b".bWp.", b".bWp.", b"rhrhr", b"jhrhj", b"k.j.k", b"..d..", b"..e..", b"..d..", b".jhj.",
];
const PIVOT_ROW: f64 = 11.0;
/// From the pivot to the tip, and to the far side of the sprite.
const TIP: f64 = 11.0;
const REACH: f64 = 13.0;

const WHITE: Rgba = Rgba::hex24(0xf2fdfd);
const ICE: Rgba = Rgba::hex24(0xa4f7fd);
const AQUA: Rgba = Rgba::hex24(0x63eefd);
const CYAN: Rgba = Rgba::hex24(0x56cef8);
const SKY: Rgba = Rgba::hex24(0x56b6e2);
const STEEL: Rgba = Rgba::hex24(0x3d85b0);
const DUSK: Rgba = Rgba::hex24(0x396b8a);
const CREAM: Rgba = Rgba::hex24(0xf8f6c3);
const GOLD: Rgba = Rgba::hex24(0xeccf54);
const AMBER: Rgba = Rgba::hex24(0xd9ae31);
const BRONZE: Rgba = Rgba::hex24(0xb5922e);
const UMBER: Rgba = Rgba::hex24(0x8b7121);
const GRIP: Rgba = Rgba::hex24(0x2d5672);
/// The additive glow round blades, trails and flashes.
const GLOW: Rgba = Rgba::hex24(0x44b2c1);

/// Where a sword is and how it is drawn at a moment.
#[derive(Clone, Copy)]
struct Pose {
    x: f64,
    y: f64,
    angle: f64,
    /// How much of the blade is drawn, from the guard out (0 to 1), while it forms.
    grown: f64,
    /// Whiteness of a flash over it (0 to 1).
    hot: f64,
    /// Opacity: below 1 it crumbles from the tip, dimmed a step.
    alpha: f64,
}

/// A 4 by 4 ordered dither threshold for a pixel, 0 to 1.
fn bayer(x: f64, y: f64) -> f64 {
    const M: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
    let xi = (libm::floor(x) as i64).rem_euclid(4) as usize;
    let yi = (libm::floor(y) as i64).rem_euclid(4) as usize;
    let v = M.get(yi).and_then(|r| r.get(xi)).copied().unwrap_or(0);
    (f64::from(v) + 0.5) / 16.0
}

/// A colour a step darker, for a sword scattering.
fn dim(c: Rgba) -> Rgba {
    match c {
        WHITE => ICE,
        ICE => AQUA,
        AQUA | CYAN => SKY,
        SKY => STEEL,
        CREAM => GOLD,
        GOLD => AMBER,
        AMBER => BRONZE,
        other => other,
    }
}

fn colour_of(ch: u8) -> Option<Rgba> {
    Some(match ch {
        b'W' => WHITE,
        b'm' => ICE,
        b'z' => AQUA,
        b'b' => CYAN,
        b'c' => SKY,
        b'p' => STEEL,
        b'd' => DUSK,
        b'r' => CREAM,
        b'h' => GOLD,
        b'j' => AMBER,
        b'i' => BRONZE,
        b'k' => UMBER,
        b'e' => GRIP,
        _ => return None,
    })
}

/// One sword at a pose, `scale` times its size: the sprite turned by nearest pixel, only the rows out to how far
/// it has grown, whitened by its flash, crumbling as it fades; a soft glow along the blade.
fn sword_at(p: &mut Painter, s: &Pose, scale: f64) {
    let (ca, sa) = (libm::cos(s.angle), libm::sin(s.angle));
    let reach = REACH * scale;
    let (x0, y0) = (libm::floor(s.x - reach), libm::floor(s.y - reach));
    let span = (reach * 2.0) as i32 + 1;
    let shown = libm::floor(PIVOT_ROW - TIP * s.grown);
    let crumbled = libm::floor((1.0 - s.alpha) * SPRITE.len() as f64);
    for j in 0..span {
        for i in 0..span {
            let (px, py) = (x0 + f64::from(i), y0 + f64::from(j));
            let (dx, dy) = (px + 0.5 - s.x, py + 0.5 - s.y);
            let along = (dx * ca + dy * sa) / scale;
            let across = (-dx * sa + dy * ca) / scale;
            let row = libm::floor(PIVOT_ROW - along + 0.5);
            let col = libm::floor(across + 2.5);
            if !(0.0..SPRITE.len() as f64).contains(&row)
                || !(0.0..5.0).contains(&col)
                || row < shown
            {
                continue;
            }
            let Some(mut c) = SPRITE
                .get(row as usize)
                .and_then(|r| r.get(col as usize))
                .and_then(|&ch| colour_of(ch))
            else {
                continue;
            };
            if s.hot > 0.0 && row < PIVOT_ROW && bayer(px, py) < s.hot {
                c = WHITE;
            }
            if s.alpha < 1.0 {
                if row < crumbled {
                    continue;
                }
                c = dim(c);
            }
            p.dot(px, py, c, 1.0);
        }
    }
    let k = s.alpha * (0.35 + 0.5 * s.hot);
    let reach = TIP * scale * s.grown;
    p.glow_line(
        s.x,
        s.y,
        s.x + ca * reach,
        s.y + sa * reach,
        3.0 * scale,
        GLOW,
        k,
    );
}

// ---------------------------------------------------------------------------------------------------------------
// arcane/sword-formation

const SWORDS: usize = 8;
/// The ring: its centre behind the shoulders, the pivots' radius.
const CX: f64 = -8.0;
const CY: f64 = -30.0;
const RING: f64 = 14.0;
/// The gathered swords, upright side by side: their pivots' height and spacing.
const STACK_Y: f64 = -24.0;
const STACK_GAP: f64 = 5.0;
/// Seconds: sword `k` of the stack appears at `GATHER + slot * GATHER_GAP`, forming over `FORM`; they fan out
/// together from `FAN` to `FAN_END`, then the ring turns an eighth of a turn every `PERIOD`.
const FPS: f64 = 30.0;
const GATHER: f64 = 0.05;
const GATHER_GAP: f64 = 0.07;
const FORM: f64 = 0.15;
const FAN: f64 = 0.7;
pub const FAN_END: f64 = 28.0 / FPS;
const PERIOD: f64 = 36.0 / FPS;
/// The opening and one loop of the ring.
pub const FORMATION_SECS: f64 = (28.0 + 36.0) / FPS;
/// The order the stack fills in (middle out), as stack positions left to right.
const ORDER: [usize; SWORDS] = [3, 4, 2, 5, 1, 6, 0, 7];
/// The ring angle each stack position fans out to, so no two cross.
const RING_ANGLE: [f64; SWORDS] = [
    PI,
    0.75 * PI,
    -0.75 * PI,
    -0.5 * PI,
    0.5 * PI,
    -0.25 * PI,
    0.25 * PI,
    0.0,
];

/// The ring's turn at `t`: none until it has fanned out, then an eighth of a turn every `PERIOD`.
fn turn(t: f64) -> f64 {
    (t - FAN_END).max(0.0) / PERIOD * TAU / SWORDS as f64
}

/// The shortest turn from angle `a` to angle `b`.
fn turn_to(a: f64, b: f64) -> f64 {
    let d = libm::fmod(b - a + PI, TAU);
    let d = if d < 0.0 { d + TAU } else { d };
    d - PI
}

/// Sword `k` in the ring at `t`: its pivot and the way it points.
fn ring(k: usize, t: f64) -> (f64, f64, f64) {
    let a = RING_ANGLE.get(k).copied().unwrap_or(0.0) + turn(t);
    (CX + RING * libm::cos(a), CY + RING * libm::sin(a), a)
}

fn slot(k: usize) -> f64 {
    ORDER.iter().position(|&o| o == k).unwrap_or(0) as f64
}

/// Sword `k` (a stack position) at `t`, or none before it appears.
fn formation_pose(k: usize, t: f64) -> Option<Pose> {
    let born = GATHER + slot(k) * GATHER_GAP;
    if t < born {
        return None;
    }
    let up = -0.5 * PI;
    let stack = (CX + (k as f64 - 3.5) * STACK_GAP, STACK_Y);
    let mut pose = Pose {
        x: stack.0,
        y: stack.1,
        angle: up,
        grown: clamp((t - born) / FORM, 0.0, 1.0),
        hot: clamp(1.0 - (t - born - FORM * 0.5) / 0.2, 0.0, 1.0),
        alpha: 1.0,
    };
    if t < FAN {
        // gathered, bobbing a pixel
        pose.y += libm::floor(libm::sin(t * 9.0 + k as f64) * 0.8 + 0.5);
        return Some(pose);
    }
    let (rx, ry, ra) = ring(k, t);
    let f = smooth(FAN, FAN_END, t);
    pose.x = stack.0 + (rx - stack.0) * f;
    pose.y = stack.1 + (ry - stack.1) * f;
    pose.angle = up + turn_to(up, ra) * f;
    if t >= FAN_END {
        // a glint as each sword passes the top of the ring
        let d = libm::fabs(turn_to(ra, up));
        pose.hot = clamp(1.0 - d / 0.35, 0.0, 1.0) * 0.8;
    }
    Some(pose)
}

/// The speck of light sword `k` forms from.
fn speck(p: &mut Painter, k: usize, t: f64) {
    let since = t - GATHER - slot(k) * GATHER_GAP;
    if !(0.0..0.2).contains(&since) {
        return;
    }
    let x = CX + (k as f64 - 3.5) * STACK_GAP;
    let y = STACK_Y - 6.0;
    let s = 1.0 - since / 0.2;
    p.dot(x, y, WHITE, 1.0);
    if s > 0.4 {
        for (dx, dy) in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
            p.dot(x + dx, y + dy, ICE, 1.0);
        }
    }
    p.glow(x, y, 6.0, GLOW, 0.8 * s);
}

struct Formation;

pub fn formation(_: &Opts) -> Box<dyn Show> {
    Box::new(Formation)
}

impl Show for Formation {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if layer != Layer::Back {
            return;
        }
        for k in 0..SWORDS {
            if let Some(pose) = formation_pose(k, t) {
                speck(p, k, t);
                sword_at(p, &pose, 1.0);
            }
        }
        // a faint bloom inside the ring
        let k = smooth(FAN, FAN_END, t);
        if k > 0.0 {
            p.glow(CX, CY, 30.0, GLOW, 0.25 * k);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// arcane/sword-strike

/// How long the strike's star and sparks last.
const FLASH: f64 = 0.3;
const STRIKE_SECS: f64 = 0.45;

struct Strike {
    seed: f64,
}

pub fn strike(o: &Opts) -> Box<dyn Show> {
    Box::new(Strike { seed: o.seed })
}

impl Show for Strike {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if layer == Layer::Back {
            return;
        }
        if t < FLASH {
            let k = 1.0 - t / FLASH;
            let ray = libm::floor(7.0 * k);
            p.dot(0.0, 0.0, WHITE, 1.0);
            for r in 1..=ray as i32 {
                let r = f64::from(r);
                let c = if r < ray * 0.5 { WHITE } else { ICE };
                for (dx, dy) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    p.dot(dx * r, dy * r, c, 1.0);
                }
                if r <= ray * 0.35 {
                    for (dx, dy) in [(1.0, 1.0), (-1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)] {
                        p.dot(dx * r, dy * r, AQUA, 1.0);
                    }
                }
            }
            p.glow(0.0, 0.0, 10.0, WHITE, 0.9 * k);
            p.glow(0.0, 0.0, 18.0, GLOW, 0.6 * k);
            for i in 0..6 {
                let fi = f64::from(i);
                // thrown back the way the sword came, fanning up and down
                let ang = PI + (rand(&[fi, self.seed, 29.0]) - 0.5) * 2.4;
                let speed = 50.0 + 50.0 * rand(&[fi, self.seed, 31.0]);
                let x = libm::cos(ang) * speed * t;
                let y = libm::sin(ang) * speed * t + 60.0 * t * t;
                let c = if i % 2 == 0 { WHITE } else { CYAN };
                p.dot(libm::floor(x), libm::floor(y), c, k);
                p.light(x, y, GLOW, 0.6 * k);
            }
        }
        // chips of the blade, falling and fading
        for i in 0..3 {
            let fi = f64::from(i);
            let vx = (rand(&[fi, self.seed, 41.0]) - 0.7) * 50.0;
            let vy = -30.0 - 30.0 * rand(&[fi, self.seed, 43.0]);
            let x = libm::floor(vx * t);
            let y = libm::floor(vy * t + 140.0 * t * t);
            let k = 1.0 - t / STRIKE_SECS;
            p.dot(x, y, SKY, k);
            p.dot(x + 1.0, y, ICE, k);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// arcane/sword-slash

/// The sword's size (the engine's `size` option), and where the cut crosses, from the feet (`hitX`, `hitY`).
const SIZE: f64 = 2.0;
const HIT_AT: (f64, f64) = (0.0, -22.0);
/// The timeline, seconds: it grows out of its guard, winds back, cuts, follows through, crumbles.
const APPEAR_END: f64 = 0.15;
const WINDUP_END: f64 = 0.30;
const SWING_END: f64 = 0.42;
const FOLLOW_END: f64 = 0.60;
pub const SLASH_SECS: f64 = 0.85;
/// The blade's angles: at rest up behind, wound back, through the point straight ahead, ending low and over.
const REST: f64 = -2.0;
const BACK: f64 = -2.35;
const HIT: f64 = 0.0;
const END: f64 = 0.75;
const OVER: f64 = 0.95;
/// How far from the guard the cut crosses the hit point, as a share of the blade.
const BITE: f64 = 0.7;
/// The share of the blade, from the guard, that leaves no crescent.
const CRESCENT: f64 = 0.4;
/// How long the crescent lasts behind the blade, seconds.
const SMEAR: f64 = 0.14;
/// The crescent's colours, from the blade's edge out to where it cools.
const RAMP: [Rgba; 7] = [WHITE, ICE, AQUA, CYAN, SKY, STEEL, DUSK];

/// The blade's angle at `t`.
fn angle(t: f64) -> f64 {
    if t < APPEAR_END {
        REST
    } else if t < WINDUP_END {
        REST + (BACK - REST) * smooth(APPEAR_END, WINDUP_END, t)
    } else if t < SWING_END {
        let u = (t - WINDUP_END) / (SWING_END - WINDUP_END);
        BACK + (END - BACK) * u * u
    } else if t < FOLLOW_END {
        let u = (t - SWING_END) / (FOLLOW_END - SWING_END);
        END + (OVER - END) * (1.0 - (1.0 - u) * (1.0 - u))
    } else {
        OVER
    }
}

/// When the blade points through the hit point.
fn hit_time() -> f64 {
    let u = libm::sqrt((HIT - BACK) / (END - BACK));
    WINDUP_END + u * (SWING_END - WINDUP_END)
}

/// The blade as a brush: its outer part to the tip, each pixel its row's edge colour, brighter toward the tip.
fn blade(scale: f64) -> Brush {
    let reach = TIP * scale;
    let mut cells = Vec::new();
    let mut d = CRESCENT * reach;
    while d <= reach {
        let row = libm::floor(PIVOT_ROW - d / scale + 0.5);
        let pick = |col: usize| {
            SPRITE
                .get(row as usize)
                .and_then(|r| r.get(col))
                .and_then(|&ch| colour_of(ch))
        };
        if let Some(color) = pick(3).or_else(|| pick(2)) {
            cells.push(Cell {
                across: d,
                color,
                light: 0.3 + 0.7 * d / reach,
            });
        }
        d += 1.0;
    }
    Brush {
        cells,
        turn: Turn::Angle(-PI / 2.0),
    }
}

struct Slash {
    scale: f64,
    hit: (f64, f64),
    pivot: (f64, f64),
    brush: Brush,
    seed: f64,
}

pub fn slash(o: &Opts) -> Box<dyn Show> {
    let bite = BITE * TIP * SIZE;
    Box::new(Slash {
        scale: SIZE,
        hit: HIT_AT,
        pivot: (
            HIT_AT.0 - bite * libm::cos(HIT),
            HIT_AT.1 - bite * libm::sin(HIT),
        ),
        brush: blade(SIZE),
        seed: o.seed,
    })
}

impl Slash {
    /// The crescent: the blade's poses from the start of the cut up to `t`, newest first.
    fn crescent(&self, p: &mut Painter, t: f64) {
        if t < WINDUP_END {
            return;
        }
        let step = 1.0 / 240.0;
        let mut poses = Vec::new();
        let mut age = 0.0;
        while age <= SMEAR && t - age >= WINDUP_END {
            poses.push(SweepPose {
                x: self.pivot.0,
                y: self.pivot.1,
                angle: angle(t - age),
                age,
            });
            age += step;
        }
        let look = Look {
            secs: SMEAR,
            fade: 4.0,
            taper: 0.3,
            wave: 0.0,
            phase: 0.0,
            breakup: 0.45,
            shift: libm::floor(t * 30.0),
            light: 0.6,
            skip: 0.0,
        };
        sweep(p, &poses, &self.brush, &RAMP, &look);
    }

    /// Where the cut lands: a white flash and sparks thrown on along the way the blade goes.
    fn flash(&self, p: &mut Painter, t: f64) {
        let since = t - hit_time();
        if !(0.0..0.2).contains(&since) {
            return;
        }
        let k = 1.0 - since / 0.2;
        let (hx, hy) = self.hit;
        p.glow(hx, hy, 6.0 * self.scale, WHITE, 0.9 * k);
        p.glow(hx, hy, 12.0 * self.scale, GLOW, 0.5 * k);
        let way = HIT + PI / 2.0;
        for i in 0..5 {
            let fi = f64::from(i);
            let a = way + (rand(&[fi, self.seed, 53.0]) - 0.5) * 1.2;
            let speed = 60.0 + 60.0 * rand(&[fi, self.seed, 59.0]);
            let x = hx + libm::cos(a) * speed * since;
            let y = hy + libm::sin(a) * speed * since + 80.0 * since * since;
            p.dot(
                libm::floor(x),
                libm::floor(y),
                if i % 2 == 0 { WHITE } else { ICE },
                k,
            );
            p.light(x, y, GLOW, 0.5 * k);
        }
    }
}

impl Show for Slash {
    fn draw(&self, p: &mut Painter, t: f64, layer: Layer) {
        if layer == Layer::Back {
            return;
        }
        self.crescent(p, t);
        // a glint as it forms, white-hot through the cut
        let glint = clamp(1.0 - libm::fabs(t - 0.12) / 0.06, 0.0, 1.0);
        let cut = if (WINDUP_END..SWING_END).contains(&t) {
            0.35
        } else {
            0.0
        };
        sword_at(
            p,
            &Pose {
                x: self.pivot.0,
                y: self.pivot.1,
                angle: angle(t),
                grown: smooth(0.0, APPEAR_END * 0.8, t),
                hot: glint.max(cut),
                alpha: 1.0 - smooth(FOLLOW_END, SCATTER, t),
            },
            self.scale,
        );
        self.flash(p, t);
    }
}

/// When the slash's sword has crumbled away.
const SCATTER: f64 = SLASH_SECS;
