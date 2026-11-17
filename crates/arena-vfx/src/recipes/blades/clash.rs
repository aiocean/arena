//! Blows meeting: `impacts/parry` (the duel's clash: two edges flash in an X, a white four-point star rings out, a
//! ring breaks into arcs and a fan of gold sparks sprays up and rains down) and `impacts/hit` (a blow landing: a
//! white-hot star stretched along the blow, a cut flashing across, sparks thrown the way it pushes, a breaking ring).
//! With the engine's hit star and broken ring (impacts `lib`).

use crate::kit::sim::{Kind, Lights, Particles, ParticlesOpts, Spawn};
use crate::kit::{
    Layer, Opts, PI, Painter, Rgba, Rng, Show, TAU, WHITE, clamp, ease, js_max, js_min, libm,
    smooth,
};

/// One spike of a hit star: angle, length and root half-width.
#[derive(Clone, Copy)]
struct Spike {
    a: f64,
    len: f64,
    w: f64,
}

/// One arc of a broken ring: centre angle, gap (0..1), how fast it flies out.
#[derive(Clone, Copy)]
struct Seg {
    a: f64,
    gap: f64,
    dr: f64,
}

/// A hit star: tapered spikes in three passes (a rim, a body paler near the root, a hot core) round a hot heart.
/// `sc` stretches the spikes, `th` thins them, `cols` are [rim, body, inner, core].
fn star(p: &mut Painter, y: f64, list: &[Spike], sc: f64, th: f64, cols: [Rgba; 4], heart: f64) {
    let [rim, body, inner, core] = cols;
    for pass in 0..3 {
        for s in list {
            let (c, sn, l) = (libm::cos(s.a), libm::sin(s.a), s.len * sc);
            let mut d = 0.0;
            while d <= l {
                if pass == 2 && d > l * 0.55 {
                    break;
                }
                let hw0 = s.w * th * (1.0 - d / l) * js_min(1.0, 0.5 + d / 3.0);
                let hw = match pass {
                    0 => hw0 + 0.8,
                    1 => hw0,
                    _ => hw0 * 0.45,
                };
                let col = match pass {
                    0 => rim,
                    1 if d < l * 0.35 => inner,
                    1 => body,
                    _ => core,
                };
                let mut o = -hw;
                while o <= hw {
                    p.dot(c * d - sn * o, y + sn * d + c * o, col, 1.0);
                    o += 0.5;
                }
                d += 0.5;
            }
        }
    }
    let r = heart * sc * libm::sqrt(th);
    p.circle(0.0, y, r + 0.8, inner, 1.0);
    p.circle(0.0, y, r, core, 1.0);
}

/// A ring round (0, `y`) that breaks into arcs: `brk` 0 a whole ring `th` thick; as it grows each arc shrinks,
/// tapers into a crescent and drifts out at its own speed. `c` the outer edge's colour, `c2` the inner's; nothing
/// is drawn at or below `max_y`.
#[expect(clippy::too_many_arguments, reason = "the engine's signature")]
fn broken_ring(
    p: &mut Painter,
    y: f64,
    r: f64,
    th: f64,
    segs: &[Seg],
    brk: f64,
    c: Rgba,
    c2: Rgba,
    k: f64,
    max_y: f64,
) {
    if r <= 0.0 || k <= 0.0 {
        return;
    }
    let full = TAU / segs.len() as f64;
    for s in segs {
        let span = full * (1.0 - s.gap * brk) + 0.02;
        let rr = r * (1.0 + s.dr * brk);
        let m = js_max(4.0, libm::ceil(rr * span * 1.8)) as usize;
        for j in 0..=m {
            let u = j as f64 / m as f64;
            let ang = s.a + (u - 0.5) * span;
            let w = js_max(
                0.5,
                th * (1.0 - brk * libm::pow(libm::fabs(2.0 * u - 1.0), 1.6)),
            );
            let (ca, sa) = (libm::cos(ang), libm::sin(ang));
            let mut o = -w / 2.0;
            while o <= w / 2.0 + 0.01 {
                if y + sa * (rr + o) < max_y {
                    let col = if o > -0.26 { c } else { c2 };
                    p.dot(ca * (rr + o), y + sa * (rr + o), col, k);
                }
                o += 0.5;
            }
        }
    }
}

