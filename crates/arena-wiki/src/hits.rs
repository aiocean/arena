//! Where a skill can hit, as the rules test it: a duel where one fighter casts the skill once at a foe who stands
//! still, every tick's shapes of that skill (`arena_duel::debug::shapes`, the source of the player's hitbox overlay)
//! and a bolt's sweep (a bolt strikes along the stretch it flies in a tick), and the skill's effects as the game plays
//! them (`Out::Play`, `Move`, `Stop`: every effect played from the cast on, the foe standing idle), drawn from the sheets main has at build time with the shapes over them.

use arena_calc::loadout::{Loadout, Slot};
use arena_duel::data::UNIT;
use arena_duel::debug::{Drawn, Mark, Shape, shapes};
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::out::{Out, PlayOpts};
use arena_duel::seat::{MakeSeat, Script};
use arena_duel::shot::ShotKind;
use arena_vfx::drawn::Drawn as DrawnFx;
use arena_vfx::sheet::Index;
use image::{Rgba, RgbaImage};
use std::collections::BTreeMap;
use std::path::Path;

/// The longest a trace follows a cast, seconds.
const LONGEST: f64 = 10.0;
/// How long after its last shape a trace waits for another, seconds.
const QUIET: f64 = 1.5;
/// Output pixels per arena pixel: one, so a sheet's pixel (`UNIT` arena pixels) is four.
const SCALE: f64 = 1.0;
/// Arena pixels kept round what a moment shows.
const MARGIN: f64 = 24.0;
/// At most this many moments drawn a skill.
const MOMENTS: usize = 4;

/// An effect playing: its id, its place (the rules', `y` down from the top), how it is played, when it began.
#[derive(Clone, Debug)]
pub struct Playing {
    pub fx: &'static str,
    pub x: f64,
    pub y: f64,
    pub opts: PlayOpts,
    pub born: f64,
}

/// A bolt's sweep in one tick: from (`x0`, `y0`) to (`x1`, `y1`), `r` round it (arena pixels, `y` up).
#[derive(Clone, Copy, Debug)]
pub struct Sweep {
    pub x0: f64,
    pub y0: f64,
    pub x1: f64,
    pub y1: f64,
    pub r: f64,
}

/// One tick a skill could hit on: seconds since its cast began, its shapes, its bolts' sweeps, its effects playing,
/// the foe's body.
#[derive(Clone, Debug)]
pub struct Moment {
    pub t: f64,
    pub shapes: Vec<Drawn>,
    pub sweeps: Vec<Sweep>,
    pub playing: Vec<Playing>,
    pub foe: Vec<[f64; 4]>,
}

/// A skill's trace: every tick it could hit on, the arena's ground and top (`y` down), its wind-up.
#[derive(Clone, Debug)]
pub struct Trace {
    pub moments: Vec<Moment>,
    pub ground: f64,
    pub windup: f64,
}

impl Trace {
    /// The stretches of time it can hit in, seconds since the cast began: runs of ticks.
    pub fn windows(&self) -> Vec<(f64, f64)> {
        let mut out: Vec<(f64, f64)> = Vec::new();
        for m in &self.moments {
            match out.last_mut() {
                Some(w) if m.t - w.1 < TICK * 1.5 => w.1 = m.t,
                _ => out.push((m.t, m.t)),
            }
        }
        out
    }

    /// The moments to draw: the first, the last and some between.
    pub fn picked(&self) -> Vec<&Moment> {
        let n = self.moments.len();
        if n <= MOMENTS {
            return self.moments.iter().collect();
        }
        let mut idx: Vec<usize> = (0..MOMENTS).map(|i| i * (n - 1) / (MOMENTS - 1)).collect();
        idx.dedup();
        idx.into_iter().map(|i| &self.moments[i]).collect()
    }
}

/// The marks that hit (not a body, nor a hand's points).
fn hits(m: Mark) -> bool {
    matches!(m, Mark::Strike | Mark::Shot | Mark::Field | Mark::Unit)
}

/// A seat that casts `id` until its cast is under way, then stands.
fn caster(id: &'static str) -> MakeSeat {
    Box::new(move |_, _| {
        let mut begun = false;
        Box::new(Script(Box::new(move |s, me| {
            begun |= s.fighters[me].act.is_some_and(|a| a.spell == id);
            (!begun).then(|| arena_mind::Choice::Cast {
                skill: id.to_owned(),
                target: None,
            })
        })))
    })
}

