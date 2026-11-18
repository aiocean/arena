//! The arena's wiki, generated from its code: every skill of the shared pool with the clip it plays in each look
//! (its frames, the rules' boxes, its wind-up, active window and follow-through), its effect's sheet, its numbers and
//! the supports that fit it; the supports, ailments, auras, stances, domains, curses, keystones, roots, units, the
//! fighters' looks, an overview of the rules and the bot's view. `site/build.sh` writes it into `dist/wiki/` on every
//! build, so it cannot drift from the rules. The pictures are drawn here from the game's sheets (`img`), the words are
//! the code's own (`desc`); where a skill has none, a sentence is made from its data and marked so.

// nothing here decides the fight (it only draws it): std's float rounding is fine
#![allow(clippy::disallowed_methods)]

pub mod desc;
pub mod html;
pub mod img;

use arena_calc::ailment::{Ailment, ForceAilment};
use arena_calc::aura::AuraKind;
use arena_calc::cap;
use arena_calc::domain::DomainKind;
use arena_calc::element::Element;
use arena_calc::gem::{self, Effect, GEMS, Gem};
use arena_calc::grade::Rank;
use arena_calc::keystone::Keystone;
use arena_calc::support::SUPPORTS;
use arena_calc::tag::{Tag, Tags};
use arena_duel::data::{self, Kind, Look, Spell};
use arena_duel::kit::Way;
use html::{esc, facts, num, page, table};
use image::RgbaImage;
use img::{Marks, Phase, Pic};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The wiki's files, by their path under `wiki/`: pages, its styles and its pictures (`img/<hash>.webp`).
#[derive(Default)]
pub struct Wiki {
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Wiki {
    /// Every file under `out` (made if missing).
    pub fn write(&self, out: &Path) -> Result<(), String> {
        for (path, bytes) in &self.files {
            let p = out.join(path);
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            std::fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        Ok(())
    }

    fn page(&mut self, path: &str, html: String) {
        self.files.insert(path.to_owned(), html.into_bytes());
    }

    /// A picture under `img/`, named by its content: its URL.
    fn picture(&mut self, bytes: Vec<u8>) -> String {
        let name = format!("img/{}.webp", img::name_of(&bytes));
        let url = format!("/wiki/{name}");
        self.files.insert(name, bytes);
        url
    }
}

/// The looks, in seat order.
pub fn looks() -> [Look; 2] {
    [Look::of_seat(0), Look::of_seat(1)]
}

/// A look's sheet: its file's stem and its school (`crates/arena-game/src/fighters.rs` pairs them the same way).
fn sheet_of(look: Look) -> (&'static str, &'static str) {
    match look {
        Look::VuongLam => ("cultivator", "thunder"),
        Look::ThatThai => ("seven-colored-daoist", "frost"),
    }
}

/// A fighter's sheet as `assets/fighters/<stem>.json` lays it out.
#[derive(Deserialize)]
struct SheetJson {
    clips: BTreeMap<String, Vec<CellJson>>,
    pages: Vec<[u32; 2]>,
}

#[derive(Deserialize, Clone, Copy)]
struct CellJson {
    #[serde(default)]
    page: u32,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    dx: i32,
    dy: i32,
}

impl From<CellJson> for Pic {
    fn from(c: CellJson) -> Pic {
        Pic {
            page: c.page,
            x: c.x,
            y: c.y,
            w: c.w,
            h: c.h,
            dx: c.dx,
            dy: c.dy,
        }
    }
}

/// A look's sheet, read: its clips' cells and its pages.
struct Sheet {
    clips: BTreeMap<String, Vec<Pic>>,
    pages: Vec<RgbaImage>,
}

fn read_sheet(assets: &Path, look: Look) -> Result<Sheet, String> {
    let (stem, school) = sheet_of(look);
    let path = assets.join(format!("fighters/{stem}.json"));
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let json: SheetJson =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut pages = Vec::new();
    for i in 0..json.pages.len() {
        let name = if i == 0 {
            format!("fighters/{stem}-{school}.png")
        } else {
            format!("fighters/{stem}-{school}-{i}.png")
        };
        pages.push(img::page(&assets.join(name))?);
    }
    let clips = json
        .clips
        .into_iter()
        .map(|(k, v)| (k, v.into_iter().map(Pic::from).collect()))
        .collect();
    Ok(Sheet { clips, pages })
}

/// The clip a skill of `spell` plays: a blade's `slash` (`overhead` from the air), a lunge's `dash`, the rest `cast`
/// (`frames::act_clip`, `pick`).
pub fn clip_of(spell: &Spell) -> &'static str {
    match spell.kind {
        Kind::Melee if spell.dive => "overhead",
        Kind::Melee => "slash",
        Kind::Dash => "dash",
        _ => "cast",
    }
}

/// A clip's pictures in one look: the whole clip with its boxes and phases, and its active frames close up.
#[derive(Clone, Debug, Default)]
struct ClipPics {
    whole: Option<(String, u32, u32)>,
    active: Option<(String, u32, u32)>,
    /// (frames, active window) as the rules have them.
    frames: usize,
    window: Option<(usize, usize)>,
}

/// An effect's pictures: (school or variant, url, w, h).
type FxPics = Vec<(String, String, u32, u32)>;

/// Everything read once and shared by the pages.
struct Book {
    sheets: Vec<(Look, Sheet)>,
    clips: BTreeMap<(usize, String), ClipPics>,
    fx: BTreeMap<String, FxPics>,
    skill_words: BTreeMap<String, desc::SkillWords>,
    support_words: BTreeMap<String, desc::SupportWords>,
}

/// The wiki from the repository at `root`.
pub fn build(root: &Path) -> Result<Wiki, String> {
    let assets = root.join("crates/arena-game/assets");
    let mut wiki = Wiki::default();
    wiki.files
        .insert("wiki.css".to_owned(), html::CSS.as_bytes().to_vec());
    let read =
        |p: PathBuf| std::fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()));
    let mut book = Book {
        sheets: Vec::new(),
        clips: BTreeMap::new(),
        fx: BTreeMap::new(),
        skill_words: desc::skills(&read(root.join("crates/arena-calc/src/gem.rs"))?),
        support_words: desc::supports(&read(
            root.join("docs/specs/2026-11-17-damage-system-design.md"),
        )?),
    };
    for look in looks() {
        book.sheets.push((look, read_sheet(&assets, look)?));
    }
    // every clip of every look, drawn once
    for li in 0..book.sheets.len() {
        let names: Vec<String> = book.sheets[li].1.clips.keys().cloned().collect();
        for clip in names {
            let pics = clip_pics(&mut wiki, &book.sheets[li], &clip);
            book.clips.insert((li, clip), pics);
        }
    }
    effects(&mut wiki, &mut book, &assets)?;
    skills(&mut wiki, &book);
    index(&mut wiki);
    supports(&mut wiki, &book);
    ailments(&mut wiki);
    buffs(&mut wiki);
    curses(&mut wiki);
    keystones(&mut wiki);
    units(&mut wiki);
    looks_page(&mut wiki, &book);
    rules(&mut wiki);
    bot(&mut wiki);
    Ok(wiki)
}

