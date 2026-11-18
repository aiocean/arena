//! The spells' hit shapes against their drawn effects, offline: real fights run on the rules, and for every effect
//! the rules play, frame by frame, where its art is drawn (placed as the game places it: a drawn sheet by
//! `arena_vfx::drawn`'s `Drawn::shot` and the game's `vfx::drawn::place`, a baked one by its cell's offset) against
//! the shape the rules test hits with (`arena_duel::debug::shapes`, the one source). It measures the art's extent
//! against the shape's (scale), how far their centres sit apart (offset), and when one is there without the other
//! (timing), and gives a verdict. It reads whatever sheets the game's assets hold, so it runs again after the art is
//! redrawn (`cargo run --release -p arena-game --example spell_hitbox`, `tests/spell_hitbox.rs`).
//!
//! Lengths are the rules' pixels; heights are up from the ground, as `debug::shapes` gives them.

// an offline tool's own measuring, never the rules': floors and roundings in f64 are fine here
#![allow(clippy::disallowed_methods)]

use arena_calc::loadout::{Loadout, Slot};
use arena_duel::data::{UNIT, spell};
use arena_duel::debug::{Drawn as Hit, Mark, Shape, shapes};
use arena_duel::duel::{Duel, DuelConfig, TICK};
use arena_duel::out::Out;
use arena_duel::seat::{Brain, MakeSeat, Script};
use arena_vfx::drawn::{Drawn, school_of};
use arena_vfx::sheet::Index;
use image::RgbaImage;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

/// A box, the rules' pixels, y up: [x0, y0, x1, y1].
pub type Box2 = [f64; 4];

/// A cell of a sheet: whether it is a drawn sheet's, its effect (a drawn sheet's pages are its own), page, x, y, w, h.
type CellKey = (bool, String, u32, u32, u32, u32, u32, u8);

/// Alpha above which a sheet pixel counts as drawn.
const SEEN: u8 = 40;

/// The game's sheets: the drawn ones and their pages, the baked index and its pages.
pub struct Sheets {
    pub drawn: Vec<Drawn>,
    drawn_pages: HashMap<String, Vec<RgbaImage>>,
    pub baked: Index,
    baked_pages: Vec<RgbaImage>,
    /// The drawn box inside a cell, by (page, x, y, w, h) and which sheets: [x0, y0, x1, y1] in cell pixels.
    seen: HashMap<CellKey, Option<[u32; 4]>>,
}

fn page(path: &Path) -> Result<RgbaImage, String> {
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("{}: {e}", path.display()))
}

impl Sheets {
    /// The sheets under the game's `assets` folder (`drawn/`, `vfx/`).
    pub fn load(assets: &Path) -> Result<Sheets, String> {
        let json =
            std::fs::read_to_string(assets.join("drawn/drawn.json")).map_err(|e| e.to_string())?;
        let drawn = arena_vfx::drawn::read(&json)?;
        let mut drawn_pages = HashMap::new();
        for d in &drawn {
            let pages = (0..d.layout.pages.len())
                .map(|i| {
                    let name = if i == 0 {
                        format!("{}.webp", d.file)
                    } else {
                        format!("{}-{i}.webp", d.file)
                    };
                    page(&assets.join("drawn").join(name))
                })
                .collect::<Result<Vec<_>, _>>()?;
            drawn_pages.insert(d.fx.clone(), pages);
        }
        let index =
            std::fs::read_to_string(assets.join("vfx/index.txt")).map_err(|e| e.to_string())?;
        let baked = Index::read(&index)?;
        let baked_pages = baked
            .pages
            .iter()
            .map(|(f, _)| page(&assets.join("vfx").join(f)))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Sheets {
            drawn,
            drawn_pages,
            baked,
            baked_pages,
            seen: HashMap::new(),
        })
    }

    fn drawn_of(&self, fx: &str) -> Option<&Drawn> {
        self.drawn.iter().find(|d| d.fx == fx)
    }

    fn picture(&self, drawn: bool, fx: &str, page: u32) -> Option<&RgbaImage> {
        if drawn {
            self.drawn_pages.get(fx)?.get(page as usize)
        } else {
            self.baked_pages.get(page as usize)
        }
    }

    /// The drawn part of a cell: the box of its pixels above `SEEN`.
    fn seen_in(&mut self, drawn: bool, fx: &str, c: [u32; 5]) -> Option<[u32; 4]> {
        self.seen_above(drawn, fx, c, SEEN)
    }

    /// The box of a cell's pixels whose alpha passes `above`.
    fn seen_above(&mut self, drawn: bool, fx: &str, c: [u32; 5], above: u8) -> Option<[u32; 4]> {
        let key = (
            drawn,
            if drawn { fx.to_string() } else { String::new() },
            c[0],
            c[1],
            c[2],
            c[3],
            c[4],
            above,
        );
        if let Some(b) = self.seen.get(&key) {
            return *b;
        }
        let [p, x, y, w, h] = c;
        let found = self.picture(drawn, fx, p).and_then(|img| {
            let mut b: Option<[u32; 4]> = None;
            for yy in 0..h {
                for xx in 0..w {
                    let (px, py) = (x + xx, y + yy);
                    if px < img.width() && py < img.height() && img.get_pixel(px, py)[3] > above {
                        b = Some(match b {
                            None => [xx, yy, xx + 1, yy + 1],
                            Some([a, bb, cc, d]) => {
                                [a.min(xx), bb.min(yy), cc.max(xx + 1), d.max(yy + 1)]
                            }
                        });
                    }
                }
            }
            b
        });
        self.seen.insert(key, found);
        found
    }
}