fn idle() -> MakeSeat {
    Box::new(|_, _| Box::new(Script(Box::new(|_, _| None))))
}

/// A loadout of `id` padded to a full bar as the duel's tests pad theirs, with no supports and no keystones.
fn alone(id: &str) -> Loadout {
    let pad = [
        "bolt", "palm", "fireball", "quake", "heal", "ward", "spark", "seal", "jet", "orb",
    ];
    let mut skills = vec![id];
    for p in pad {
        if skills.len() >= arena_calc::loadout::ACTIVES {
            break;
        }
        if !skills.contains(&p) {
            skills.push(p);
        }
    }
    Loadout {
        actives: skills
            .into_iter()
            .map(|s| Slot {
                skill: s.to_owned(),
                supports: Vec::new(),
                fuse: None,
            })
            .collect(),
        keystones: Vec::new(),
        root: None,
    }
}

/// A duel where seat 0 casts `id` once at seat 1, who stands still; `None` when it cannot be cast alone.
fn one_cast(id: &'static str) -> Option<Duel> {
    arena_duel::data::spell(id)?;
    let config = DuelConfig {
        seed: 1,
        left: 0.0,
        right: 880.0,
        ground: 600.0,
        sky: 384.0,
        effects: arena_duel::effects::all(),
    };
    Duel::with_loadouts(config, [caster(id), idle()], [alone(id), alone("palm")]).ok()
}

/// An effect beside the fighters in one cast: its id, its point (arena pixels, `y` down), whether it is mirrored,
/// seconds since the cast began, both fighters' hurtboxes then and the skill's hitboxes then (`[x0, y0, x1, y1]`, `y`
/// down; a circle is its box). The moment is the first tick the skill can hit while the effect plays, else the tick
/// it is first played.
#[derive(Clone, Debug)]
pub struct Play {
    pub fx: &'static str,
    pub x: f64,
    pub y: f64,
    pub flip: bool,
    pub t: f64,
    pub bodies: [Vec<[f64; 4]>; 2],
    pub hits: Vec<(Mark, [f64; 4])>,
}

/// The arena's ground in the one-cast duel (`y` down).
pub const FLOOR: f64 = 600.0;

/// A shape's box, `y` down.
fn down(s: Shape) -> [f64; 4] {
    match s {
        Shape::Rect { x0, y0, x1, y1 } => [x0, FLOOR - y1, x1, FLOOR - y0],
        Shape::Circle { x, y, r } => [x - r, FLOOR - y - r, x + r, FLOOR - y + r],
    }
}