/// A spark, a mote or a star of the sim.
fn spawn(kind: Kind, x: f64, y: f64, vx: f64, vy: f64, life: f64) -> Spawn {
    Spawn::new(kind, x, y, vx, vy, life)
}

/// What a parry and a hit keep: the palette's seven steps, the dice, the particles and lights, the star and ring.
struct Blow {
    c: [Rgba; 7],
    /// The clash's height over the origin (a parry), 0 for a hit.
    y: f64,
    /// A hit's weight (0 a jab, 1 a throw) and its size from it.
    w: f64,
    s: f64,
    r: Rng,
    ps: Particles,
    lights: Lights,
    sp: Vec<Spike>,
    segs: Vec<Seg>,
    edges: [f64; 2],
    t: f64,
    parry: bool,
}

/// Metal struck, dark to white: the engine's parry and hit palette.
const METAL: [Rgba; 7] = [
    Rgba::hex24(0x2a1a08),
    Rgba::hex24(0x6a4410),
    Rgba::hex24(0xb07a1a),
    Rgba::hex24(0xe8b030),
    Rgba::hex24(0xffe066),
    Rgba::hex24(0xfff6c4),
    Rgba::hex24(0xffffff),
];

fn palette(o: &Opts) -> [Rgba; 7] {
    let pal = o.palette_or(&METAL);
    core::array::from_fn(|i| pal.get(i).copied().unwrap_or(WHITE))
}

// ---------------------------------------------------------------------------------------------------------------
// impacts/parry

pub fn parry(o: &Opts) -> Box<dyn Show> {
    let c = palette(o);
    let y = -o.height_or(30.0);
    let mut r = Rng::new(o.seed as u32);
    let mut ps = Particles::new(ParticlesOpts {
        ground: 0.0,
        sparks: vec![c[4], c[4], c[3], c[2]],
        ..ParticlesOpts::default()
    });
    let mut lights = Lights::new();
    // a hard four-point star: long arms on the axes, short ones between
    let mut sp = Vec::with_capacity(8);
    for i in 0..8 {
        let a = f64::from(i) * PI / 4.0 + r.range(-0.04, 0.04);
        let len = if i % 2 == 1 {
            r.range(6.0, 8.0)
        } else if i % 4 == 0 {
            r.range(19.0, 22.0)
        } else {
            r.range(14.0, 17.0)
        };
        let w = if i % 2 == 1 { 1.2 } else { 2.2 };
        sp.push(Spike { a, len, w });
    }
    let mut segs = Vec::with_capacity(6);
    for i in 0..6 {
        let a = (f64::from(i) / 6.0) * TAU + r.range(-0.1, 0.1);
        let gap = r.range(0.55, 0.8);
        let dr = r.range(0.1, 0.35);
        segs.push(Seg { a, gap, dr });
    }
    // the two edges that met: lines crossing in an X
    let e0 = -0.62 + r.range(-0.08, 0.08);
    let e1 = PI + 0.62 + r.range(-0.08, 0.08);
    lights.add(0.0, y, 60.0, c[4], 0.65, 0.4);
    lights.add(0.0, y, 22.0, WHITE, 0.8, 0.08);
    // the fan: sparks thrown up and out to both sides, arcing over and raining down to bounce on the floor
    for i in 0..24 {
        let s = if i % 2 == 1 { 1.0 } else { -1.0 };
        let a = -PI / 2.0 + s * r.range(0.3, 1.3);
        let v = r.range(130.0, 290.0);
        let x = s * r.range(1.0, 3.0);
        let life = r.range(0.45, 0.85);
        let delay = if i > 17 { r.range(0.02, 0.07) } else { 0.0 };
        let sp = Spawn {
            delay,
            ..spawn(Kind::Spark, x, y, libm::cos(a) * v, libm::sin(a) * v, life)
        };
        ps.spawn(&mut r, sp);
    }
    for _ in 0..8 {
        let a = r.range(0.0, TAU);
        let v = r.range(40.0, 110.0);
        let life = r.range(0.3, 0.55);
        let col = r.pick(&[c[5], c[6]]).copied();
        let delay = r.range(0.0, 0.04);
        let sp = Spawn {
            col,
            size: 0.6,
            delay,
            ..spawn(
                Kind::Mote,
                0.0,
                y,
                libm::cos(a) * v,
                libm::sin(a) * v - 20.0,
                life,
            )
        };
        ps.spawn(&mut r, sp);
    }
    for _ in 0..3 {
        let a = r.range(0.0, TAU);
        let d = r.range(12.0, 26.0);
        let life = r.range(0.22, 0.34);
        let size = r.range(1.6, 2.6);
        let delay = r.range(0.1, 0.3);
        let sp = Spawn {
            size,
            delay,
            ..spawn(
                Kind::Star,
                libm::cos(a) * d,
                y + libm::sin(a) * d,
                0.0,
                -3.0,
                life,
            )
        };
        ps.spawn(&mut r, sp);
    }
    Box::new(Blow {
        c,
        y,
        w: 0.0,
        s: 1.0,
        r,
        ps,
        lights,
        sp,
        segs,
        edges: [e0, e1],
        t: 0.0,
        parry: true,
    })
}

