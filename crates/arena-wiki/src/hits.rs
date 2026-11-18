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
use std::collections::BTreeMap;
use std::path::Path;

/// The longest a trace follows a cast, seconds.
const LONGEST: f64 = 10.0;
/// How long after its last shape a trace waits for another, seconds.
const QUIET: f64 = 1.5;

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

/// The effects' layouts, read once: the drawn ones and the baked ones (the pictures stay on their pages: the player
/// loads them from the game's).
pub struct Sheets {
    drawn: Vec<DrawnFx>,
    baked: Index,
}

impl Sheets {
    pub fn read(assets: &Path) -> Result<Sheets, String> {
        let text =
            |p: &str| std::fs::read_to_string(assets.join(p)).map_err(|e| format!("{p}: {e}"));
        Ok(Sheets {
            drawn: arena_vfx::drawn::read(&text("drawn/drawn.json")?)?,
            baked: Index::read(&text("vfx/index.txt")?)?,
        })
    }
}

/// A picture placed in the arena: a cell of a page (and the page's file under the game's assets), its centre (arena
/// pixels, `y` down), arena pixels per sheet pixel, turned `angle` radians clockwise, mirrored.
struct Placed {
    file: String,
    cell: (u32, u32, u32, u32),
    cx: f64,
    cy: f64,
    k: f64,
    angle: f64,
    flip: bool,
}