/// How one frame of an effect lies: which picture, the part of it, where its centre is (rules px, y up), its scale
/// (rules px a sheet px), its turn (radians, counter-clockwise, y up) and whether it is mirrored.
#[derive(Clone, Copy, Debug)]
pub struct Laid {
    pub drawn: bool,
    pub page: u32,
    pub cell: [u32; 4],
    pub centre: (f64, f64),
    pub scale: f64,
    pub turn: f64,
    pub flip: bool,
}

impl Laid {
    /// Where a point of the cell (cell pixels from its top-left) is, rules px, y up.
    fn at(&self, px: f64, py: f64) -> (f64, f64) {
        let (w, h) = (f64::from(self.cell[2]), f64::from(self.cell[3]));
        let mut lx = (px - w / 2.0) * self.scale;
        let ly = -(py - h / 2.0) * self.scale;
        if self.flip {
            lx = -lx;
        }
        let (s, c) = self.turn.sin_cos();
        (
            self.centre.0 + lx * c - ly * s,
            self.centre.1 + lx * s + ly * c,
        )
    }
}

/// An effect the rules play, as the audit follows it.
#[derive(Clone, Debug)]
struct Playing {
    fx: &'static str,
    born: u32,
    x: f64,
    /// Up from the ground.
    y: f64,
    flip: bool,
    school: &'static str,
    variant: String,
    stopped: Option<u32>,
}

/// One tick of an effect: how its art lies (none when nothing is drawn) and the shape its spell is tested with
/// (none when the rules test nothing for it).
#[derive(Clone, Debug)]
pub struct Sample {
    pub tick: u32,
    pub laid: Vec<Laid>,
    pub art: Option<Box2>,
    pub hit: Option<(Mark, Shape)>,
    /// The point the rules play it at (y up), whether it faces west, and its age, seconds.
    pub at: (f64, f64),
    pub flip: bool,
    pub age: f64,
}

/// One blow against its art, from the point the effect was played at and as it faces east (a west-facing one
/// mirrored): the blow's box, the art's box at its largest, and the art's age when it landed and when it peaked.
#[derive(Clone, Copy, Debug)]
pub struct Blow {
    pub hit: Box2,
    pub art: Box2,
    pub hit_age: f64,
    pub peak_age: f64,
}

/// Box `b` from point `at`, mirrored when `flip`.
fn from_point(b: Box2, at: (f64, f64), flip: bool) -> Box2 {
    let (x0, x1) = if flip {
        (at.0 - b[2], at.0 - b[0])
    } else {
        (b[0] - at.0, b[2] - at.0)
    };
    [x0, b[1] - at.1, x1, b[3] - at.1]
}

/// What an effect showed against what its spell tested, over every playing seen.
#[derive(Clone, Debug, Default)]
pub struct Report {
    pub fx: String,
    /// The spells it plays for.
    pub spells: Vec<String>,
    pub drawn: bool,
    pub playings: usize,
    /// Ticks the art was seen, the shape was there, and both.
    pub art: usize,
    pub hit: usize,
    pub both: usize,
    /// Median of the art's width and height over the shape's (none for a point or a bolt's disc).
    pub scale: Option<(f64, f64)>,
    /// Median distance between the centres, over the shape's half size (a point's over 24 px).
    pub offset: Option<f64>,
    /// For a point or a bolt's disc: the share of ticks its centre lay inside the art.
    pub inside: Option<f64>,
    /// The share of the art's ticks with no shape, and of the shape's ticks with no art.
    pub art_alone: f64,
    pub hit_alone: f64,
    pub verdict: Vec<String>,
    /// One playing's ticks, to draw.
    pub shown: Vec<Sample>,
    /// The skills whose one cast played it.
    pub cast_by: Vec<String>,
    /// The kinds of shape it was matched with (strike, shot, field, unit).
    pub marks: Vec<String>,
    /// A drawn effect no run played.
    pub unplayed: bool,
    /// For a blow's effect: each playing's blow against its art at its largest.
    pub blows: Vec<Blow>,
}

impl Report {
    pub fn ok(&self) -> bool {
        self.verdict.is_empty()
    }

    /// Played, but nothing the rules test goes with it: a decoration.
    pub fn decoration(&self) -> bool {
        !self.unplayed && self.hit == 0
    }
}