impl Blow {
    fn draw_parry(&self, p: &mut Painter) {
        let (c, y, t) = (&self.c, self.y, self.t);
        // the ring: rings out off the star and breaks into arcs that fly on and cool
        let u = (t - 0.03) / 0.36;
        if (0.0..1.0).contains(&u) {
            let r = 8.0 + 24.0 * ease::out_cubic(u);
            let i = if u < 0.15 {
                6
            } else if u < 0.35 {
                5
            } else if u < 0.6 {
                4
            } else {
                3
            };
            broken_ring(
                p,
                y,
                r,
                js_max(1.0, 2.6 * libm::pow(1.0 - u, 1.2)),
                &self.segs,
                smooth(0.15, 0.6, u),
                c[i],
                c[i.saturating_sub(2).max(2)],
                if u < 0.6 { 1.0 } else { (1.0 - u) / 0.4 },
                0.0,
            );
            if u < 0.5 {
                p.glow_ring(0.0, y, r, 5.0, c[4], 0.3 * (1.0 - u / 0.5));
            }
        }
        self.ps.draw(p, Layer::Front);
        // the edges flash where they met
        if t < 0.058 {
            for a in self.edges {
                let (ca, sa) = (libm::cos(a), libm::sin(a));
                let (l, k) = if t < 0.025 { (26.0, 1.0) } else { (32.0, 0.6) };
                let mut d = -l;
                while d <= l {
                    let q = libm::fabs(d) / l;
                    let col = if q < 0.4 {
                        WHITE
                    } else if q < 0.75 {
                        c[5]
                    } else {
                        c[4]
                    };
                    let al = if q < 0.6 { 1.0 } else { k * (1.3 - q) };
                    p.dot(ca * d, y + sa * d, col, al);
                    d += 0.5;
                }
                p.glow_line(-ca * l, y - sa * l, ca * l, y + sa * l, 4.0, c[5], 0.35 * k);
            }
        }
        // the star: white, then bigger and gold as it thins, then just a glint
        if t < 0.025 {
            star(p, y, &self.sp, 1.0, 1.0, [c[4], WHITE, WHITE, WHITE], 2.6);
        } else if t < 0.058 {
            star(p, y, &self.sp, 1.25, 0.65, [c[2], c[5], c[6], WHITE], 2.6);
        } else if t < 0.092 {
            star(p, y, &self.sp, 1.4, 0.3, [c[2], c[4], c[5], c[6]], 2.0);
        }
        let pop = libm::exp(-t / 0.05);
        if pop > 0.02 {
            p.glow(0.0, y, 30.0, c[4], 0.5 * pop);
            p.glow(0.0, y, 10.0, WHITE, 0.4 * pop);
        }
        self.lights.draw(p);
    }
}