/// Every effect one cast of skill `id` plays, each once, with the fighters' bodies and the skill's hitboxes.
pub fn plays(id: &'static str) -> Vec<Play> {
    let Some(mut duel) = one_cast(id) else {
        return Vec::new();
    };
    let mut out: Vec<Play> = Vec::new();
    // the effects playing now: key -> (fx, x, y, flip)
    let mut playing: BTreeMap<u32, (&'static str, f64, f64, bool)> = BTreeMap::new();
    let (mut clock, mut began) = (0.0, None::<f64>);
    while clock < LONGEST {
        let step = duel.step(TICK);
        clock += TICK;
        let state = duel.state();
        if began.is_none() {
            began = state.fighters[0]
                .act
                .filter(|a| a.spell == id)
                .map(|a| clock - a.t);
        }
        let t = clock - began.unwrap_or(clock);
        for o in step {
            match o {
                Out::Play {
                    key,
                    fx,
                    x,
                    y,
                    opts,
                    ..
                } => {
                    playing.insert(key, (fx, x, y, opts.flip == Some(true)));
                }
                Out::Move { key, x, y } => {
                    if let Some(p) = playing.get_mut(&key) {
                        (p.1, p.2) = (x, y);
                    }
                }
                Out::Stop { key } => {
                    playing.remove(&key);
                }
                _ => {}
            }
        }
        let all = shapes(state);
        let bodies = [0, 1].map(|me| {
            all.iter()
                .filter(|d| d.owner == Some(me) && d.mark == Mark::Body)
                .map(|d| down(d.shape))
                .collect::<Vec<_>>()
        });
        let hitting: Vec<(Mark, [f64; 4])> = all
            .iter()
            .filter(|d| d.skill == id && hits(d.mark))
            .map(|d| (d.mark, down(d.shape)))
            .collect();
        for &(fx, x, y, flip) in playing.values() {
            let now = Play {
                fx,
                x,
                y,
                flip,
                t,
                bodies: bodies.clone(),
                hits: hitting.clone(),
            };
            match out.iter_mut().find(|p| p.fx == fx) {
                None => out.push(now),
                // a moment it can hit beats one it cannot
                Some(p) if p.hits.is_empty() && !now.hits.is_empty() => *p = now,
                Some(_) => {}
            }
        }
        if began.is_some_and(|b| clock - b > 4.0) {
            break;
        }
    }
    out
}

/// The trace of one cast of skill `id` by seat 0 at a foe who stands still; `None` when it cannot be cast alone or
/// never hits.
pub fn trace(id: &'static str) -> Option<Trace> {
    let mut duel = one_cast(id)?;
    let mut playing: BTreeMap<u32, Playing> = BTreeMap::new();
    let mut bolts: BTreeMap<u32, (f64, f64)> = BTreeMap::new();
    let mut moments = Vec::new();
    let (mut began, mut windup) = (None::<f64>, 0.0);
    let mut clock = 0.0;
    while clock < LONGEST + 3.0 {
        let out = duel.step(TICK);
        clock += TICK;
        let state = duel.state();
        if began.is_none() {
            if let Some(a) = state.fighters[0].act.filter(|a| a.spell == id) {
                began = Some(clock - a.t);
                windup = a.windup;
            } else {
                if clock > 3.0 {
                    return None;
                }
                continue;
            }
        }
        let t = clock - began.unwrap_or(clock);
        for o in out {
            match o {
                Out::Play {
                    key,
                    fx,
                    x,
                    y,
                    opts,
                } => {
                    playing.insert(
                        key,
                        Playing {
                            fx,
                            x,
                            y,
                            opts,
                            born: t,
                        },
                    );
                }
                Out::Move { key, x, y } => {
                    if let Some(p) = playing.get_mut(&key) {
                        p.x = x;
                        p.y = y;
                    }
                }
                Out::Stop { key } => {
                    playing.remove(&key);
                }
                _ => {}
            }
        }
        let all = shapes(state);
        let mine: Vec<Drawn> = all
            .iter()
            .filter(|d| d.skill == id && hits(d.mark))
            .copied()
            .collect();
        let mut sweeps = Vec::new();
        let mut now = BTreeMap::new();
        for s in state.shots.iter().filter(|s| s.name == id) {
            if let (ShotKind::Bolt { .. }, Some((x, y, _))) = (&s.kind, s.bolt_at(state)) {
                if let Some(&(px, py)) = bolts.get(&s.id) {
                    sweeps.push(Sweep {
                        x0: px,
                        y0: py,
                        x1: x,
                        y1: y,
                        r: s.spell.radius.unwrap_or(0.0),
                    });
                }
                now.insert(s.id, (x, y));
            }
        }
        bolts = now;
        if !mine.is_empty() {
            moments.push(Moment {
                t,
                shapes: mine,
                sweeps,
                playing: playing.values().cloned().collect(),
                foe: all
                    .iter()
                    .filter(|d| d.owner == Some(1) && d.mark == Mark::Body)
                    .filter_map(|d| match d.shape {
                        Shape::Rect { x0, y0, x1, y1 } => Some([x0, y0, x1, y1]),
                        Shape::Circle { .. } => None,
                    })
                    .collect(),
            });
        }
        let quiet = match moments.last() {
            Some(m) => t - m.t > QUIET,
            None => t > windup + 3.0,
        };
        if t > LONGEST || quiet {
            break;
        }
    }
    (!moments.is_empty()).then_some(Trace {
        moments,
        ground: 600.0,
        windup,
    })
}

/// The effects' sheets, read once: drawn ones and baked ones with their pages.
pub struct Sheets {
    drawn: Vec<(DrawnFx, Vec<RgbaImage>)>,
    baked: Index,
    pages: Vec<RgbaImage>,
}

impl Sheets {
    pub fn read(assets: &Path) -> Result<Sheets, String> {
        let text =
            |p: &str| std::fs::read_to_string(assets.join(p)).map_err(|e| format!("{p}: {e}"));
        let mut drawn = Vec::new();
        for d in arena_vfx::drawn::read(&text("drawn/drawn.json")?)? {
            let mut pages = Vec::new();
            for i in 0..d.layout.pages.len() {
                let name = if i == 0 {
                    format!("drawn/{}.webp", d.file)
                } else {
                    format!("drawn/{}-{i}.webp", d.file)
                };
                pages.push(crate::img::page(&assets.join(name))?);
            }
            drawn.push((d, pages));
        }
        let baked = Index::read(&text("vfx/index.txt")?)?;
        let mut pages = Vec::new();
        for (file, _) in &baked.pages {
            pages.push(crate::img::page(&assets.join("vfx").join(file))?);
        }
        Ok(Sheets {
            drawn,
            baked,
            pages,
        })
    }
}

/// A picture placed in the arena: a cell of a page, its centre (arena pixels, `y` down), arena pixels per sheet
/// pixel, turned `angle` radians clockwise, mirrored.
struct Placed<'a> {
    page: &'a RgbaImage,
    cell: (u32, u32, u32, u32),
    cx: f64,
    cy: f64,
    k: f64,
    angle: f64,
    flip: bool,
}