/// The tolerances a verdict allows.
pub const SCALE: (f64, f64) = (0.6, 1.7);
pub const OFFSET: f64 = 0.6;
pub const ALONE: f64 = 0.35;
/// How far, seconds, a blow's art may peak from the tick it lands.
pub const LATE: f64 = 0.2;

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    Some(v[v.len() / 2])
}

fn shape_box(s: Shape) -> Box2 {
    match s {
        Shape::Rect { x0, y0, x1, y1 } => [x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1)],
        Shape::Circle { x, y, r } => [x - r, y - r, x + r, y + r],
    }
}

fn centre(b: Box2) -> (f64, f64) {
    ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0)
}

fn union(a: Option<Box2>, b: Box2) -> Box2 {
    match a {
        None => b,
        Some(a) => [
            a[0].min(b[0]),
            a[1].min(b[1]),
            a[2].max(b[2]),
            a[3].max(b[3]),
        ],
    }
}

/// Whether a spell plays `fx`.
fn plays(sp: &arena_duel::data::Spell, fx: &str) -> bool {
    [sp.fx, sp.hit_fx, sp.end_fx].contains(&Some(fx))
}

impl Sheets {
    /// How effect `p` lies at `tick`: one laid cell for a drawn sheet, a baked one's back and front.
    fn lay(&mut self, p: &Playing, tick: u32) -> Vec<Laid> {
        let t = f64::from(tick.saturating_sub(p.born)) * TICK;
        if p.stopped.is_some_and(|s| tick > s) {
            return Vec::new();
        }
        if let Some(d) = self.drawn_of(p.fx) {
            let Some(shot) = d.shot(p.school, t as f32) else {
                return Vec::new();
            };
            let k = f64::from(shot.size * d.px()) * UNIT;
            let c = shot.cell;
            let (cx, cy) = (
                (f64::from(c.dx) + f64::from(c.w) / 2.0) * k,
                -(f64::from(c.dy) + f64::from(c.h) / 2.0) * k,
            );
            let a = -f64::from(shot.angle);
            let (s, co) = a.sin_cos();
            let mut tx = cx * co - cy * s + f64::from(shot.at.0) * UNIT;
            let ty = cx * s + cy * co - f64::from(shot.at.1) * UNIT;
            let turn = if p.flip {
                tx = -tx;
                f64::from(shot.angle)
            } else {
                a
            };
            let anchor_y = if d.layout.motion.ground { 0.0 } else { p.y };
            return vec![Laid {
                drawn: true,
                page: c.page,
                cell: [c.x, c.y, c.w, c.h],
                centre: (p.x + tx, anchor_y + ty),
                scale: k,
                turn,
                flip: p.flip,
            }];
        }
        let Some(v) = self.baked.find(p.fx, &p.variant).cloned() else {
            return Vec::new();
        };
        let n = v.back.len().max(1);
        let mut i = (t * f64::from(v.fps)).floor() as usize;
        if v.looping && i >= n {
            let span = n.saturating_sub(v.restart).max(1);
            i = v.restart + (i - v.restart) % span;
        }
        if i >= n {
            return Vec::new();
        }
        [v.back.get(i), v.front.get(i)]
            .into_iter()
            .flatten()
            .flatten()
            .map(|c| {
                let mut ox = (f64::from(c.dx) + f64::from(c.w) / 2.0) * UNIT;
                if p.flip {
                    ox = -ox;
                }
                Laid {
                    drawn: false,
                    page: c.page,
                    cell: [c.x, c.y, c.w, c.h],
                    centre: (
                        p.x + ox,
                        p.y - (f64::from(c.dy) + f64::from(c.h) / 2.0) * UNIT,
                    ),
                    scale: UNIT,
                    turn: 0.0,
                    flip: p.flip,
                }
            })
            .collect()
    }

    /// The box of the drawn pixels of `laid`, rules px, y up.
    fn art_box(&mut self, fx: &str, laid: &[Laid]) -> Option<Box2> {
        let mut out = None;
        for l in laid {
            let Some(b) = self.seen_in(
                l.drawn,
                fx,
                [l.page, l.cell[0], l.cell[1], l.cell[2], l.cell[3]],
            ) else {
                continue;
            };
            for (px, py) in [(b[0], b[1]), (b[2], b[1]), (b[0], b[3]), (b[2], b[3])] {
                let (x, y) = l.at(f64::from(px), f64::from(py));
                out = Some(union(out, [x, y, x, y]));
            }
        }
        out
    }
}

/// The arena the audit fights in (the server's).
pub fn config(seed: u32) -> DuelConfig {
    DuelConfig {
        seed,
        left: 88.0,
        right: 1192.0,
        ground: 632.0,
        sky: 196.0,
        effects: arena_duel::effects::all(),
    }
}