// ---------------------------------------------------------------------------------------------------------------
// impacts/hit

/// The star holds white this long (the frame a hit-stop freezes on); the cut flashes across in `CUT`, gone by
/// `CUT * 3`, slanting up the way the blow goes.
const HOT: f64 = 0.034;
const CUT: f64 = 0.05;
const SLANT: f64 = -0.55;
/// How heavy the blow (the engine's `power` option; the duel passes none).
const POWER: f64 = 0.5;

pub fn hit(o: &Opts) -> Box<dyn Show> {
    let c = palette(o);
    let w = clamp(POWER, 0.0, 1.0);
    // everything grows with the blow
    let s = 0.75 + 0.6 * w;
    let mut r = Rng::new(o.seed as u32);
    let mut ps = Particles::new(ParticlesOpts {
        sparks: vec![c[6], c[5], c[4], c[2]],
        ..ParticlesOpts::default()
    });
    let mut lights = Lights::new();
    // the star: long along the blow (longest ahead of it), short across, shorter still between
    let mut sp = Vec::with_capacity(8);
    for i in 0..8 {
        let a = f64::from(i) * PI / 4.0 + r.range(-0.05, 0.05);
        let len = match i {
            0 => r.range(17.0, 20.0),
            4 => r.range(9.0, 11.0),
            2 | 6 => r.range(7.0, 9.0),
            _ => r.range(3.5, 5.0),
        } * s;
        let wd = if i % 2 == 1 { 1.0 } else { 2.0 };
        sp.push(Spike { a, len, w: wd });
    }
    let mut segs = Vec::with_capacity(5);
    for i in 0..5 {
        let a = (f64::from(i) / 5.0) * TAU + r.range(-0.15, 0.15);
        let gap = r.range(0.5, 0.75);
        let dr = r.range(0.1, 0.4);
        segs.push(Seg { a, gap, dr });
    }
    lights.add(0.0, 0.0, 34.0 + 20.0 * w, c[4], 0.55, 0.25);
    lights.add(0.0, 0.0, 14.0, WHITE, 0.7, 0.06);
    // sparks thrown the way the blow pushes, a few kicked back off the body
    let n = 6 + (6.0 * w) as usize;
    for i in 0..n {
        let back = i % 5 == 4;
        let a = if back {
            PI + r.range(-0.7, 0.7)
        } else {
            r.range(-0.75, 0.55)
        };
        let v = r.range(150.0, 260.0 + 140.0 * w) * if back { 0.6 } else { 1.0 };
        let life = r.range(0.16, 0.3 + 0.1 * w);
        let delay = if i > n / 2 { r.range(0.0, 0.03) } else { 0.0 };
        let q = Spawn {
            delay,
            ..spawn(
                Kind::Spark,
                libm::cos(a) * 2.0,
                libm::sin(a) * 2.0,
                libm::cos(a) * v,
                libm::sin(a) * v,
                life,
            )
        };
        ps.spawn(&mut r, q);
    }
    for _ in 0..(2 + (3.0 * w) as usize) {
        let a = r.range(-1.2, 1.2);
        let v = r.range(60.0, 130.0);
        let col = r.pick(&[c[5], c[6]]).copied();
        let life = r.range(0.2, 0.35);
        let delay = r.range(0.0, 0.03);
        let q = Spawn {
            col,
            size: 0.6,
            delay,
            ..spawn(
                Kind::Mote,
                0.0,
                0.0,
                libm::cos(a) * v,
                libm::sin(a) * v,
                life,
            )
        };
        ps.spawn(&mut r, q);
    }
    Box::new(Blow {
        c,
        y: 0.0,
        w,
        s,
        r,
        ps,
        lights,
        sp,
        segs,
        edges: [0.0, 0.0],
        t: 0.0,
        parry: false,
    })
}