fn clip_pics(wiki: &mut Wiki, (look, sheet): &(Look, Sheet), clip: &str) -> ClipPics {
    let cells = sheet.clips.get(clip).cloned().unwrap_or_default();
    let (window, frames) = arena_duel::frames::clip_of(*look, clip).unwrap_or((None, &[]));
    let n = cells.len().min(frames.len().max(cells.len()));
    let marks: Vec<Marks> = (0..n)
        .map(|i| {
            let f = frames.get(i);
            Marks {
                hurt: f.map(|f| f.hurt.to_vec()).unwrap_or_default(),
                hit: f.and_then(|f| f.hit),
                hand: f.and_then(|f| f.hand),
                phase: window.map(|(s, e)| {
                    if i < s {
                        Phase::Windup
                    } else if i <= e {
                        Phase::Active
                    } else {
                        Phase::Follow
                    }
                }),
            }
        })
        .collect();
    let mut out = ClipPics {
        frames: frames.len(),
        window,
        ..ClipPics::default()
    };
    if let Some(strip) = img::strip(&sheet.pages, &cells[..n], &marks, 1.0, false) {
        let (w, h) = strip.dimensions();
        out.whole = Some((wiki.picture(img::webp(&strip, true)), w, h));
    }
    if let Some((s, e)) = window {
        let e = e.min(n.saturating_sub(1));
        if s <= e {
            let marks: Vec<Marks> = marks[s..=e]
                .iter()
                .map(|m| Marks {
                    phase: None,
                    ..m.clone()
                })
                .collect();
            if let Some(strip) = img::strip(&sheet.pages, &cells[s..=e], &marks, 2.0, false) {
                let (w, h) = strip.dimensions();
                out.active = Some((wiki.picture(img::webp(&strip, true)), w, h));
            }
        }
    }
    out
}

/// Every effect's frames: a drawn sheet (`assets/drawn`), else a baked one (`assets/vfx`).
fn effects(wiki: &mut Wiki, book: &mut Book, assets: &Path) -> Result<(), String> {
    let text = |p: &str| std::fs::read_to_string(assets.join(p)).map_err(|e| format!("{p}: {e}"));
    for d in arena_vfx::drawn::read(&text("drawn/drawn.json")?)? {
        let mut pages = Vec::new();
        for i in 0..d.layout.pages.len() {
            let name = if i == 0 {
                format!("drawn/{}.webp", d.file)
            } else {
                format!("drawn/{}-{i}.webp", d.file)
            };
            pages.push(img::page(&assets.join(name))?);
        }
        let mut shown = Vec::new();
        for (school, cells) in &d.layout.clips {
            let pics: Vec<Pic> = cells
                .iter()
                .map(|c| Pic {
                    page: c.page,
                    x: c.x,
                    y: c.y,
                    w: c.w,
                    h: c.h,
                    dx: c.dx,
                    dy: c.dy,
                })
                .collect();
            let tallest = pics.iter().map(|p| p.h).max().unwrap_or(1).max(1);
            let scale = (110.0 / f64::from(tallest)).min(1.0);
            if let Some(strip) = img::strip(&pages, &pics, &[], scale, true) {
                let (w, h) = strip.dimensions();
                shown.push((
                    format!("drawn, {school}"),
                    wiki.picture(img::webp(&strip, false)),
                    w,
                    h,
                ));
            }
        }
        book.fx.insert(d.fx.clone(), shown);
    }
    let index = arena_vfx::sheet::Index::read(&text("vfx/index.txt")?)?;
    let mut pages = Vec::new();
    for (file, _) in &index.pages {
        pages.push(img::page(&assets.join("vfx").join(file))?);
    }
    for v in &index.variants {
        if book
            .fx
            .get(&v.fx)
            .is_some_and(|s| s.iter().any(|(k, ..)| k.starts_with("drawn")))
        {
            continue;
        }
        let pics: Vec<Pic> = v
            .front
            .iter()
            .chain(v.back.iter())
            .flatten()
            .map(|c| Pic {
                page: c.page,
                x: c.x,
                y: c.y,
                w: c.w,
                h: c.h,
                dx: c.dx,
                dy: c.dy,
            })
            .take(24)
            .collect();
        if let Some(strip) = img::strip(&pages, &pics, &[], 1.0, false) {
            let (w, h) = strip.dimensions();
            let label = if v.key.is_empty() {
                "baked".to_owned()
            } else {
                format!("baked, {}", v.key)
            };
            let url = wiki.picture(img::webp(&strip, true));
            book.fx
                .entry(v.fx.clone())
                .or_default()
                .push((label, url, w, h));
        }
    }
    Ok(())
}