/// Where effect `p` shows `t` seconds into the cast, as the game draws it (`arena-game`'s `vfx::drawn`,
/// `vfx::sprites`); none once it has played out.
fn place<'a>(sheets: &'a Sheets, p: &Playing, t: f64, ground: f64) -> Vec<Placed<'a>> {
    let age = (t - p.born).max(0.0);
    let flip = p.opts.flip == Some(true);
    if let Some((d, pages)) = sheets.drawn.iter().find(|(d, _)| d.fx == p.fx) {
        let school = arena_vfx::drawn::school_of(p.opts.palette);
        let Some(s) = d.shot(school, age as f32) else {
            return Vec::new();
        };
        let Some(page) = pages.get(s.cell.page as usize) else {
            return Vec::new();
        };
        // world pixels a sheet pixel, and the cell's centre from the anchor turned with it (y down)
        let k = f64::from(s.size * d.px());
        let (ox, oy) = (
            (f64::from(s.cell.dx) + f64::from(s.cell.w) / 2.0) * k,
            (f64::from(s.cell.dy) + f64::from(s.cell.h) / 2.0) * k,
        );
        let a = f64::from(s.angle);
        let (sin, cos) = (libm::sin(a), libm::cos(a));
        let mut cx = ox * cos - oy * sin + f64::from(s.at.0);
        let cy = ox * sin + oy * cos + f64::from(s.at.1);
        if flip {
            cx = -cx;
        }
        let ay = if d.layout.motion.ground { ground } else { p.y };
        return vec![Placed {
            page,
            cell: (s.cell.x, s.cell.y, s.cell.w, s.cell.h),
            cx: p.x + cx * UNIT,
            cy: ay + cy * UNIT,
            k: k * UNIT,
            angle: if flip { -a } else { a },
            flip,
        }];
    }
    let key = arena_vfx::variant_key(&p.opts);
    let Some(v) = sheets
        .baked
        .variants
        .iter()
        .find(|v| v.fx == p.fx && v.key == key)
        .or_else(|| sheets.baked.variants.iter().find(|v| v.fx == p.fx))
    else {
        return Vec::new();
    };
    let n = v.front.len().max(v.back.len());
    let mut f = (age * f64::from(v.fps)) as usize;
    if f >= n {
        if !v.looping || n == 0 {
            return Vec::new();
        }
        let r = v.restart.min(n - 1);
        f = r + (f - r) % (n - r);
    }
    [v.back.get(f), v.front.get(f)]
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|c| {
            let page = sheets.pages.get(c.page as usize)?;
            let cx = f64::from(c.dx) + f64::from(c.w) / 2.0;
            let cy = f64::from(c.dy) + f64::from(c.h) / 2.0;
            Some(Placed {
                page,
                cell: (c.x, c.y, c.w, c.h),
                cx: p.x + (if flip { -cx } else { cx }) * UNIT,
                cy: p.y + cy * UNIT,
                k: UNIT,
                angle: 0.0,
                flip,
            })
        })
        .collect()
}