/// What the runs gathered: every playing's ticks by effect, the spells whose shapes each effect was matched with, the
/// skills whose cast played it, and the kinds of shape it was matched with.
#[derive(Default)]
struct Gathered {
    playings: BTreeMap<&'static str, Vec<Vec<Sample>>>,
    spells: BTreeMap<&'static str, BTreeSet<String>>,
    cast_by: BTreeMap<&'static str, BTreeSet<String>>,
    marks: BTreeMap<&'static str, BTreeSet<&'static str>>,
}

fn mark_name(m: Mark) -> &'static str {
    match m {
        Mark::Strike => "strike",
        Mark::Shot => "shot",
        Mark::Field => "field",
        Mark::Unit => "unit",
        _ => "other",
    }
}

impl Gathered {
    /// Follow `duel` for `ticks`, measuring every effect it plays; `cast`, the skill the run casts, if one.
    fn follow(
        &mut self,
        sheets: &mut Sheets,
        mut duel: Duel,
        ground: f64,
        ticks: u32,
        cast: Option<&str>,
    ) {
        let mut playing: HashMap<u32, (Playing, Vec<Sample>)> = HashMap::new();
        let keep = |g: &mut Gathered, p: Playing, samples: Vec<Sample>| {
            if let Some(c) = cast {
                g.cast_by.entry(p.fx).or_default().insert(c.to_string());
            }
            g.playings.entry(p.fx).or_default().push(samples);
        };
        for tick in 0..ticks {
            for out in duel.step(TICK) {
                match out {
                    Out::Play {
                        key,
                        fx,
                        x,
                        y,
                        opts,
                    } => {
                        let p = Playing {
                            fx,
                            born: tick,
                            x,
                            y: ground - y,
                            flip: opts.flip.unwrap_or(false),
                            school: school_of(opts.palette),
                            variant: arena_vfx::variant_key(&opts),
                            stopped: None,
                        };
                        if let Some((old, samples)) = playing.insert(key, (p, Vec::new())) {
                            keep(self, old, samples);
                        }
                    }
                    Out::Move { key, x, y } => {
                        if let Some((p, _)) = playing.get_mut(&key) {
                            p.x = x;
                            p.y = ground - y;
                        }
                    }
                    Out::Stop { key } => {
                        if let Some((p, _)) = playing.get_mut(&key) {
                            p.stopped.get_or_insert(tick);
                        }
                    }
                    _ => {}
                }
            }
            let hits: Vec<Hit> = shapes(duel.state())
                .into_iter()
                .filter(|h| {
                    matches!(h.mark, Mark::Strike | Mark::Shot | Mark::Field | Mark::Unit)
                        && !h.skill.is_empty()
                })
                .collect();
            let mut done = Vec::new();
            for (key, (p, samples)) in playing.iter_mut() {
                let laid = sheets.lay(p, tick);
                let art = sheets.art_box(p.fx, &laid);
                // the shape of a spell that plays this effect, nearest the art (or the effect's point)
                let at = art.map_or((p.x, p.y), centre);
                let hit = hits
                    .iter()
                    .filter(|h| spell(h.skill).is_some_and(|sp| plays(sp, p.fx)))
                    .min_by(|a, b| {
                        let d = |h: &Hit| {
                            let c = centre(shape_box(h.shape));
                            (c.0 - at.0).hypot(c.1 - at.1)
                        };
                        d(a).total_cmp(&d(b))
                    });
                if let Some(h) = hit {
                    self.spells
                        .entry(p.fx)
                        .or_default()
                        .insert(h.skill.to_string());
                    self.marks
                        .entry(p.fx)
                        .or_default()
                        .insert(mark_name(h.mark));
                }
                if art.is_none() && hit.is_none() && tick > p.born + 2 {
                    done.push(*key);
                } else {
                    samples.push(Sample {
                        tick,
                        laid,
                        art,
                        hit: hit.map(|h| (h.mark, h.shape)),
                        at: (p.x, p.y),
                        flip: p.flip,
                        age: f64::from(tick.saturating_sub(p.born)) * TICK,
                    });
                }
            }
            for k in done {
                if let Some((p, samples)) = playing.remove(&k) {
                    keep(self, p, samples);
                }
            }
        }
        for (_, (p, samples)) in playing {
            keep(self, p, samples);
        }
    }
}

/// A seat that casts `id` until its cast is under way, then stands (`arena-wiki`'s `hits`, as the wiki traces a cast).
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

/// A loadout of `id` padded to a full bar, no supports, no keystones (as the duel's tests and the wiki pad theirs).
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

/// Measure every effect the rules play: each skill of the pool cast once by each seat (so each look's art) at a
/// foe who stands, for `CAST_TICKS`, then `seeds` fights of house brains for `ticks` each.
pub fn audit(sheets: &mut Sheets, seeds: std::ops::Range<u32>, ticks: u32) -> Vec<Report> {
    let mut g = Gathered::default();
    for sp in arena_duel::data::spells() {
        g.cast(sheets, sp.name);
    }
    for seed in seeds {
        let cfg = config(seed);
        let ground = cfg.ground;
        g.follow(
            sheets,
            Duel::new(cfg, [Brain::make(), Brain::make()]),
            ground,
            ticks,
            None,
        );
    }
    g.finish(sheets, true)
}