fn rank_name(r: Rank) -> &'static str {
    match r {
        Rank::Art => "Thuật (Art)",
        Rank::Divine => "Thần thông (Divine)",
        Rank::Rule => "Quy tắc (Rule)",
        Rank::Origin => "Bản nguyên (Origin)",
        Rank::Faith => "Tín niệm (Faith)",
    }
}

fn tier_name(g: &Gem) -> String {
    format!("{:?}", g.tier)
}

fn skill_url(id: &str) -> String {
    format!("/wiki/skill/{id}.html")
}

fn skill_link(g: &Gem) -> String {
    format!("<a href=\"{}\">{}</a>", skill_url(g.id), esc(g.name))
}

fn tag_chips(tags: Tags) -> String {
    Tag::ALL
        .iter()
        .filter(|t| tags.has(**t))
        .map(|t| {
            format!(
                "<a class=\"tag\" href=\"/wiki/index.html#tag={}\">{}</a>",
                esc(&format!("{t:?}")),
                esc(t.name())
            )
        })
        .collect()
}

fn element_link(e: Element) -> String {
    format!(
        "<a href=\"/wiki/ailments.html#el-{e:?}\">{}</a>",
        esc(e.name())
    )
}

fn ailment_link(a: Ailment) -> String {
    format!(
        "<a href=\"/wiki/ailments.html#ail-{a:?}\">{}</a>",
        esc(a.name())
    )
}

/// The ailments a skill's blows can lay, one per kind of damage it deals.
fn ailments_of(g: &Gem) -> Vec<Ailment> {
    let mut out: Vec<Ailment> = Vec::new();
    if g.deals_damage() {
        for &(e, _) in g.split {
            let a = Ailment::of(e, g.force);
            if !out.contains(&a) {
                out.push(a);
            }
        }
    }
    out
}

fn way_of(g: &Gem) -> Way {
    Way::of(g, g.tags())
}

fn way_html(g: &Gem) -> String {
    match way_of(g) {
        Way::Aura => match gem::aura_of(g) {
            Some(a) => format!("an aura: <a href=\"/wiki/buffs.html#aura-{a:?}\">{}</a>", esc(a.data().name)),
            None => "an aura".to_owned(),
        },
        Way::Domain { damaging } => {
            let d = gem::domain_of(g).map_or(String::new(), |d| {
                format!(" <a href=\"/wiki/buffs.html#domain-{d:?}\">{}</a>", esc(d.data().name))
            });
            format!("a domain{d}{}", if damaging { ", which also strikes" } else { "" })
        }
        Way::Curse => match gem::curse_of(g) {
            Some(c) => format!("a curse: <a href=\"/wiki/curses.html#curse-{c:?}\">{}</a>", esc(c.name())),
            None => "a curse".to_owned(),
        },
        Way::Buff => "a timed buff on the caster (<a href=\"/wiki/buffs.html#stances\">stances and blessings</a>)".to_owned(),
        Way::Device(k) => format!("it puts a <a href=\"/wiki/units.html#unit-{k:?}\">{k:?}</a> on the field"),
        _ => "as its motion strikes".to_owned(),
    }
}

fn cost_html(g: &Gem) -> String {
    g.costs
        .iter()
        .map(|(r, v)| format!("{} {:?}", num(*v), r))
        .collect::<Vec<_>>()
        .join(", ")
}