/// Where effect `p` shows `t` seconds into the cast, as the game draws it (`arena-game`'s `vfx::drawn`,
/// `vfx::sprites`); none once it has played out.
fn place(sheets: &Sheets, p: &Playing, t: f64, ground: f64) -> Vec<Placed> {
    let age = (t - p.born).max(0.0);
    let flip = p.opts.flip == Some(true);
    if let Some(d) = sheets.drawn.iter().find(|d| d.fx == p.fx) {
        let school = arena_vfx::drawn::school_of(p.opts.palette);
        let Some(s) = d.shot(school, age as f32) else {
            return Vec::new();
        };
        if s.cell.page as usize >= d.layout.pages.len() {
            return Vec::new();
        }
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
        let i = s.cell.page;
        return vec![Placed {
            file: if i == 0 {
                format!("drawn/{}.webp", d.file)
            } else {
                format!("drawn/{}-{i}.webp", d.file)
            },
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
            let cx = f64::from(c.dx) + f64::from(c.w) / 2.0;
            let cy = f64::from(c.dy) + f64::from(c.h) / 2.0;
            Some(Placed {
                file: format!("vfx/{}", sheets.baked.pages.get(c.page as usize)?.0),
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

/// A fighter's frame on its sheet, from its feet (sheet pixels, one an arena pixel, `y` down): page, x, y, w, h, dx, dy.
pub type FrameCell = [i64; 7];

/// One cast of skill `id` as the player shows it, tick by tick from the cast on, until a while after the last tick
/// it could hit (or its effects end): each fighter's clip and frame on its sheet (`cell`), where it stands and faces,
/// its act's phase; every shape the rules test (`arena_duel::debug::shapes`: bodies, blows, shots, fields, units,
/// hands) and a bolt's sweep in the tick; every effect as the game draws it. Arena pixels, `y` up from the floor.
/// `None` when the skill cannot be cast alone.
pub fn record(
    id: &'static str,
    sheets: &Sheets,
    cell: &dyn Fn(arena_duel::data::Look, &str, usize) -> Option<FrameCell>,
) -> Option<serde_json::Value> {
    use serde_json::json;
    let r1 = |v: f64| (v * 10.0).round() / 10.0;
    let mut duel = one_cast(id)?;
    let mut playing: BTreeMap<u32, Playing> = BTreeMap::new();
    let mut bolts: BTreeMap<u32, (f64, f64)> = BTreeMap::new();
    let mut frames = Vec::new();
    let (mut clock, mut began, mut last_hit) = (0.0, None::<f64>, None::<f64>);
    while clock < LONGEST {
        let out = duel.step(TICK);
        clock += TICK;
        let state = duel.state();
        if began.is_none() {
            began = state.fighters[0]
                .act
                .filter(|a| a.spell == id)
                .map(|a| clock - a.t);
            if began.is_none() {
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
        let fighters: Vec<serde_json::Value> = state
            .fighters
            .iter()
            .enumerate()
            .map(|(me, f)| {
                let b = &f.body;
                let (clip, i) = arena_duel::frames::shown(state, me).unwrap_or(("", 0));
                let (window, all) = arena_duel::frames::clip_of(f.look, clip).unwrap_or((None, &[]));
                json!({
                    "clip": clip, "i": i, "n": all.len(), "window": window,
                    "x": r1(b.x), "air": r1(b.air), "face": b.face,
                    "cell": cell(f.look, clip, i),
                    "act": f.act.map(|a| json!({
                        "spell": a.spell,
                        "phase": match a.phase { arena_duel::fighter::Phase::Windup => "windup", arena_duel::fighter::Phase::Recover => "recover" },
                        "t": r1(a.t * 1000.0) / 1000.0, "windup": a.windup, "recover": a.recover,
                    })),
                })
            })
            .collect();
        let all = shapes(state);
        let shapes_now: Vec<serde_json::Value> = all
            .iter()
            .filter(|d| !hits(d.mark) || d.skill == id)
            .map(|d| {
                let shape = match d.shape {
                    Shape::Rect { x0, y0, x1, y1 } => {
                        json!({"rect": [r1(x0), r1(y0), r1(x1), r1(y1)]})
                    }
                    Shape::Circle { x, y, r } => json!({"circle": [r1(x), r1(y), r1(r)]}),
                };
                json!({"mark": format!("{:?}", d.mark), "owner": d.owner, "shape": shape})
            })
            .collect();
        if all.iter().any(|d| d.skill == id && hits(d.mark)) {
            last_hit = Some(t);
        }
        let mut sweeps = Vec::new();
        let mut now = BTreeMap::new();
        for s in state.shots.iter().filter(|s| s.name == id) {
            if let (ShotKind::Bolt { .. }, Some((x, y, _))) = (&s.kind, s.bolt_at(state)) {
                if let Some(&(px, py)) = bolts.get(&s.id) {
                    sweeps.push(json!([
                        r1(px),
                        r1(py),
                        r1(x),
                        r1(y),
                        r1(s.spell.radius.unwrap_or(0.0))
                    ]));
                }
                now.insert(s.id, (x, y));
            }
        }
        bolts = now;
        let effects: Vec<serde_json::Value> = playing
            .values()
            .flat_map(|p| place(sheets, p, t, FLOOR))
            .map(|pl| {
                json!({
                    "file": pl.file,
                    "cell": [pl.cell.0, pl.cell.1, pl.cell.2, pl.cell.3],
                    "cx": r1(pl.cx), "cy": r1(FLOOR - pl.cy), "k": pl.k,
                    "angle": (pl.angle * 1000.0).round() / 1000.0, "flip": pl.flip,
                })
            })
            .collect();
        frames.push(json!({
            "t": (t * 1000.0).round() / 1000.0,
            "fighters": fighters, "shapes": shapes_now, "sweeps": sweeps, "effects": effects,
        }));
        // a while after the last tick it could hit, or after its act and its effects are over
        let busy = state.fighters[0].act.is_some_and(|a| a.spell == id) || !playing.is_empty();
        let done = match last_hit {
            Some(h) => t - h > 0.6 && (!busy || t - h > QUIET),
            None => !busy && t > 0.3,
        };
        if done || t > 6.0 {
            break;
        }
    }
    (!frames.is_empty()).then(|| json!({ "skill": id, "tick": TICK, "frames": frames }))
}

/// Each effect the skills of the pool play, once: the skill and the play, on the first tick the skill can hit while
/// the effect plays (else when it is first played).
pub fn first_plays() -> BTreeMap<&'static str, (&'static str, Play)> {
    let mut first: BTreeMap<&'static str, (&'static str, Play)> = BTreeMap::new();
    for g in arena_calc::gem::GEMS {
        for p in plays(g.id) {
            let better = match first.get(p.fx) {
                None => true,
                Some((_, q)) => q.hits.is_empty() && !p.hits.is_empty(),
            };
            if better {
                first.insert(p.fx, (g.id, p));
            }
        }
    }
    first
}

/// Effect `d`'s largest frame where the game draws it for `play` (facing east, its point at the rules'), with both
/// fighters' hurtboxes (blue the caster, pink the foe), the skill's hitboxes (red a blow or shot, orange a field, green
/// a unit), the floor and the point (gold): `k` output pixels a sheet pixel.
pub fn effect_shot(
    play: &Play,
    d: &DrawnFx,
    pages: &[image::RgbaImage],
    k: i32,
) -> Option<image::RgbaImage> {
    use image::{Rgba, RgbaImage};
    let cells = d.layout.clips.values().next()?;
    let c = cells.iter().max_by_key(|c| c.w * c.h)?;
    let page = pages.get(c.page as usize)?;
    // the rules' boxes from the point, sheet pixels, `y` down, facing east
    let rel = |b: &[f64; 4]| {
        let (mut x0, mut x1) = ((b[0] - play.x) / UNIT, (b[2] - play.x) / UNIT);
        if play.flip {
            (x0, x1) = (-x1, -x0);
        }
        [x0, (b[1] - play.y) / UNIT, x1, (b[3] - play.y) / UNIT]
    };
    let floor = (FLOOR - play.y) / UNIT;
    let lift = if d.layout.motion.ground { floor } else { 0.0 };
    let bodies: Vec<([f64; 4], Rgba<u8>)> = play
        .bodies
        .iter()
        .zip([Rgba([90, 180, 255, 255]), Rgba([255, 140, 200, 255])])
        .flat_map(|(bs, c)| bs.iter().map(move |b| (rel(b), c)))
        .collect();
    let hits: Vec<([f64; 4], Rgba<u8>)> = play
        .hits
        .iter()
        .map(|(m, b)| {
            let c = match m {
                Mark::Field => Rgba([255, 160, 40, 255]),
                Mark::Unit => Rgba([90, 230, 120, 255]),
                _ => Rgba([255, 70, 70, 255]),
            };
            (rel(b), c)
        })
        .collect();
    let (dx, dy) = (f64::from(c.dx), f64::from(c.dy) + lift);
    let mut bx = [
        dx.min(-1.0),
        dy.min(-1.0),
        (dx + f64::from(c.w)).max(1.0),
        (dy + f64::from(c.h)).max(floor + 2.0),
    ];
    for (b, _) in bodies.iter().chain(hits.iter()) {
        bx = [
            bx[0].min(b[0]),
            bx[1].min(b[1]),
            bx[2].max(b[2]),
            bx[3].max(b[3]),
        ];
    }
    let (x0, y0) = (bx[0].floor() as i32 - 4, bx[1].floor() as i32 - 4);
    let (w, h) = (bx[2].ceil() as i32 + 4 - x0, bx[3].ceil() as i32 + 4 - y0);
    let mut img = RgbaImage::from_pixel((w * k) as u32, (h * k) as u32, Rgba([20, 18, 28, 255]));
    let at = |x: f64, y: f64| {
        (
            ((x - f64::from(x0)) * f64::from(k)).round() as i32,
            ((y - f64::from(y0)) * f64::from(k)).round() as i32,
        )
    };
    let put = |img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>| {
        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
            let a = u32::from(c[3]);
            let p = img.get_pixel_mut(x as u32, y as u32);
            for i in 0..3 {
                p[i] = ((u32::from(c[i]) * a + u32::from(p[i]) * (255 - a)) / 255) as u8;
            }
        }
    };
    let (_, gy) = at(0.0, floor);
    for x in 0..w * k {
        put(&mut img, x, gy, Rgba([110, 100, 130, 255]));
    }
    let (fx0, fy0) = at(dx, dy);
    for v in 0..c.h as i32 * k {
        for u in 0..c.w as i32 * k {
            let (px, py) = (c.x + (u / k) as u32, c.y + (v / k) as u32);
            if px < page.width() && py < page.height() {
                let p = *page.get_pixel(px, py);
                if p[3] > 0 {
                    put(&mut img, fx0 + u, fy0 + v, p);
                }
            }
        }
    }
    for (b, c) in bodies.iter().chain(hits.iter()) {
        let (l, t) = at(b[0], b[1]);
        let (r, bt) = at(b[2], b[3]);
        for x in l..=r {
            put(&mut img, x, t, *c);
            put(&mut img, x, bt, *c);
        }
        for y in t..=bt {
            put(&mut img, l, y, *c);
            put(&mut img, r, y, *c);
        }
    }
    let (gx, gy) = at(0.0, 0.0);
    for i in -3..=3 {
        put(&mut img, gx + i, gy, Rgba([243, 201, 105, 255]));
        put(&mut img, gx, gy + i, Rgba([243, 201, 105, 255]));
    }
    Some(img)
}