/// Measure the effects of skills `ids`, each cast once by each seat at a foe who stands (no fights).
pub fn audit_casts(sheets: &mut Sheets, ids: &[&'static str]) -> Vec<Report> {
    let mut g = Gathered::default();
    for id in ids {
        g.cast(sheets, id);
    }
    g.finish(sheets, false)
}

impl Gathered {
    /// Skill `id` cast once by each seat at a foe who stands, followed for `CAST_TICKS`.
    fn cast(&mut self, sheets: &mut Sheets, id: &'static str) {
        for seat in 0..2 {
            let cfg = config(1);
            let ground = cfg.ground;
            let seats = if seat == 0 {
                [caster(id), idle()]
            } else {
                [idle(), caster(id)]
            };
            let loadouts = if seat == 0 {
                [alone(id), alone("palm")]
            } else {
                [alone("palm"), alone(id)]
            };
            if let Ok(duel) = Duel::with_loadouts(cfg, seats, loadouts) {
                self.follow(sheets, duel, ground, CAST_TICKS, Some(id));
            }
        }
    }

    /// The reports, an effect each (with the drawn effects no run played, when `all`), by name.
    fn finish(mut self, sheets: &mut Sheets, all: bool) -> Vec<Report> {
        let mut out: Vec<Report> = std::mem::take(&mut self.playings)
            .into_iter()
            .map(|(fx, playings)| {
                let mut r = report(
                    sheets,
                    fx,
                    playings,
                    self.spells.remove(fx).unwrap_or_default(),
                    self.marks.remove(fx).unwrap_or_default(),
                );
                r.cast_by = self
                    .cast_by
                    .remove(fx)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                r
            })
            .collect();
        if all {
            for d in &sheets.drawn {
                if !out.iter().any(|r| r.fx == d.fx) {
                    out.push(Report {
                        fx: d.fx.clone(),
                        drawn: true,
                        unplayed: true,
                        ..Report::default()
                    });
                }
            }
        }
        out.sort_by(|a, b| a.fx.cmp(&b.fx));
        out
    }
}

/// How long a cast's run is followed, ticks.
pub const CAST_TICKS: u32 = 600;

fn report(
    sheets: &Sheets,
    fx: &str,
    playings: Vec<Vec<Sample>>,
    spells: BTreeSet<String>,
    marks: BTreeSet<&'static str>,
) -> Report {
    let samples: Vec<&Sample> = playings.iter().flatten().collect();
    let art = samples.iter().filter(|s| s.art.is_some()).count();
    let hit = samples.iter().filter(|s| s.hit.is_some()).count();
    // a blow is tested on one tick and its art swells after: it is measured against the art at its largest in each
    // playing, and how long after the blow that comes
    let blow = !marks.is_empty() && marks.iter().all(|m| *m == "strike");
    let mut late: Vec<f64> = Vec::new();
    let mut blows: Vec<Blow> = Vec::new();
    let both: Vec<(Box2, Shape)> = if blow {
        playings
            .iter()
            .filter_map(|p| {
                let struck = p.iter().find(|s| s.hit.is_some())?;
                let shape = struck.hit?.1;
                let area = |b: &Box2| (b[2] - b[0]) * (b[3] - b[1]);
                // its impact: the first frame at four fifths of its largest (a later, larger one is its smoke spreading)
                let most = p
                    .iter()
                    .filter_map(|s| s.art)
                    .map(|b| area(&b))
                    .fold(0.0, f64::max);
                let peak = p
                    .iter()
                    .find(|s| s.art.is_some_and(|b| area(&b) >= 0.8 * most))?;
                let art = peak.art?;
                late.push(peak.age - struck.age);
                blows.push(Blow {
                    hit: from_point(shape_box(shape), struck.at, struck.flip),
                    art: from_point(art, peak.at, peak.flip),
                    hit_age: struck.age,
                    peak_age: peak.age,
                });
                Some((art, shape))
            })
            .collect()
    } else {
        samples
            .iter()
            .filter_map(|s| Some((s.art?, s.hit?.1)))
            .collect()
    };
    // a bolt (a disc, or a point when its motion has no drawn body) is long along its flight in its art, a head and a
    // trail, where the rules test only its head: its thickness is the art's own (`bolt_bodies`), so it is measured by
    // where it is, its centre inside the art, not by the art's box over the disc's
    let point = |s: &Shape| matches!(s, Shape::Circle { .. });
    let areas: Vec<&(Box2, Shape)> = both.iter().filter(|(_, s)| !point(s)).collect();
    let points: Vec<&(Box2, Shape)> = both.iter().filter(|(_, s)| point(s)).collect();
    let scale = (!areas.is_empty()).then(|| {
        let sw = areas.iter().map(|(a, s)| {
            let b = shape_box(*s);
            (a[2] - a[0]) / (b[2] - b[0]).max(1.0)
        });
        let sh = areas.iter().map(|(a, s)| {
            let b = shape_box(*s);
            (a[3] - a[1]) / (b[3] - b[1]).max(1.0)
        });
        (
            median(sw.collect()).unwrap_or(0.0),
            median(sh.collect()).unwrap_or(0.0),
        )
    });
    let offset = median(
        both.iter()
            .map(|(a, s)| {
                let b = shape_box(*s);
                let (ca, cb) = (centre(*a), centre(b));
                let half = ((b[2] - b[0]).max(b[3] - b[1]) / 2.0).max(24.0);
                (ca.0 - cb.0).hypot(ca.1 - cb.1) / half
            })
            .collect(),
    );
    let inside = (!points.is_empty()).then(|| {
        let n = points
            .iter()
            .filter(|(a, s)| {
                let (x, y) = centre(shape_box(*s));
                x >= a[0] - 8.0 && x <= a[2] + 8.0 && y >= a[1] - 8.0 && y <= a[3] + 8.0
            })
            .count();
        n as f64 / points.len() as f64
    });
    let share = |a: usize, of: usize| if of == 0 { 0.0 } else { a as f64 / of as f64 };
    let art_alone = share(
        samples
            .iter()
            .filter(|s| s.art.is_some() && s.hit.is_none())
            .count(),
        art,
    );
    let hit_alone = share(
        samples
            .iter()
            .filter(|s| s.hit.is_some() && s.art.is_none())
            .count(),
        hit,
    );

    let mut verdict = Vec::new();
    if hit == 0 {
        // nothing the rules test plays it: a decoration (a cast's flash, a hit's spark), nothing to compare
    } else if art == 0 {
        verdict.push("luật đang tính trúng mà không thấy hình vẽ".to_string());
    } else {
        if let Some(l) = median(late).filter(|l| l.abs() > LATE) {
            verdict.push(if l > 0.0 {
                format!(
                    "thời gian: hình vẽ to nhất đến sau đòn trúng {:.0} ms",
                    l * 1000.0
                )
            } else {
                format!(
                    "thời gian: hình vẽ to nhất đến trước đòn trúng {:.0} ms",
                    -l * 1000.0
                )
            });
        }
        if let Some((w, h)) = scale {
            for (what, v) in [("bề ngang", w), ("bề cao", h)] {
                if v < SCALE.0 {
                    verdict.push(format!(
                        "hình quá nhỏ: {what} chỉ bằng {:.0}% hình dạng tính trúng",
                        v * 100.0
                    ));
                } else if v > SCALE.1 {
                    verdict.push(format!(
                        "hình quá to: {what} bằng {:.0}% hình dạng tính trúng",
                        v * 100.0
                    ));
                }
            }
        }
        if let Some(o) = offset.filter(|o| *o > OFFSET && inside.is_none()) {
            verdict.push(format!("lệch: hai tâm cách nhau {o:.1} lần nửa cỡ"));
        }
        if let Some(i) = inside.filter(|i| *i < 0.7) {
            verdict.push(format!(
                "điểm tính trúng nằm ngoài hình vẽ {:.0}% thời gian",
                (1.0 - i) * 100.0
            ));
        }
        if hit_alone > ALONE {
            verdict.push(format!(
                "thời gian: {:.0}% lúc luật tính trúng mà không vẽ gì",
                hit_alone * 100.0
            ));
        }
        // a blow is tested on the tick it lands and its art plays on after: only a shape that lasts (a shot, a field, a
        // unit) is out of time when drawn without it
        let lasting = marks
            .iter()
            .any(|m| matches!(*m, "shot" | "field" | "unit"));
        if lasting && art_alone > ALONE {
            verdict.push(format!(
                "thời gian: {:.0}% lúc có vẽ mà luật không tính trúng gì",
                art_alone * 100.0
            ));
        }
    }
    // the playing to show: the longest with both art and a shape
    let shown = playings
        .iter()
        .max_by_key(|p| {
            p.iter()
                .filter(|s| s.art.is_some() && s.hit.is_some())
                .count()
        })
        .cloned()
        .unwrap_or_default();
    Report {
        fx: fx.to_string(),
        spells: spells.into_iter().collect(),
        drawn: sheets.drawn_of(fx).is_some(),
        playings: playings.len(),
        art,
        hit,
        both: both.len(),
        scale,
        offset,
        inside,
        art_alone,
        hit_alone,
        verdict,
        shown,
        marks: marks.into_iter().map(str::to_string).collect(),
        blows,
        ..Report::default()
    }
}

/// Draw up to `n` ticks of a report's playing side by side: the art as the game places it, the drawn box (grey),
/// the rules' shape (red), the ground (white). One image pixel a rules pixel over `zoom`.
pub fn picture(sheets: &Sheets, r: &Report, n: usize, zoom: f64) -> Option<RgbaImage> {
    let frames: Vec<&Sample> = {
        let s = &r.shown;
        if s.is_empty() {
            return None;
        }
        let step = (s.len() as f64 / n as f64).max(1.0);
        (0..n)
            .filter_map(|i| s.get((i as f64 * step) as usize))
            .collect()
    };
    // each tick in its own crop round its art and its shape (a bolt flies far between them), all one size
    let pad = 24.0;
    let crops: Vec<Box2> = frames
        .iter()
        .filter_map(|s| {
            let mut b = s.art;
            if let Some((_, h)) = s.hit {
                b = Some(union(b, shape_box(h)));
            }
            b.map(|b| [b[0] - pad, b[1] - pad, b[2] + pad, b[3] + pad])
        })
        .collect();
    if crops.is_empty() {
        return None;
    }
    let span = |f: fn(&Box2) -> f64| crops.iter().map(f).fold(0.0, f64::max);
    let (cw, ch) = (span(|b| b[2] - b[0]), span(|b| b[3] - b[1]));
    let (w, h) = (
        ((cw / zoom).ceil() as u32).clamp(16, 700),
        ((ch / zoom).ceil() as u32).clamp(16, 700),
    );
    let frames: Vec<(&Sample, Box2)> = frames
        .into_iter()
        .zip(crops.iter().map(|b| {
            let (cx, cy) = centre(*b);
            let (hw, hh) = (f64::from(w) * zoom / 2.0, f64::from(h) * zoom / 2.0);
            [cx - hw, cy - hh, cx + hw, cy + hh]
        }))
        .collect();
    let gap = 6;
    let mut img = RgbaImage::from_pixel(
        (w + gap) * frames.len() as u32,
        h,
        image::Rgba([18, 14, 28, 255]),
    );
    for (i, (s, b)) in frames.iter().enumerate() {
        let (s, b) = (*s, *b);
        let ox = (w + gap) * i as u32;
        let to_px = |x: f64, y: f64| -> (i64, i64) {
            (
                ((x - b[0]) / zoom) as i64 + i64::from(ox),
                ((b[3] - y) / zoom) as i64,
            )
        };
        let put =
            |img: &mut RgbaImage, x: i64, y: i64, c: [u8; 4]| put_px(img, (ox, w, h), x, y, c);
        // the ground
        let (_, gy) = to_px(0.0, 0.0);
        for x in ox..ox + w {
            put(&mut img, i64::from(x), gy, [200, 200, 210, 160]);
        }
        // the art: each image pixel looked up in its cell
        for l in &s.laid {
            let Some(pic) = sheets.picture(l.drawn, &r.fx, l.page) else {
                continue;
            };
            let (cw, ch) = (f64::from(l.cell[2]), f64::from(l.cell[3]));
            for yy in 0..h {
                for xx in 0..w {
                    let (x, y) = (
                        b[0] + (f64::from(xx) + 0.5) * zoom,
                        b[3] - (f64::from(yy) + 0.5) * zoom,
                    );
                    // into the cell: undo the turn, the mirror and the scale
                    let (dx, dy) = (x - l.centre.0, y - l.centre.1);
                    let (sn, cs) = (-l.turn).sin_cos();
                    let (mut lx, ly) = (dx * cs - dy * sn, dx * sn + dy * cs);
                    if l.flip {
                        lx = -lx;
                    }
                    let (px, py) = (lx / l.scale + cw / 2.0, -ly / l.scale + ch / 2.0);
                    if px < 0.0 || py < 0.0 || px >= cw || py >= ch {
                        continue;
                    }
                    let (sx, sy) = (l.cell[0] + px as u32, l.cell[1] + py as u32);
                    if sx < pic.width() && sy < pic.height() {
                        let c = pic.get_pixel(sx, sy).0;
                        put(&mut img, i64::from(ox + xx), i64::from(yy), c);
                    }
                }
            }
        }
        let outline = |img: &mut RgbaImage, bx: Box2, c: [u8; 4], dashed: bool| {
            let (x0, y1) = to_px(bx[0], bx[1]);
            let (x1, y0) = to_px(bx[2], bx[3]);
            for x in x0..=x1 {
                if !dashed || (x / 4) % 2 == 0 {
                    put(img, x, y0, c);
                    put(img, x, y1, c);
                }
            }
            for y in y0..=y1 {
                if !dashed || (y / 4) % 2 == 0 {
                    put(img, x0, y, c);
                    put(img, x1, y, c);
                }
            }
        };
        if let Some(a) = s.art {
            outline(&mut img, a, [170, 170, 190, 220], true);
        }
        if let Some((_, shape)) = s.hit {
            match shape {
                Shape::Circle { x, y, r } if r > 0.0 => {
                    let steps = 96;
                    for k in 0..steps {
                        let a = k as f64 / f64::from(steps) * std::f64::consts::TAU;
                        let (px, py) = to_px(x + r * a.cos(), y + r * a.sin());
                        put(&mut img, px, py, [255, 50, 50, 255]);
                    }
                }
                Shape::Circle { x, y, .. } => {
                    let (px, py) = to_px(x, y);
                    for d in -3..=3 {
                        put(&mut img, px + d, py, [255, 50, 50, 255]);
                        put(&mut img, px, py + d, [255, 50, 50, 255]);
                    }
                }
                Shape::Rect { .. } => {
                    outline(&mut img, shape_box(shape), [255, 50, 50, 255], false)
                }
            }
        }
    }
    Some(img)
}

/// Blend colour `c` over pixel (`x`, `y`) of `img`, inside the frame `(left, width, height)`.
fn put_px(img: &mut RgbaImage, frame: (u32, u32, u32), x: i64, y: i64, c: [u8; 4]) {
    let (ox, w, h) = frame;
    if x >= i64::from(ox) && x < i64::from(ox + w) && y >= 0 && y < i64::from(h) {
        let p = img.get_pixel_mut(x as u32, y as u32);
        let a = f64::from(c[3]) / 255.0;
        for k in 0..3 {
            p[k] = (f64::from(p[k]) * (1.0 - a) + f64::from(c[k]) * a) as u8;
        }
    }
}

impl Sheets {
    /// How thick a cell's body is across a flight drawn east, in cell pixels: for each column the count of its pixels
    /// above `BODY`, and of those counts the 90th percentile (the head, not a thin trail or a zigzag spark).
    fn thickness(&self, drawn: bool, fx: &str, c: [u32; 5]) -> Option<f64> {
        let [p, x, y, w, h] = c;
        let img = self.picture(drawn, fx, p)?;
        let mut cols: Vec<f64> = (0..w)
            .map(|xx| {
                (0..h)
                    .filter(|yy| {
                        let (px, py) = (x + xx, y + yy);
                        px < img.width() && py < img.height() && img.get_pixel(px, py)[3] > BODY
                    })
                    .count() as f64
            })
            .filter(|n| *n > 0.0)
            .collect();
        if cols.is_empty() {
            return None;
        }
        cols.sort_by(f64::total_cmp);
        Some(cols[(cols.len() - 1) * 9 / 10])
    }
}

/// Alpha a bolt's body must have: its soft glow is not the body the rules should test.
pub const BODY: u8 = 127;

/// Each bolt motion's drawn body: half its thickness across its flight (its sheets are drawn flying east), the rules'
/// pixels: the median over its frames of half its head's thickness (`Sheets::thickness`: the 90th percentile of its
/// columns' solid pixels above `BODY`), the thicker of its schools (0 for a motion whose sheet draws nothing that
/// solid). Sorted by name, as the rules search it (`bolts_data.rs`).
pub fn bolt_bodies(sheets: &mut Sheets) -> Vec<(&'static str, f64)> {
    let mut out = Vec::new();
    for sp in arena_duel::data::spells()
        .iter()
        .filter(|s| s.kind == arena_duel::data::Kind::Bolt)
    {
        let Some(fx) = sp.fx else {
            out.push((sp.name, 0.0));
            continue;
        };
        let mut best: f64 = 0.0;
        if let Some(d) = sheets.drawn_of(fx).cloned() {
            let k = f64::from(d.px()) * UNIT;
            for cells in d.layout.clips.values() {
                let halves: Vec<f64> = cells
                    .iter()
                    .filter_map(|c| sheets.thickness(true, fx, [c.page, c.x, c.y, c.w, c.h]))
                    .map(|t| t / 2.0 * k)
                    .collect();
                best = best.max(median(halves).unwrap_or(0.0));
            }
        } else if let Some(v) = sheets.baked.find(fx, "").cloned() {
            let halves: Vec<f64> = v
                .back
                .iter()
                .chain(&v.front)
                .flatten()
                .filter_map(|c| sheets.thickness(false, fx, [c.page, c.x, c.y, c.w, c.h]))
                .map(|t| t / 2.0 * UNIT)
                .collect();
            best = median(halves).unwrap_or(0.0);
        }
        out.push((sp.name, (best * 10.0).round() / 10.0));
    }
    out.sort_by(|a, b| a.0.cmp(b.0));
    out
}

/// `bolts_data.rs`: the table of `bolt_bodies`, as the rules read it.
pub fn bolts_data(bodies: &[(&str, f64)]) -> String {
    let mut s = String::from(
        "//! @generated by `cargo run --release -p arena-game --example spell_hitbox -- <out dir> --bolts crates/arena-duel/src/bolts_data.rs`\n\
         //! from the effect sheets (`arena_game::audit::bolt_bodies`): each bolt motion's drawn body, half its thickness\n\
         //! across its flight, in the rules' pixels, sorted by name. Run it again after a bolt's art changes.\n\n\
         #[rustfmt::skip]\n\
         pub static BOLT_BODIES: &[(&str, f64)] = &[\n",
    );
    for (name, r) in bodies {
        s += &format!("    ({name:?}, {r:.1}),\n");
    }
    s += "];\n";
    s
}