fn split_html(g: &Gem) -> String {
    g.split
        .iter()
        .map(|&(e, s)| format!("{} {}%", element_link(e), num(s * 100.0)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A sentence from a skill's data, for a skill the code has no words for.
fn generated(g: &Gem, sp: Option<&Spell>) -> String {
    let what = sp.map_or("skill".to_owned(), |s| format!("{} skill", s.kind.name()));
    let kinds = g
        .split
        .iter()
        .map(|&(e, _)| e.name())
        .collect::<Vec<_>>()
        .join(" and ");
    let mut s = format!("A {what}");
    if g.deals_damage() && !kinds.is_empty() {
        let _ = write!(s, " that deals {kinds} damage");
    }
    let cost = cost_html(g);
    if !cost.is_empty() {
        let _ = write!(s, ", costing {cost}");
    }
    let _ = write!(s, ", with a {} s cooldown.", num(g.cooldown));
    s
}

/// One bare cast's damage against the one-cast cap (`arena_calc::cap`), for a skill that deals damage.
fn cap_row(g: &Gem) -> String {
    if !cap::damaging(g) {
        return "deals no damage".to_owned();
    }
    let of = cap::cap_life();
    let d = cap::cast_of(g, &[]);
    format!(
        "{}% of a life and its shield ({} at hit + {} over time = {} of {}); the cap is {}%{}",
        num((d.share(of) * 1000.0).round() / 10.0),
        num(d.hit.round()),
        num(d.over_time.round()),
        num(d.total().round()),
        num(of),
        num(cap::CAST_CAP * 100.0),
        if d.over(of) { ", <b>over it</b>" } else { "" }
    )
}

/// One cast with each support that fits it, against the cap: measured, not held to it.
fn cap_forms(g: &Gem) -> String {
    if !cap::damaging(g) {
        return String::new();
    }
    let of = cap::cap_life();
    let mut forms: Vec<(&str, &str, cap::Dealt)> = cap::fitting(&cap::skill_of(g))
        .into_iter()
        .map(|s| (s.id, s.name, cap::cast_of(g, &[s])))
        .collect();
    if forms.is_empty() {
        return String::new();
    }
    forms.sort_by(|a, b| b.2.total().total_cmp(&a.2.total()));
    let rows: Vec<Vec<String>> = forms
        .iter()
        .map(|(id, name, d)| {
            vec![
                format!(
                    "<a href=\"/wiki/supports.html#{}\">{}</a>",
                    esc(id),
                    esc(name)
                ),
                num(d.total().round()),
                format!(
                    "{}%{}",
                    num((d.share(of) * 1000.0).round() / 10.0),
                    if d.over(of) { " (over)" } else { "" }
                ),
            ]
        })
        .collect();
    format!(
        "<h2>One cast with a support</h2><p class=\"gen\">One cast with each support that fits it, against the one-cast cap ({}% of {}). Measured, not held to the cap: the rule holds the bare skill.</p>{}",
        num(cap::CAST_CAP * 100.0),
        num(of),
        table(&["Support", "Damage", "Share"], &rows)
    )
}

fn spell_numbers(sp: &Spell) -> Vec<(&'static str, String)> {
    let opt = |v: Option<f64>, unit: &str| v.map_or(String::new(), |v| format!("{}{unit}", num(v)));
    vec![
        ("Range", opt(sp.range, " px")),
        ("Reach", opt(sp.reach, " px")),
        ("Radius", opt(sp.radius, " px")),
        ("Distance", opt(sp.distance, " px")),
        ("Height", opt(sp.height, " px")),
        ("Speed", opt(sp.speed, " px/s")),
        ("Damage a blow (rules)", opt(sp.damage, "")),
        ("Strikes", opt(sp.strikes, "")),
        ("Every", opt(sp.every, " s")),
        ("Tick", opt(sp.tick, " s")),
        ("Lasts", opt(sp.life, " s")),
        ("Heals", opt(sp.amount, "")),
        ("Pull", opt(sp.pull, "")),
    ]
}

fn timeline(pics: &ClipPics, sp: &Spell) -> String {
    let Some((s, e)) = pics.window else {
        return format!(
            "{} frames; the clip has no active window. Wind-up {} s, recovery {} s.",
            pics.frames,
            num(sp.windup),
            num(sp.recover)
        );
    };
    let last = pics.frames.saturating_sub(1);
    format!(
        "Wind-up: frames 0 to {} played over {} s. Active window: frames {s} to {e}. Follow-through: frames {} to {last} played over the {} s recovery (by progress).",
        s.saturating_sub(1),
        num(sp.windup),
        (e + 1).min(last),
        num(sp.recover)
    )
}

const LEGEND: &str = "<p class=\"legend gen\">Boxes: <span style=\"background:#5ab4ff\"></span>hurtbox <span style=\"background:#ff4646\"></span>hitbox <span style=\"background:#ffdc3c\"></span>casting hand. Bar: <span style=\"background:#9696a5\"></span>wind-up <span style=\"background:#ff4646\"></span>active <span style=\"background:#f0aa3c\"></span>follow-through.</p>";

fn strip_html(src: &(String, u32, u32), alt: &str, pixel: bool) -> String {
    format!(
        "<div class=\"strip\">{}</div>",
        html::img(&src.0, src.1, src.2, alt, pixel)
    )
}

fn skills(wiki: &mut Wiki, book: &Book) {
    for g in GEMS {
        let sp = data::spell(g.id);
        let words = book.skill_words.get(g.id);
        let mut body = format!("<h1>{} <code>{}</code></h1>", esc(g.name), esc(g.id));
        match words.and_then(|w| w.comment.as_deref()) {
            Some(c) => {
                let _ = write!(body, "<p>{}</p>", esc(c));
            }
            None => {
                let _ = write!(
                    body,
                    "<p>{} <span class=\"gen\">(generated from its data: the code has no words for this skill)</span></p>",
                    generated(g, sp)
                );
            }
        }
        let ailments = ailments_of(g)
            .into_iter()
            .map(ailment_link)
            .collect::<Vec<_>>()
            .join(", ");
        let effects = g
            .effects
            .iter()
            .map(|e| match e {
                Effect::Control(c, s) => format!("{c:?} for {} s", num(*s)),
                Effect::Ailment(v) => format!("ailment build-up {}", num(*v)),
                Effect::Hits(n) => format!("{} hits a cast", num(*n)),
                Effect::Heal(v) => format!("heals {}% of life", num(*v)),
                Effect::Expose(v) => format!("exposes {}%", num(v * 100.0)),
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mut rows: Vec<(&str, String)> = vec![
            (
                "Section of the pool",
                words.map_or(String::new(), |w| esc(&w.section)),
            ),
            (
                "Kind",
                sp.map_or(String::new(), |s| {
                    format!("{} ({})", esc(s.kind.name()), way_html(g))
                }),
            ),
            (
                "Rank",
                format!("{} · level {}", rank_name(g.rank), g.rank.level()),
            ),
            ("Tier", tier_name(g)),
            ("Tags", tag_chips(g.all_tags())),
            ("Damage kinds", split_html(g)),
            ("Ailments it lays", ailments),
            (
                "Blade or palm",
                match g.force {
                    ForceAilment::Bleed => "cuts (bleed)".to_owned(),
                    ForceAilment::Daze => "shakes (daze)".to_owned(),
                },
            ),
            ("Effects", effects),
            ("Cost", cost_html(g)),
            ("Cooldown", format!("{} s", num(g.cooldown))),
            ("Wind-up", format!("{} s", num(g.windup))),
            ("Recovery", format!("{} s", num(g.recover))),
            ("Uses", g.uses.map_or(String::new(), |u| u.to_string())),
            ("Base", num(g.base)),
            ("Expected damage a cast", num(gem::expected_damage(g))),
            ("Power score", num(gem::power_score(g))),
            (
                "Binds the caster",
                if g.binds() {
                    "yes: no other binding skill while it lasts".to_owned()
                } else {
                    String::new()
                },
            ),
            (
                "Restriction",
                if g.restriction {
                    "yes".to_owned()
                } else {
                    String::new()
                },
            ),
            ("Motion borrowed", format!("<code>{}</code>", esc(g.spell))),
            ("Source", format!("<code>{}</code>", esc(g.source))),
            ("One-cast cap", cap_row(g)),
        ];
        if let Some(sp) = sp {
            rows.extend(spell_numbers(sp));
        }
        body.push_str(&facts(&rows));
        if let Some(sp) = sp {
            let clip = clip_of(sp);
            let _ = write!(
                body,
                "<h2>The clip it plays: <code>{clip}</code></h2><p>Every fighter carries the whole pool; this is how each look plays it (<a href=\"/wiki/looks.html#clip-{clip}\">all clips</a>).</p>{LEGEND}"
            );
            for (li, (look, _)) in book.sheets.iter().enumerate() {
                let Some(pics) = book.clips.get(&(li, clip.to_owned())) else {
                    continue;
                };
                let _ = write!(
                    body,
                    "<h3><a href=\"/wiki/looks.html#look-{}\">{}</a></h3><p>{}</p>",
                    look.name(),
                    esc(look.display()),
                    timeline(pics, sp)
                );
                if let Some(w) = &pics.whole {
                    body.push_str(&strip_html(
                        w,
                        &format!("{clip} in {}", look.display()),
                        true,
                    ));
                }
                if let Some(a) = &pics.active {
                    let _ = write!(
                        body,
                        "<p class=\"gen\">The active frames close up, ×2:</p>{}",
                        strip_html(a, "active frames", true)
                    );
                }
            }
            let fxs: Vec<&str> = [sp.fx, sp.hit_fx, sp.end_fx, sp.slam_fx]
                .into_iter()
                .flatten()
                .collect();
            if !fxs.is_empty() {
                body.push_str("<h2>Effects</h2>");
                for fx in fxs {
                    let _ = write!(body, "<h3><code>{}</code></h3>", esc(fx));
                    match book.fx.get(fx) {
                        Some(shown) if !shown.is_empty() => {
                            for (label, url, w, h) in shown {
                                let pixel = label.starts_with("baked");
                                let _ = write!(body, "<p class=\"gen\">{}</p>", esc(label));
                                body.push_str(&strip_html(&(url.clone(), *w, *h), fx, pixel));
                            }
                        }
                        _ => body
                            .push_str("<p class=\"gen\">No sheet: it plays live in the game.</p>"),
                    }
                }
            }
        }
        body.push_str(&cap_forms(g));
        let fit: Vec<String> = SUPPORTS
            .iter()
            .filter(|s| s.fits(g.all_tags()))
            .map(|s| {
                format!(
                    "<a class=\"tag\" href=\"/wiki/supports.html#{}\">{}</a>",
                    esc(s.id),
                    esc(s.name)
                )
            })
            .collect();
        let _ = write!(
            body,
            "<h2>Supports that fit ({})</h2><p>{}</p>",
            fit.len(),
            if fit.is_empty() {
                "None.".to_owned()
            } else {
                fit.join("")
            }
        );
        wiki.page(&format!("skill/{}.html", g.id), page(g.name, "", &body));
    }
}

fn index(wiki: &mut Wiki) {
    let mut kinds: Vec<&str> = Vec::new();
    let mut rows = String::new();
    for g in GEMS {
        let sp = data::spell(g.id);
        let kind = sp.map_or("", |s| s.kind.name());
        if !kind.is_empty() && !kinds.contains(&kind) {
            kinds.push(kind);
        }
        let tags: Vec<String> = Tag::ALL
            .iter()
            .filter(|t| g.all_tags().has(**t))
            .map(|t| format!("{t:?}"))
            .collect();
        let els: Vec<String> = g.split.iter().map(|(e, _)| format!("{e:?}")).collect();
        let _ = write!(
            rows,
            "<tr data-kind=\"{kind}\" data-tags=\" {} \" data-el=\" {} \" data-tier=\"{}\" data-text=\"{}\"><td>{}</td><td><code>{}</code></td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{} s</td></tr>",
            tags.join(" "),
            els.join(" "),
            tier_name(g),
            esc(&format!("{} {}", g.name, g.id).to_lowercase()),
            skill_link(g),
            esc(g.id),
            esc(kind),
            rank_name(g.rank),
            tier_name(g),
            split_html(g),
            cost_html(g),
            num(g.cooldown),
        );
    }
    let opts = |items: Vec<(String, String)>| -> String {
        items
            .into_iter()
            .map(|(v, l)| format!("<option value=\"{}\">{}</option>", esc(&v), esc(&l)))
            .collect()
    };
    let kinds = opts(
        kinds
            .iter()
            .map(|k| (k.to_string(), k.to_string()))
            .collect(),
    );
    let tags = opts(
        Tag::ALL
            .iter()
            .map(|t| (format!("{t:?}"), t.name().to_owned()))
            .collect(),
    );
    let els = opts(
        Element::ALL
            .iter()
            .map(|e| (format!("{e:?}"), e.name().to_owned()))
            .collect(),
    );
    let body = format!(
        r##"<h1>Skills</h1>
<p>All {n} skills of the shared pool: every fighter can carry any of them. Pick one for its frames, boxes, timings, effect and the supports that fit it.</p>
<div class="filters">
  <input id="q" type="search" placeholder="Search a name or id" aria-label="Search">
  <select id="kind" aria-label="Kind"><option value="">Any kind</option>{kinds}</select>
  <select id="tag" aria-label="Tag"><option value="">Any tag</option>{tags}</select>
  <select id="el" aria-label="Element"><option value="">Any element</option>{els}</select>
  <select id="tier" aria-label="Tier"><option value="">Any tier</option><option>S</option><option>A</option><option>B</option><option>C</option></select>
  <span id="count" class="gen"></span>
</div>
<div class="scroll"><table id="skills"><thead><tr><th>Name</th><th>Id</th><th>Kind</th><th>Rank</th><th>Tier</th><th>Damage kinds</th><th>Cost</th><th>Cooldown</th></tr></thead><tbody>{rows}</tbody></table></div>
<script>
(() => {{
  const f = ["q", "kind", "tag", "el", "tier"].map((id) => document.getElementById(id));
  const rows = [...document.querySelectorAll("#skills tbody tr")];
  const count = document.getElementById("count");
  const tagFromHash = () => {{ const m = location.hash.match(/^#tag=(\w+)/); if (m) f[2].value = m[1]; }};
  const apply = () => {{
    const [q, kind, tag, el, tier] = f.map((x) => x.value.toLowerCase());
    let n = 0;
    for (const r of rows) {{
      const d = r.dataset;
      const ok = (!q || d.text.includes(q)) && (!kind || d.kind.toLowerCase() === kind) &&
        (!tag || d.tags.toLowerCase().includes(" " + tag + " ")) && (!el || d.el.toLowerCase().includes(" " + el + " ")) &&
        (!tier || d.tier.toLowerCase() === tier);
      r.hidden = !ok; if (ok) n++;
    }}
    count.textContent = n + " of " + rows.length;
  }};
  f.forEach((x) => x.addEventListener("input", apply));
  addEventListener("hashchange", () => {{ tagFromHash(); apply(); }});
  tagFromHash(); apply();
}})();
</script>"##,
        n = GEMS.len(),
    );
    wiki.page("index.html", page("Skills", "index.html", &body));
}

fn tags_list(tags: &[Tag]) -> String {
    tags.iter().map(|t| t.name()).collect::<Vec<_>>().join(", ")
}

fn supports(wiki: &mut Wiki, book: &Book) {
    let mut rows = Vec::new();
    for s in SUPPORTS {
        let words = book.support_words.get(s.id);
        let mut asks = Vec::new();
        if !s.all_of.is_empty() {
            asks.push(format!("all of: {}", tags_list(s.all_of)));
        }
        if !s.any_of.is_empty() {
            asks.push(format!("any of: {}", tags_list(s.any_of)));
        }
        if !s.none_of.is_empty() {
            asks.push(format!("none of: {}", tags_list(s.none_of)));
        }
        let fit: Vec<String> = GEMS
            .iter()
            .filter(|g| s.fits(g.all_tags()))
            .map(skill_link)
            .collect();
        let excludes = s
            .excludes
            .iter()
            .map(|x| format!("<a href=\"#{}\">{}</a>", esc(x), esc(x)))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(vec![
            format!(
                "<span id=\"{}\"></span><b>{}</b><br><code>{}</code>",
                esc(s.id),
                esc(s.name),
                esc(s.id)
            ),
            esc(&asks.join("; ")),
            words.map_or(
                "<span class=\"gen\">(no words in the design)</span>".to_owned(),
                |w| esc(&w.does),
            ),
            format!("×{}", num(s.cost)),
            s.points.to_string(),
            excludes,
            format!(
                "<details><summary>{} skills</summary>{}</details>",
                fit.len(),
                fit.join(", ")
            ),
        ]);
    }
    let body = format!(
        "<h1>Supports (bí quyết)</h1><p>{} supports. A support fits a skill that has every tag of its <i>all of</i>, at least one of its <i>any of</i> and none of its <i>none of</i>; a skill's damage kinds count as tags. Its cost multiplies the skill's. What each does is the design's own words (<code>docs/specs/2026-11-17-damage-system-design.md</code>, §8).</p>{}",
        SUPPORTS.len(),
        table(
            &[
                "Support", "Asks", "Does", "Cost", "Points", "Excludes", "Fits"
            ],
            &rows
        )
    );
    wiki.page("supports.html", page("Supports", "supports.html", &body));
}

fn ailments(wiki: &mut Wiki) {
    let mut el_rows = Vec::new();
    for e in Element::ALL {
        let name = |o: Option<Element>| o.map_or(String::new(), element_link);
        el_rows.push(vec![
            format!("<span id=\"el-{e:?}\"></span><b>{}</b>", esc(e.name())),
            name(e.overcomes()),
            name(e.feeds()),
            name(e.opposes()),
            [ForceAilment::Bleed, ForceAilment::Daze]
                .iter()
                .map(|f| Ailment::of(e, *f))
                .fold(Vec::<Ailment>::new(), |mut v, a| {
                    if !v.contains(&a) {
                        v.push(a);
                    }
                    v
                })
                .into_iter()
                .map(ailment_link)
                .collect::<Vec<_>>()
                .join(", "),
        ]);
    }
    let mut rows = Vec::new();
    for a in Ailment::ALL {
        let d = a.data();
        let from: Vec<String> = GEMS
            .iter()
            .filter(|g| ailments_of(g).contains(&a))
            .map(skill_link)
            .collect();
        rows.push(vec![
            format!("<span id=\"ail-{a:?}\"></span><b>{}</b>", esc(a.name())),
            element_link(a.element()),
            num(d.k),
            num(d.cap),
            format!("{} s", num(d.time)),
            d.control
                .map_or(String::new(), |c| format!("{c:?} for {} s", num(d.seconds))),
            format!(
                "<details><summary>{} skills</summary>{}</details>",
                from.len(),
                from.join(", ")
            ),
        ]);
    }
    let body = format!(
        "<h1>Ailments and elements</h1><h2>The ten kinds of damage</h2>{}<h2>Ailments</h2><p>A blow lays the ailment of each kind of damage it deals (a blade cuts, a fist or palm shakes, for force).</p>{}",
        table(
            &["Element", "Overcomes", "Feeds", "Opposes", "Its ailments"],
            &el_rows
        ),
        table(
            &[
                "Ailment", "Element", "k", "Cap", "Lasts", "Control", "Laid by"
            ],
            &rows
        )
    );
    wiki.page(
        "ailments.html",
        page("Ailments and elements", "ailments.html", &body),
    );
}

fn buffs(wiki: &mut Wiki) {
    let mut aura_rows = Vec::new();
    for a in AuraKind::ALL {
        let d = a.data();
        let from: Vec<String> = GEMS
            .iter()
            .filter(|g| gem::aura_of(g) == Some(a))
            .map(skill_link)
            .collect();
        aura_rows.push(vec![
            format!("<span id=\"aura-{a:?}\"></span><b>{}</b>", esc(d.name)),
            if d.pressing { "yes" } else { "" }.to_owned(),
            if d.emotional { "yes" } else { "" }.to_owned(),
            num(d.reserve),
            from.join(", "),
        ]);
    }
    let mut domain_rows = Vec::new();
    for k in DomainKind::ALL {
        let d = k.data();
        let from: Vec<String> = GEMS
            .iter()
            .filter(|g| gem::domain_of(g) == Some(k))
            .map(skill_link)
            .collect();
        domain_rows.push(vec![
            format!("<span id=\"domain-{k:?}\"></span><b>{}</b>", esc(d.name)),
            num(d.life),
            num(d.foe_shot_speed),
            d.swallow_after
                .map_or(String::new(), |s| format!("{} s", num(s))),
            num(d.pull),
            from.join(", "),
        ]);
    }
    let stances: Vec<String> = GEMS
        .iter()
        .filter(|g| matches!(way_of(g), Way::Buff))
        .map(skill_link)
        .collect();
    let body = format!(
        "<h1>Auras, stances, domains</h1><h2>Auras</h2>{}<h2 id=\"stances\">Stances and blessings</h2><p>Body skills with no data of their own: a timed buff on the caster.</p><p>{}</p><h2>Domains</h2>{}",
        table(
            &["Aura", "Pressing", "Emotional", "Reserve", "Skills"],
            &aura_rows
        ),
        stances.join(", "),
        table(
            &[
                "Domain",
                "Life",
                "Foe's shots' speed",
                "Swallows after",
                "Pull",
                "Skills"
            ],
            &domain_rows
        )
    );
    wiki.page(
        "buffs.html",
        page("Auras, stances, domains", "buffs.html", &body),
    );
}

fn curses(wiki: &mut Wiki) {
    let mut by: BTreeMap<String, (String, f64, Vec<String>)> = BTreeMap::new();
    for g in GEMS {
        if let Some(c) = gem::curse_of(g) {
            let e = by
                .entry(format!("{c:?}"))
                .or_insert_with(|| (c.name().to_owned(), c.time(), Vec::new()));
            e.2.push(skill_link(g));
        }
    }
    let rows: Vec<Vec<String>> = by
        .into_iter()
        .map(|(id, (name, time, from))| {
            vec![
                format!(
                    "<span id=\"curse-{}\"></span><b>{}</b>",
                    esc(&id),
                    esc(&name)
                ),
                format!("{} s", num(time)),
                from.join(", "),
            ]
        })
        .collect();
    let body = format!(
        "<h1>Curses</h1><p>Every curse a skill of the pool lays on the foe.</p>{}",
        table(&["Curse", "Lasts", "Laid by"], &rows)
    );
    wiki.page("curses.html", page("Curses", "curses.html", &body));
}

fn keystones(wiki: &mut Wiki) {
    let rows: Vec<Vec<String>> = Keystone::ALL
        .iter()
        .map(|k| {
            vec![
                format!("<b>{}</b><br><code>{}</code>", esc(k.name()), esc(k.id())),
                esc(k.gain()),
                esc(k.cost()),
                format!("<code>{}</code>", esc(k.source())),
            ]
        })
        .collect();
    let roots = arena_calc::loadout::ROOTS
        .iter()
        .map(|e| element_link(*e))
        .collect::<Vec<_>>()
        .join(", ");
    let basics = arena_calc::loadout::BASICS
        .iter()
        .filter_map(|id| gem::gem(id))
        .map(skill_link)
        .collect::<Vec<_>>()
        .join(", ");
    let body = format!(
        "<h1>Keystones and roots</h1><h2>Keystones (đạo tâm)</h2>{}<h2>Roots</h2><p>A fighter's root is one of: {roots}.</p><h2>The loadout</h2><p>Every fighter always holds the basics ({basics}) and spends a budget of {} points on the rest.</p>",
        table(&["Keystone", "Gain", "Cost", "Source"], &rows),
        arena_calc::loadout::BUDGET
    );
    wiki.page(
        "keystones.html",
        page("Keystones and roots", "keystones.html", &body),
    );
}

fn units(wiki: &mut Wiki) {
    use arena_calc::device::{self as dv, Kind as Dk};
    let kinds = [
        (
            Dk::Trap,
            vec![
                ("Lasts", format!("{} s", num(dv::TRAP_LIFETIME))),
                ("Arms after", format!("{} s", num(dv::TRAP_ARM))),
                ("Life", num(dv::DEVICE_LIFE)),
            ],
        ),
        (
            Dk::Mine,
            vec![
                ("Lasts", format!("{} s", num(dv::MINE_LIFETIME))),
                ("Arms after", format!("{} s", num(dv::MINE_ARM))),
                ("Stagger", format!("{} s", num(dv::MINE_STAGGER))),
                ("Life", num(dv::DEVICE_LIFE)),
            ],
        ),
        (
            Dk::Totem,
            vec![
                ("Lasts", format!("{} s", num(dv::TOTEM_LIFETIME))),
                ("Life", num(dv::TOTEM_LIFE)),
                (
                    "Casts at",
                    format!(
                        "{}% of its skill's cadence",
                        num(dv::TOTEM_CADENCE_SHARE * 100.0)
                    ),
                ),
            ],
        ),
        (
            Dk::Minion,
            vec![
                ("Life", num(dv::MINION_LIFE)),
                ("At most", num(dv::MINION_CAP)),
            ],
        ),
    ];
    let mut body = String::from(
        "<h1>Units</h1><p>Traps, mines, totems and minions a skill puts on the field; they strike as the skill's motion does. In the game they are drawn as blocks of their owner's colour for now (no pictures yet).</p>",
    );
    for (k, nums) in kinds {
        let from: Vec<String> = GEMS
            .iter()
            .filter(|g| matches!(way_of(g), Way::Device(d) if d == k))
            .map(skill_link)
            .collect();
        let rows: Vec<(&str, String)> = nums.into_iter().collect();
        let _ = write!(
            body,
            "<h2 id=\"unit-{k:?}\">{k:?}</h2>{}<p>Skills: {}</p>",
            facts(&rows),
            if from.is_empty() {
                "none".to_owned()
            } else {
                from.join(", ")
            }
        );
    }
    wiki.page("units.html", page("Units", "units.html", &body));
}

/// The poses a fighter is hit in, first on the looks page: (clip, what it is).
const POSES: &[(&str, &str)] = &[
    ("stance-idle", "standing"),
    ("duck", "ducking"),
    ("jump", "in the air"),
    ("knockdown", "lying"),
    ("cast", "casting"),
    ("slash", "slashing"),
];

fn looks_page(wiki: &mut Wiki, book: &Book) {
    let mut body = String::from(
        "<h1>Fighters</h1><p>The two looks a fighter is drawn in, every clip of each with the rules' boxes on every frame (the boxes the rules hit and strike with, measured from the sheets). Each look carries the whole pool.</p>",
    );
    body.push_str(LEGEND);
    let mut order: Vec<String> = POSES.iter().map(|(c, _)| (*c).to_owned()).collect();
    for c in arena_duel::pick::CLIPS {
        if !order.iter().any(|o| o == c) {
            order.push((*c).to_owned());
        }
    }
    for (li, (look, sheet)) in book.sheets.iter().enumerate() {
        let _ = write!(
            body,
            "<h2 id=\"look-{}\">{} <code>{}</code></h2><p>{} clips at {} frames a second.</p>",
            look.name(),
            esc(look.display()),
            esc(look.name()),
            sheet.clips.len(),
            num(arena_duel::frames::fps(*look))
        );
        for clip in &order {
            let Some(pics) = book.clips.get(&(li, clip.clone())) else {
                continue;
            };
            let what = POSES
                .iter()
                .find(|(c, _)| c == clip)
                .map_or(String::new(), |(_, w)| format!(" ({w})"));
            let anchor = if li == 0 {
                format!(" id=\"clip-{clip}\"")
            } else {
                String::new()
            };
            let _ = write!(
                body,
                "<h3{anchor}><code>{}</code>{what}</h3><p class=\"gen\">{} frames{}</p>",
                esc(clip),
                pics.frames,
                pics.window
                    .map_or(String::new(), |(s, e)| format!(", active {s} to {e}"))
            );
            if let Some(w) = &pics.whole {
                body.push_str(&strip_html(w, clip, true));
            }
        }
    }
    wiki.page("looks.html", page("Fighters", "looks.html", &body));
}

fn rules(wiki: &mut Wiki) {
    let rows: Vec<(&str, String)> = vec![
        ("Rules version", arena_link::RULES_VERSION.to_string()),
        (
            "A tick",
            format!("1/{} s", num(1.0 / arena_duel::duel::TICK)),
        ),
        ("Between rounds", format!("{} s", num(data::ROUND_PAUSE))),
        ("Skills in the pool", GEMS.len().to_string()),
        ("Supports", SUPPORTS.len().to_string()),
        ("Keystones", Keystone::ALL.len().to_string()),
    ];
    let body = format!(
        r#"<h1>Rules overview</h1>{}
<h2>How a fight runs</h2>
<ul>
<li><b>Turn order.</b> Both fighters' turns, the first of them swapping every tick: seat 0 first on an even tick, seat 1 on an odd one.</li>
<li><b>A blow plays out before the next.</b> While what a skill that binds its caster sent is still out (<i>Binds the caster</i> on its page), the caster casts no other such skill; curses, auras, blessings, movement, guard, recovery and stances never bind.</li>
<li><b>Recovery by progress.</b> A skill's clip plays its wind-up over the skill's wind-up and its follow-through stretched over the skill's recovery.</li>
<li><b>One cast, at most 40% of a life.</b> One bare cast of a skill, at hit and over time, deals at most that share of a base fighter's life and shield; each skill's page gives its own share (<code>arena_calc::cap</code>).</li>
<li><b>Boxes from the pictures.</b> Hurtboxes and hitboxes follow the frame a fighter shows, measured from the sprite sheets (see <a href="/wiki/looks.html">Fighters</a>).</li>
</ul>
<p>The full rules: <a href="/docs/rules.html">docs: Rules</a>. Training a bot: <a href="/docs/training.html">docs: Train a bot</a>. The season: <a href="/docs/tournament.html">docs: Tournament</a>.</p>"#,
        facts(&rows)
    );
    wiki.page("rules.html", page("Rules overview", "rules.html", &body));
}

fn bot(wiki: &mut Wiki) {
    use arena_mind::policy::{ACTIONS, Act, OBS, SLOTS, TAG};
    let acts: Vec<Vec<String>> = (0..ACTIONS)
        .filter_map(|i| Act::from_index(i).map(|a| vec![i.to_string(), esc(&format!("{a:?}"))]))
        .collect();
    let rows: Vec<(&str, String)> = vec![
        ("Numbers it sees", OBS.to_string()),
        ("Of which its style's tag", TAG.to_string()),
        ("Skill slots", SLOTS.to_string()),
        ("Acts", ACTIONS.to_string()),
    ];
    let body = format!(
        "<h1>The bot's view</h1><p>What a bot sees each decision and what it can do; the layout of the numbers is in <a href=\"/docs/bot.html\">docs: Your bot</a>.</p>{}<h2>Its acts</h2>{}",
        facts(&rows),
        table(&["Index", "Act"], &acts)
    );
    wiki.page("bot.html", page("The bot's view", "bot.html", &body));
}