impl Blow {
    /// The cut: a slanted crescent through the point struck, flashing across from its back end to its front.
    fn cut(&self, p: &mut Painter, t: f64) {
        let c = &self.c;
        let reach = smooth(0.0, CUT, t);
        let k = 1.0 - smooth(CUT, CUT * 3.0, t);
        if k <= 0.0 {
            return;
        }
        let half = 15.0 * self.s;
        let bend = 4.0 * self.s;
        let thick = (0.7 + 0.6 * self.w) * k;
        let (ca, sa) = (libm::cos(SLANT), libm::sin(SLANT));
        let (edge, body) = if k > 0.7 {
            (WHITE, c[5])
        } else if k > 0.4 {
            (c[5], c[4])
        } else {
            (c[4], c[2])
        };
        let mut u = -1.0;
        while u <= -1.0 + 2.0 * reach {
            let (along, off) = (u * half, bend * (1.0 - u * u));
            let (x, y) = (along * ca + off * sa, along * sa - off * ca);
            let w = js_max(0.5, thick * (1.0 - u * u));
            let mut o = -w;
            while o <= w {
                let col = if o < -w * 0.3 { edge } else { body };
                p.dot(x - sa * o, y + ca * o, col, 1.0);
                o += 0.5;
            }
            u += 0.04;
        }
        p.glow_line(
            -half * ca,
            -half * sa,
            half * ca,
            half * sa,
            4.0,
            c[5],
            0.3 * k,
        );
    }

    fn draw_hit(&self, p: &mut Painter) {
        let (c, t, w) = (&self.c, self.t, self.w);
        // a blow of some weight rings out, the ring breaking into arcs that fly on and cool
        if w >= 0.4 {
            let u = (t - 0.03) / 0.26;
            if (0.0..1.0).contains(&u) {
                let r = 5.0 + (10.0 + 14.0 * w) * ease::out_cubic(u);
                let i = if u < 0.2 {
                    6
                } else if u < 0.45 {
                    5
                } else {
                    4
                };
                broken_ring(
                    p,
                    0.0,
                    r,
                    js_max(1.0, 2.2 * libm::pow(1.0 - u, 1.3)),
                    &self.segs,
                    smooth(0.1, 0.55, u),
                    c[i],
                    c[i - 2],
                    if u < 0.6 { 1.0 } else { (1.0 - u) / 0.4 },
                    f64::INFINITY,
                );
            }
        }
        self.cut(p, t);
        self.ps.draw(p, Layer::Front);
        // the star: all white for the frame a hit-stop holds, then gold and stretched as it thins, then a glint
        if t < HOT {
            p.circle(0.0, 0.0, 4.0 * self.s, WHITE, 1.0);
            star(p, 0.0, &self.sp, 1.0, 1.0, [c[5], WHITE, WHITE, WHITE], 3.0);
        } else if t < HOT * 2.0 {
            star(p, 0.0, &self.sp, 1.25, 0.6, [c[3], c[5], c[6], WHITE], 2.4);
        } else if t < HOT * 3.0 {
            star(p, 0.0, &self.sp, 1.4, 0.3, [c[2], c[4], c[5], c[6]], 1.6);
        }
        let pop = libm::exp(-t / 0.05);
        if pop > 0.02 {
            p.glow(0.0, 0.0, 24.0 + 10.0 * w, c[4], 0.5 * pop);
            p.glow(0.0, 0.0, 9.0, WHITE, 0.45 * pop);
        }
        self.lights.draw(p);
    }
}

impl Show for Blow {
    fn update(&mut self, dt: f64) {
        self.t += dt;
        self.ps.update(&mut self.r, dt);
        self.lights.update(dt);
    }

    fn draw(&self, p: &mut Painter, _t: f64, layer: Layer) {
        if layer == Layer::Back {
            return;
        }
        if self.parry {
            self.draw_parry(p);
        } else {
            self.draw_hit(p);
        }
    }
}