impl Placed<'_> {
    /// Its corners' box, arena pixels (`y` down).
    fn bounds(&self) -> [f64; 4] {
        let (w, h) = (
            f64::from(self.cell.2) * self.k,
            f64::from(self.cell.3) * self.k,
        );
        let r = libm::hypot(w, h) / 2.0;
        let (hw, hh) = if self.angle == 0.0 {
            (w / 2.0, h / 2.0)
        } else {
            (r, r)
        };
        [self.cx - hw, self.cy - hh, self.cx + hw, self.cy + hh]
    }
}

const SHOT: Rgba<u8> = Rgba([255, 70, 70, 255]);
const FIELD: Rgba<u8> = Rgba([255, 160, 40, 255]);
const UNITS: Rgba<u8> = Rgba([90, 230, 120, 255]);
const BODY: Rgba<u8> = Rgba([90, 180, 255, 160]);
const GROUND: Rgba<u8> = Rgba([110, 100, 130, 255]);
const BACK: Rgba<u8> = Rgba([16, 14, 23, 255]);

fn colour(m: Mark) -> Rgba<u8> {
    match m {
        Mark::Field => FIELD,
        Mark::Unit => UNITS,
        _ => SHOT,
    }
}

/// Moment `m` drawn: its effects from the sheets, the foe's body, the skill's shapes and sweeps over them.
pub fn draw(sheets: &Sheets, m: &Moment, ground: f64) -> Option<RgbaImage> {
    let placed: Vec<Placed> = m
        .playing
        .iter()
        .flat_map(|p| place(sheets, p, m.t, ground))
        .collect();
    // the box round the shapes (y down) and the effects, kept to the shapes' neighbourhood
    let mut bx = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    let grow = |bx: &mut [f64; 4], b: [f64; 4]| {
        bx[0] = bx[0].min(b[0]);
        bx[1] = bx[1].min(b[1]);
        bx[2] = bx[2].max(b[2]);
        bx[3] = bx[3].max(b[3]);
    };
    for d in &m.shapes {
        let b = match d.shape {
            Shape::Rect { x0, y0, x1, y1 } => [x0, ground - y1, x1, ground - y0],
            Shape::Circle { x, y, r } => [x - r, ground - y - r, x + r, ground - y + r],
        };
        grow(&mut bx, b);
    }
    for s in &m.sweeps {
        let b = [
            s.x0.min(s.x1) - s.r,
            ground - s.y0.max(s.y1) - s.r,
            s.x0.max(s.x1) + s.r,
            ground - s.y0.min(s.y1) + s.r,
        ];
        grow(&mut bx, b);
    }
    if bx[0] > bx[2] {
        return None;
    }
    let near = [bx[0] - 160.0, bx[1] - 160.0, bx[2] + 160.0, bx[3] + 160.0];
    for p in &placed {
        let b = p.bounds();
        let b = [
            b[0].max(near[0]),
            b[1].max(near[1]),
            b[2].min(near[2]),
            b[3].min(near[3]),
        ];
        if b[0] < b[2] && b[1] < b[3] {
            grow(&mut bx, b);
        }
    }
    let [x0, y0, x1, y1] = bx;
    let (x0, y0) = (x0 - MARGIN, (y0 - MARGIN).min(ground - 8.0));
    let (x1, y1) = (
        x1 + MARGIN,
        (y1 + MARGIN).max(ground + 8.0).min(ground + 16.0),
    );
    let to = |x: f64, y: f64| ((x - x0) * SCALE, (y - y0) * SCALE);
    let (w, h) = (
        ((x1 - x0) * SCALE).ceil().max(1.0) as u32,
        ((y1 - y0) * SCALE).ceil().max(1.0) as u32,
    );
    let mut img = RgbaImage::from_pixel(w, h, BACK);
    let gy = to(0.0, ground).1.round() as i32;
    for x in 0..w as i32 {
        put(&mut img, x, gy, GROUND);
    }
    for b in &m.foe {
        let (l, t) = to(b[0], ground - b[3]);
        let (r, bt) = to(b[2], ground - b[1]);
        rect(&mut img, l, t, r, bt, BODY);
    }
    for p in &placed {
        blit(&mut img, p, &to);
    }
    for s in &m.sweeps {
        capsule(
            &mut img,
            to(s.x0, ground - s.y0),
            to(s.x1, ground - s.y1),
            s.r * SCALE,
        );
    }
    for d in &m.shapes {
        let c = colour(d.mark);
        match d.shape {
            Shape::Rect { x0, y0, x1, y1 } => {
                let (l, t) = to(x0, ground - y1);
                let (r, b) = to(x1, ground - y0);
                rect(&mut img, l, t, r, b, c);
            }
            Shape::Circle { x, y, r } => {
                let (cx, cy) = to(x, ground - y);
                circle(&mut img, cx, cy, (r * SCALE).max(1.0), c);
            }
        }
    }
    Some(img)
}

fn put(img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        let a = u32::from(c[3]);
        let p = img.get_pixel_mut(x as u32, y as u32);
        for i in 0..3 {
            p[i] = ((u32::from(c[i]) * a + u32::from(p[i]) * (255 - a)) / 255) as u8;
        }
        p[3] = 255;
    }
}

fn rect(img: &mut RgbaImage, l: f64, t: f64, r: f64, b: f64, c: Rgba<u8>) {
    let (l, t, r, b) = (
        l.round() as i32,
        t.round() as i32,
        r.round() as i32,
        b.round() as i32,
    );
    for x in l..=r {
        put(img, x, t, c);
        put(img, x, b, c);
    }
    for y in t + 1..b {
        put(img, l, y, c);
        put(img, r, y, c);
    }
}

fn circle(img: &mut RgbaImage, cx: f64, cy: f64, r: f64, c: Rgba<u8>) {
    let n = ((r * 8.0) as usize).max(24);
    for i in 0..n {
        let a = i as f64 / n as f64 * core::f64::consts::TAU;
        put(
            img,
            (cx + r * libm::cos(a)).round() as i32,
            (cy + r * libm::sin(a)).round() as i32,
            c,
        );
    }
}

/// The stretch a bolt sweeps in a tick, filled faintly and outlined.
fn capsule(img: &mut RgbaImage, (ax, ay): (f64, f64), (bx, by): (f64, f64), r: f64) {
    let r = r.max(1.0);
    let (l, t) = (
        (ax.min(bx) - r).floor() as i32,
        (ay.min(by) - r).floor() as i32,
    );
    let (rt, bt) = (
        (ax.max(bx) + r).ceil() as i32,
        (ay.max(by) + r).ceil() as i32,
    );
    let (dx, dy) = (bx - ax, by - ay);
    let len2 = (dx * dx + dy * dy).max(1e-9);
    for y in t..=bt {
        for x in l..=rt {
            let (px, py) = (f64::from(x) - ax, f64::from(y) - ay);
            let u = ((px * dx + py * dy) / len2).clamp(0.0, 1.0);
            let d = libm::hypot(px - u * dx, py - u * dy);
            if d <= r {
                let edge = d > r - 1.0;
                put(img, x, y, Rgba([255, 70, 70, if edge { 255 } else { 60 }]));
            }
        }
    }
}

/// Picture `p` onto `img`, nearest neighbour, turned and mirrored as placed.
fn blit(img: &mut RgbaImage, p: &Placed, to: &dyn Fn(f64, f64) -> (f64, f64)) {
    let b = p.bounds();
    let (l, t) = to(b[0], b[1]);
    let (r, bt) = to(b[2], b[3]);
    let (cx, cy) = to(p.cx, p.cy);
    let s = p.k * SCALE;
    if s <= 0.0 {
        return;
    }
    let (sin, cos) = (libm::sin(-p.angle), libm::cos(-p.angle));
    let (cw, ch) = (f64::from(p.cell.2), f64::from(p.cell.3));
    for y in (t.floor() as i32).max(0)..=(bt.ceil() as i32).min(img.height() as i32 - 1) {
        for x in (l.floor() as i32).max(0)..=(r.ceil() as i32).min(img.width() as i32 - 1) {
            let (vx, vy) = (f64::from(x) + 0.5 - cx, f64::from(y) + 0.5 - cy);
            let (mut ux, uy) = (vx * cos - vy * sin, vx * sin + vy * cos);
            if p.flip {
                ux = -ux;
            }
            let (sx, sy) = (ux / s + cw / 2.0, uy / s + ch / 2.0);
            if sx < 0.0 || sy < 0.0 || sx >= cw || sy >= ch {
                continue;
            }
            let (px, py) = (p.cell.0 + sx as u32, p.cell.1 + sy as u32);
            if px < p.page.width() && py < p.page.height() {
                let c = *p.page.get_pixel(px, py);
                if c[3] > 0 {
                    put(img, x, y, c);
                }
            }
        }
    }
}
