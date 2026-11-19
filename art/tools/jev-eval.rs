#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
---
//! jev-eval: measures whether Jev's answers (art/tools/jev.rs) can be trusted, on
//! art whose answers are known.
//!
//!   cargo -Zscript art/tools/jev-eval.rs cases <dir>
//!     writes labelled pictures into <dir> and cases.json (the truth), plus the
//!     lists jev.rs reads: fighter.txt (one picture a request), sheets.txt (4x2
//!     sheets, ask with --grid 4x2) and vfx.txt (effects, ask with --checks effect)
//!   cargo -Zscript art/tools/jev-eval.rs report <dir> <review-dir> <name>=<answers.jsonl>...
//!     scores the answers against cases.json, prints the numbers and writes a
//!     review page (index.html, annotated thumbnails, summary.json) into
//!     <review-dir>. The first run named is the main one; any other fighter run
//!     (say the same pictures upscaled) is compared with it in one table.
//!
//! The pictures: PixelLab rotations (facing by file name) and animation frames
//! (single figures facing east), mirrored frames (west), the few smooth fighter
//! pictures there are, research composites (several figures), drawn effect
//! frames (no figure), and frames broken on purpose: a second figure beside the
//! first, a neighbour's body crossing in at an edge, an effect frame pasted on the
//! fighter, the figure cut by the picture's edge, the feet cropped away. The feet
//! truth is the row under the lowest opaque pixel (alpha >= 128).

use image::{Rgba, RgbaImage, imageops};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

const YES: f64 = 0.7;
const NO: f64 = 0.3;
const ALPHA: u8 = 128;

#[derive(Serialize, Deserialize, Clone, Default)]
struct Truth {
    /// accepted facings; empty: not scored
    facing: Vec<String>,
    one_figure: Option<bool>,
    effect: Option<bool>,
    cut_off: Option<bool>,
    feet_visible: Option<bool>,
    /// the row under the lowest opaque pixel, in the picture's pixels
    feet_y: Option<u32>,
    /// the opaque pixels' box [left, top, right, bottom), exclusive
    bbox: Option<[u32; 4]>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Case {
    id: String,
    file: String,
    cell: Option<u32>,
    /// clean, mirror, rotation, two, intrude, fx, cut, nofeet, composite, vfx, sheet
    group: String,
    /// pixel (PixelLab), smooth (image model), vfx
    art: String,
    source: String,
    w: u32,
    h: u32,
    truth: Truth,
}

fn load(p: &Path) -> RgbaImage {
    image::open(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .to_rgba8()
}

fn bbox(img: &RgbaImage) -> Option<[u32; 4]> {
    let (mut l, mut t, mut r, mut b) = (u32::MAX, u32::MAX, 0, 0);
    for (x, y, p) in img.enumerate_pixels() {
        if p[3] >= ALPHA {
            l = l.min(x);
            t = t.min(y);
            r = r.max(x + 1);
            b = b.max(y + 1);
        }
    }
    (r > 0).then_some([l, t, r, b])
}

fn opaque(img: &RgbaImage) -> usize {
    img.pixels().filter(|p| p[3] >= ALPHA).count()
}

fn touches_edge(img: &RgbaImage) -> bool {
    bbox(img).is_some_and(|[l, t, r, b]| l == 0 || t == 0 || r == img.width() || b == img.height())
}

/// Truth that follows from the pixels: the box, the feet row, an edge cut.
fn measured(img: &RgbaImage, feet: bool) -> Truth {
    let bb = bbox(img);
    Truth {
        bbox: bb,
        feet_y: if feet { bb.map(|b| b[3]) } else { None },
        // art that touches an edge may or may not be cut (a feather tip on the
        // border): unknown; only the crops made here are known to be cut
        cut_off: if touches_edge(img) { None } else { Some(false) },
        ..Default::default()
    }
}

/// The magenta screen and its anti-aliased fringe.
fn magenta(p: &Rgba<u8>) -> bool {
    let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
    r > g + 50 && b > g + 50 && (r - b).abs() < 90
}

/// Makes a background colour transparent (magenta, green screens).
fn key(img: &mut RgbaImage, f: impl Fn(&Rgba<u8>) -> bool) {
    for p in img.pixels_mut() {
        if f(p) {
            *p = Rgba([0, 0, 0, 0]);
        }
    }
}

fn pad(img: &RgbaImage, w: u32, h: u32, x: i64, y: i64) -> RgbaImage {
    let mut c = RgbaImage::new(w, h);
    imageops::overlay(&mut c, img, x, y);
    c
}

struct Builder {
    dir: PathBuf,
    cases: Vec<Case>,
}

impl Builder {
    fn add(
        &mut self,
        id: &str,
        group: &str,
        art: &str,
        source: &str,
        img: &RgbaImage,
        truth: Truth,
    ) {
        let file = format!("{id}.png");
        img.save(self.dir.join(&file)).unwrap();
        self.cases.push(Case {
            id: id.into(),
            file,
            cell: None,
            group: group.into(),
            art: art.into(),
            source: source.into(),
            w: img.width(),
            h: img.height(),
            truth,
        });
    }
}

const UPRIGHT: &[&str] = &[
    "stance-idle",
    "walk",
    "cast",
    "guard",
    "hit",
    "frozen",
    "shocked",
    "slash",
    "overhead",
    "thrust",
    "win",
    "duck",
    "dash",
];
const STANDING: &[&str] = &[
    "stance-idle",
    "walk",
    "cast",
    "guard",
    "hit",
    "frozen",
    "shocked",
    "win",
];

fn sorted_pngs(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
        .map(|r| r.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|e| e == "png"));
    v.sort();
    v
}

fn clean_truth(img: &RgbaImage, clip: &str, facing: &str) -> Truth {
    let standing = STANDING.contains(&clip);
    let mut t = measured(img, standing);
    if UPRIGHT.contains(&clip) {
        t.facing = vec![facing.into()];
    }
    t.one_figure = Some(true);
    // the status clips may draw their own sparks or ice
    t.effect = if ["shocked", "frozen"].contains(&clip) {
        None
    } else {
        Some(false)
    };
    t.feet_visible = UPRIGHT.contains(&clip).then_some(true);
    t
}

/// The broken variants of one clean upright figure.
fn variants(
    b: &mut Builder,
    tag: &str,
    art: &str,
    src: &str,
    img: &RgbaImage,
    other: &RgbaImage,
    fx: &RgbaImage,
    kinds: &[&str],
) {
    let [l, t, r, bo] = bbox(img).unwrap();
    let (fw, fh) = (r - l, bo - t);
    let base = |mut tr: Truth| {
        tr.facing = vec!["east".into()];
        tr
    };
    for kind in kinds {
        match *kind {
            "two" => {
                // a second figure beside the first, both whole
                let w = img.width() + fw + fw / 4;
                let mut c = pad(img, w, img.height(), 0, 0);
                imageops::overlay(&mut c, other, (fw + fw / 4) as i64, 0);
                let mut tr = base(measured(&c, false));
                tr.one_figure = Some(false);
                tr.feet_visible = Some(true);
                tr.cut_off = None;
                b.add(&format!("{tag}-two"), "two", art, src, &c, tr);
            }
            "intrude" => {
                // a neighbour's body crossing in from the right edge (a hand in the next cell)
                let mut c = img.clone();
                let ob = bbox(other).unwrap();
                let x = img.width() as i64 - (ob[2] - ob[0]) as i64 * 2 / 5 - ob[0] as i64;
                imageops::overlay(&mut c, other, x, 0);
                let mut tr = base(measured(img, true));
                tr.one_figure = Some(false);
                tr.cut_off = None;
                tr.feet_visible = Some(true);
                b.add(&format!("{tag}-intrude"), "intrude", art, src, &c, tr);
            }
            "fx" => {
                // an effect drawn on the fighter, about half its height, at the chest and hand
                let k = (fh as f64 * 0.55) / fx.height() as f64;
                let (ew, eh) = (
                    ((fx.width() as f64 * k) as u32).max(1),
                    ((fx.height() as f64 * k) as u32).max(1),
                );
                let e = imageops::resize(fx, ew, eh, imageops::FilterType::Triangle);
                let cx = (l + fw * 3 / 5) as i64 - ew as i64 / 2;
                let cy = (t + fh * 2 / 5) as i64 - eh as i64 / 2;
                let mut c = img.clone();
                imageops::overlay(&mut c, &e, cx, cy);
                let mut tr = base(measured(img, true));
                tr.one_figure = Some(true);
                tr.effect = Some(true);
                tr.feet_visible = Some(true);
                b.add(&format!("{tag}-fx"), "fx", art, src, &c, tr);
            }
            "cut" => {
                // the picture's right edge and top cut through the figure
                let cw = l + fw * 3 / 5;
                let ct = t + fh / 8;
                let c = imageops::crop_imm(img, 0, ct, cw, img.height() - ct).to_image();
                let mut tr = base(measured(&c, true));
                tr.one_figure = Some(true);
                tr.effect = Some(false);
                tr.cut_off = Some(true);
                // the right edge may cut the front foot
                tr.feet_visible = None;
                b.add(&format!("{tag}-cut"), "cut", art, src, &c, tr);
            }
            "nofeet" => {
                // the bottom edge cuts the legs above the feet
                let ch = t + fh * 7 / 10;
                let c = imageops::crop_imm(img, 0, 0, img.width(), ch).to_image();
                let mut tr = base(measured(&c, false));
                tr.one_figure = Some(true);
                tr.effect = Some(false);
                tr.cut_off = Some(true);
                tr.feet_visible = Some(false);
                b.add(&format!("{tag}-nofeet"), "nofeet", art, src, &c, tr);
            }
            _ => unreachable!(),
        }
    }
}

fn cases(dir: &Path) {
    let art = Path::new("art");
    let refs = Path::new("../../references/agent-sprite-forge/src");
    std::fs::create_dir_all(dir).unwrap();
    let mut b = Builder {
        dir: dir.into(),
        cases: vec![],
    };

    // the drawn effects: the fullest frame of each (most opaque pixels)
    let mut effects: Vec<(String, RgbaImage)> = vec![];
    let mut names: Vec<PathBuf> = std::fs::read_dir(art.join("vfx"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    names.sort();
    for n in names.iter().filter(|n| n.join("frames").is_dir()) {
        let mut frames = vec![];
        for d in std::fs::read_dir(n.join("frames")).unwrap() {
            frames.extend(sorted_pngs(&d.unwrap().path()));
        }
        frames.sort();
        if let Some(best) = frames.iter().map(|f| load(f)).max_by_key(opaque) {
            effects.push((n.file_name().unwrap().to_string_lossy().into(), best));
        }
    }
    // a vivid effect for pasting, picked by colour spread: prefer the named ones
    let paste: Vec<&(String, RgbaImage)> = [
        "fire-fire-pillar",
        "lightning-storm-field",
        "golden-bell",
        "holy-regen-aura",
        "ice-frost-armor",
        "arcane-mana-shield",
        "shadow-curse",
        "fire-flame-ring",
        "lightning-arc-discharge",
        "rune-circle",
    ]
    .iter()
    .filter_map(|n| effects.iter().find(|(e, _)| e == n))
    .collect();
    println!("{} effects, {} for pasting", effects.len(), paste.len());

    // rotations: facing from the file name
    let rot_dirs = [
        ("cul-idle", "cultivator/src/Idle"),
        ("cul-stance", "cultivator/src/Stance"),
        ("dao-idle", "seven-colored-daoist/src/Idle"),
        ("dao-d128", "seven-colored-daoist/design-128/Idle"),
        ("dao-d128v2", "seven-colored-daoist/design-128/Idle_v2"),
    ];
    for (tag, d) in rot_dirs {
        for f in sorted_pngs(&art.join(d).join("rotations")) {
            let dirn = f.file_stem().unwrap().to_string_lossy().to_string();
            let facing: Vec<String> = match dirn.as_str() {
                "east" => vec!["east"],
                "west" => vec!["west"],
                "south" => vec!["toward"],
                "north" => vec!["away"],
                "south-east" => vec!["east", "toward"],
                "south-west" => vec!["west", "toward"],
                "north-east" => vec!["east", "away"],
                "north-west" => vec!["west", "away"],
                _ => continue,
            }
            .into_iter()
            .map(String::from)
            .collect();
            let img = load(&f);
            let mut t = measured(&img, true);
            t.facing = facing;
            t.one_figure = Some(true);
            t.effect = Some(false);
            t.feet_visible = Some(true);
            b.add(
                &format!("rot-{tag}-{dirn}"),
                "rotation",
                "pixel",
                &f.display().to_string(),
                &img,
                t,
            );
        }
    }

    // animation frames: the first and the middle frame of every clip
    let anim_dirs = [
        ("cul-idle", "cultivator/src/Idle"),
        ("cul-stance", "cultivator/src/Stance"),
        ("dao", "seven-colored-daoist/src/Idle"),
    ];
    let mut uprights: Vec<(String, String, RgbaImage)> = vec![];
    for (tag, d) in anim_dirs {
        let mut clips: Vec<PathBuf> = std::fs::read_dir(art.join(d).join("animations"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        clips.sort();
        for clip in clips {
            let name = clip.file_name().unwrap().to_string_lossy().to_string();
            let frames = sorted_pngs(&clip.join("east"));
            for (k, i) in [0, frames.len() / 2].into_iter().enumerate() {
                let f = &frames[i];
                let img = load(f);
                let t = clean_truth(&img, &name, "east");
                let id = format!("{tag}-{name}-{i:02}");
                if k == 1 && UPRIGHT.contains(&name.as_str()) {
                    uprights.push((id.clone(), f.display().to_string(), img.clone()));
                }
                b.add(&id, "clean", "pixel", &f.display().to_string(), &img, t);
            }
        }
    }
    // mirrored: every other upright middle frame, facing west
    for (id, src, img) in uprights.iter().step_by(2) {
        let m = imageops::flip_horizontal(img);
        let clip = UPRIGHT.iter().find(|c| id.contains(*c)).unwrap();
        let t = clean_truth(&m, clip, "west");
        b.add(&format!("{id}-mirror"), "mirror", "pixel", src, &m, t);
    }
    // broken on purpose: round-robin over the upright middle frames
    let kinds = ["two", "intrude", "fx", "cut", "nofeet"];
    for (n, (id, src, img)) in uprights.iter().enumerate() {
        let other = &uprights[(n + 7) % uprights.len()].2;
        let fx = &paste[n % paste.len()].1;
        variants(
            &mut b,
            id,
            "pixel",
            src,
            img,
            other,
            fx,
            &[kinds[n % kinds.len()]],
        );
    }

    // smooth fighters: the one master and its three takes (magenta keyed)
    let mut smooth: Vec<(String, String, RgbaImage)> = vec![];
    let master = refs.join("v040/aria-master.png");
    if master.exists() {
        smooth.push(("aria".into(), master.display().to_string(), load(&master)));
        let takes = refs.join("v040/aria-master-takes.png");
        let all = load(&takes);
        for i in 0..3u32 {
            let mut c = imageops::crop_imm(&all, i * 320, 20, 320, all.height() - 20).to_image();
            key(&mut c, magenta);
            smooth.push((
                format!("aria-take{}", i + 1),
                takes.display().to_string(),
                c,
            ));
        }
        let mut whole = all.clone();
        key(&mut whole, magenta);
        let t = Truth {
            facing: vec!["east".into()],
            one_figure: Some(false),
            effect: Some(false),
            feet_visible: Some(true),
            ..Default::default()
        };
        b.add(
            "aria-takes-sheet",
            "composite",
            "smooth",
            &takes.display().to_string(),
            &whole,
            t,
        );
    }
    for (n, (id, src, img)) in smooth.iter().enumerate() {
        let t = clean_truth(img, "stance-idle", "east");
        b.add(&format!("{id}-clean"), "clean", "smooth", src, img, t);
        let m = imageops::flip_horizontal(img);
        b.add(
            &format!("{id}-mirror"),
            "mirror",
            "smooth",
            src,
            &m,
            clean_truth(&m, "stance-idle", "west"),
        );
        let other = &smooth[(n + 1) % smooth.len()].2;
        let fx = &paste[(n * 3) % paste.len()].1;
        variants(&mut b, id, "smooth", src, img, other, fx, &kinds);
    }
    // research composites: several figures in one picture
    for (id, f, facing) in [
        (
            "research-compare",
            art.join("research/density/compare.png"),
            "east",
        ),
        (
            "research-hd-pair",
            art.join("research/density/hd_pair.png"),
            "east",
        ),
        (
            "research-tiers",
            art.join("research/density/tiers_same_scale.png"),
            "",
        ),
    ] {
        let img = load(&f);
        let t = Truth {
            facing: if facing.is_empty() {
                vec![]
            } else {
                vec![facing.into()]
            },
            one_figure: Some(false),
            effect: Some(false),
            ..Default::default()
        };
        b.add(id, "composite", "pixel", &f.display().to_string(), &img, t);
    }
    // a front view on a green screen
    let kael = refs.join("pixel-looks/kael-picture.png");
    if kael.exists() {
        let mut img = load(&kael);
        key(&mut img, |p| p[1] > 180 && p[0] < 100 && p[2] < 100);
        let mut t = measured(&img, true);
        t.facing = vec!["toward".into()];
        t.one_figure = Some(true);
        t.effect = Some(false);
        t.feet_visible = Some(true);
        b.add(
            "kael-front",
            "clean",
            "pixel",
            &kael.display().to_string(),
            &img,
            t,
        );
    }

    // effects alone: no figure in them
    let mut vfx_list = vec![];
    for (name, img) in effects.iter().step_by((effects.len() / 30).max(1)) {
        let t = Truth {
            one_figure: Some(false),
            effect: Some(true),
            ..Default::default()
        };
        b.add(
            &format!("vfx-{name}"),
            "vfx",
            "vfx",
            &format!("art/vfx/{name}"),
            img,
            t,
        );
        vfx_list.push(dir.join(format!("vfx-{name}.png")));
    }
    let n_fighter = b.cases.len();

    // 4x2 sheets of key poses, for --grid 4x2
    let dao: Vec<&(String, String, RgbaImage)> = uprights
        .iter()
        .filter(|u| u.0.starts_with("dao-"))
        .collect();
    let cul: Vec<&(String, String, RgbaImage)> = uprights
        .iter()
        .filter(|u| u.0.starts_with("cul-stance"))
        .collect();
    for (s, pool) in [("sheet-dao", &dao), ("sheet-cul", &cul)] {
        let (cw, ch) = pool[0].2.dimensions();
        let mut sheet = RgbaImage::new(cw * 4, ch * 2);
        let mut truths = vec![];
        for i in 0..8u32 {
            let (id, _, img) = pool[i as usize % pool.len()];
            let clip = UPRIGHT.iter().find(|c| id.contains(*c)).unwrap();
            let (x, y) = ((i % 4) * cw, (i / 4) * ch);
            let mut img = img.clone();
            let mut facing = "east";
            if s == "sheet-dao" && i == 2 {
                img = imageops::flip_horizontal(&img);
                facing = "west";
            }
            let mut t = clean_truth(&img, clip, facing);
            if s == "sheet-dao" && i == 5 {
                let [l, tp, r, bo] = bbox(&img).unwrap();
                let fx = &paste[0].1;
                let k = ((bo - tp) as f64 * 0.55) / fx.height() as f64;
                let e = imageops::resize(
                    fx,
                    (fx.width() as f64 * k) as u32,
                    (fx.height() as f64 * k) as u32,
                    imageops::FilterType::Triangle,
                );
                imageops::overlay(
                    &mut img,
                    &e,
                    (l + (r - l) * 3 / 5) as i64 - e.width() as i64 / 2,
                    (tp + (bo - tp) * 2 / 5) as i64 - e.height() as i64 / 2,
                );
                t.effect = Some(true);
            }
            if s == "sheet-cul" && i == 1 {
                // this pose is drawn too far right: it crosses into the next cell
                let [l, _, r, _] = bbox(&img).unwrap();
                let shift = (cw - r) as i64 + (r - l) as i64 * 3 / 10;
                imageops::overlay(&mut sheet, &img, x as i64 + shift, y as i64);
                let cell = imageops::crop_imm(&sheet, x, y, cw, ch).to_image();
                let mut t = clean_truth(&cell, clip, facing);
                t.cut_off = Some(true);
                truths.push(t);
                continue;
            }
            imageops::overlay(&mut sheet, &img, x as i64, y as i64);
            truths.push(t);
        }
        // truth that follows from the sheet's pixels, cell by cell
        for (i, t) in truths.iter_mut().enumerate() {
            let i = i as u32;
            let cell = imageops::crop_imm(&sheet, (i % 4) * cw, (i / 4) * ch, cw, ch).to_image();
            let m = measured(&cell, t.feet_y.is_some());
            t.bbox = m.bbox;
            t.feet_y = m.feet_y;
            if t.cut_off != Some(true) {
                t.cut_off = m.cut_off;
            }
        }
        if s == "sheet-cul" {
            // the neighbour's weapon or hand now shows in cell 2
            truths[2].one_figure = Some(false);
            truths[2].cut_off = None;
        }
        let file = format!("{s}.png");
        sheet.save(dir.join(&file)).unwrap();
        for (i, t) in truths.into_iter().enumerate() {
            b.cases.push(Case {
                id: format!("{s}-c{i}"),
                file: file.clone(),
                cell: Some(i as u32),
                group: "sheet".into(),
                art: "pixel".into(),
                source: s.into(),
                w: cw,
                h: ch,
                truth: t,
            });
        }
    }

    let abs = |f: &str| {
        std::fs::canonicalize(dir.join(f))
            .unwrap()
            .display()
            .to_string()
    };
    let fighter: Vec<String> = b.cases[..n_fighter].iter().map(|c| abs(&c.file)).collect();
    std::fs::write(dir.join("fighter.txt"), fighter.join("\n") + "\n").unwrap();
    std::fs::write(
        dir.join("sheets.txt"),
        format!("{}\n{}\n", abs("sheet-dao.png"), abs("sheet-cul.png")),
    )
    .unwrap();
    let vfx: Vec<String> = vfx_list
        .iter()
        .map(|p| std::fs::canonicalize(p).unwrap().display().to_string())
        .collect();
    std::fs::write(dir.join("vfx.txt"), vfx.join("\n") + "\n").unwrap();
    std::fs::write(
        dir.join("cases.json"),
        serde_json::to_string_pretty(&b.cases).unwrap(),
    )
    .unwrap();
    let mut by: BTreeMap<String, usize> = BTreeMap::new();
    for c in &b.cases {
        *by.entry(format!("{}/{}", c.art, c.group)).or_default() += 1;
    }
    println!(
        "{} cases ({} fighter pictures, 2 sheets, {} effects): {by:?}",
        b.cases.len(),
        n_fighter,
        vfx.len()
    );
}

// ---------------------------------------------------------------- report

/// One noul: its name, and which answer means the picture is bad.
const NOULS: &[(&str, bool)] = &[
    ("one_figure", false),
    ("figure", true),
    ("effect", true),
    ("cut_off", true),
    ("feet_visible", false),
];

#[derive(Default, Clone)]
struct Tally {
    n: usize,
    flag: usize,
    unsure: usize,
}

impl Tally {
    fn add(&mut self, v: f64, bad_is_yes: bool) {
        self.n += 1;
        let (flag, clear) = if bad_is_yes {
            (v >= YES, v <= NO)
        } else {
            (v <= NO, v >= YES)
        };
        if flag {
            self.flag += 1;
        } else if !clear {
            self.unsure += 1;
        }
    }
    fn rate(&self, k: usize) -> String {
        if self.n == 0 {
            "-".into()
        } else {
            format!(
                "{}/{} ({:.0}%)",
                k,
                self.n,
                100.0 * k as f64 / self.n as f64
            )
        }
    }
}

fn noul(a: &Value, q: &str) -> Option<f64> {
    a[q]["noul"].as_f64()
}

fn bx(a: &Value, q: &str) -> Option<[f64; 4]> {
    let b = &a[q]["box"];
    Some([
        b["left"].as_f64()?,
        b["top"].as_f64()?,
        b["right"].as_f64()?,
        b["bottom"].as_f64()?,
    ])
}

fn iou(a: [f64; 4], b: [f64; 4]) -> f64 {
    let iw = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let ih = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    let i = iw * ih;
    let u = (a[2] - a[0]) * (a[3] - a[1]) + (b[2] - b[0]) * (b[3] - b[1]) - i;
    if u <= 0.0 { 0.0 } else { i / u }
}

/// What was wrong with one answer set, question by question.
fn verdicts(c: &Case, a: &Value) -> Vec<(String, String, bool)> {
    let mut out = vec![];
    let t = &c.truth;
    if let Some(ch) = a["facing"]["choice"].as_str() {
        let ok = t.facing.is_empty() || t.facing.iter().any(|f| f == ch);
        let truth = if t.facing.is_empty() {
            "-".into()
        } else {
            t.facing.join("|")
        };
        out.push((
            "facing".into(),
            format!(
                "{ch} {:.2} (truth {truth})",
                a["facing"]["confidence"].as_f64().unwrap_or(0.0)
            ),
            ok,
        ));
    }
    for &(q, bad_is_yes) in NOULS {
        let Some(v) = noul(a, q) else { continue };
        let tv = match q {
            "one_figure" => t.one_figure,
            "figure" => (c.art == "vfx").then_some(false),
            "effect" => t.effect,
            "cut_off" => t.cut_off,
            _ => t.feet_visible,
        };
        let ok = match tv {
            None => true,
            Some(true) => v >= YES,
            Some(false) => v <= NO,
        };
        let _ = bad_is_yes;
        let truth = tv.map(|b| if b { "yes" } else { "no" }).unwrap_or("-");
        out.push((q.into(), format!("{v:.2} (truth {truth})"), ok));
    }
    if let (Some(fy), Some(b)) = (t.feet_y, bx(a, "feet_box")) {
        let err = b[3] * c.h as f64 - fy as f64;
        let fh = t.bbox.map(|b| (b[3] - b[1]) as f64).unwrap_or(c.h as f64);
        out.push((
            "feet_box".into(),
            format!(
                "bottom {:+.0} px ({:+.0}% of figure)",
                err,
                100.0 * err / fh
            ),
            err.abs() <= 0.05 * fh,
        ));
    }
    if let (Some(bb), Some(b), false) = (t.bbox, bx(a, "figure_box"), c.group == "two") {
        let tb = [
            bb[0] as f64 / c.w as f64,
            bb[1] as f64 / c.h as f64,
            bb[2] as f64 / c.w as f64,
            bb[3] as f64 / c.h as f64,
        ];
        let v = iou(tb, b);
        out.push(("figure_box".into(), format!("IoU {v:.2}"), v >= 0.6));
    }
    out
}

fn load_runs(specs: &[String]) -> Vec<(String, BTreeMap<(String, Option<u32>), Value>)> {
    specs
        .iter()
        .map(|s| {
            let (name, file) = s.split_once('=').expect("<name>=<answers.jsonl>");
            let mut m = BTreeMap::new();
            for l in std::fs::read_to_string(file).unwrap().lines() {
                let v: Value = serde_json::from_str(l).unwrap();
                let p = std::fs::canonicalize(v["path"].as_str().unwrap())
                    .unwrap()
                    .display()
                    .to_string();
                let cell = v["cell"].as_u64().map(|c| c as u32);
                m.insert((p, cell), v);
            }
            (name.to_string(), m)
        })
        .collect()
}

fn pct(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[((v.len() - 1) as f64 * p).round() as usize]
}

struct Scores {
    lines: Vec<String>,
    json: Value,
}

/// The numbers of one run over the cases it answered (filtered by `keep`).
fn score(
    cases: &[Case],
    dir: &Path,
    run: &BTreeMap<(String, Option<u32>), Value>,
    keep: &dyn Fn(&Case) -> bool,
) -> Scores {
    let mut facing: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut clean: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut bad: BTreeMap<&str, Tally> = BTreeMap::new();
    let mut feet = vec![];
    let mut feet_rel = vec![];
    let mut ious = vec![];
    let (mut lat, mut cost, mut n) = (vec![], 0.0, 0);
    for c in cases.iter().filter(|c| keep(c)) {
        let p = std::fs::canonicalize(dir.join(&c.file))
            .unwrap()
            .display()
            .to_string();
        let Some(v) = run.get(&(p, c.cell)) else {
            continue;
        };
        let a = &v["answers"];
        if a.is_null() {
            continue;
        }
        n += 1;
        lat.push(v["latency_ms"].as_f64().unwrap_or(0.0));
        cost += v["cost"].as_f64().unwrap_or(0.0);
        let t = &c.truth;
        if !t.facing.is_empty()
            && let Some(ch) = a["facing"]["choice"].as_str()
        {
            let class = format!(
                "{}{}",
                t.facing.join("|"),
                if c.art == "smooth" { " (smooth)" } else { "" }
            );
            let e = facing.entry(class).or_default();
            e.0 += 1;
            e.1 += t.facing.iter().any(|f| f == ch) as usize;
        }
        for &(q, bad_is_yes) in NOULS {
            let Some(val) = noul(a, q) else { continue };
            let tv = match q {
                "one_figure" => t.one_figure,
                "figure" => (c.art == "vfx").then_some(false),
                "effect" => t.effect,
                "cut_off" => t.cut_off,
                _ => t.feet_visible,
            };
            match tv {
                // the picture is clean on this question
                Some(x) if x != bad_is_yes => clean.entry(q).or_default().add(val, bad_is_yes),
                Some(_) => bad.entry(q).or_default().add(val, bad_is_yes),
                None => {}
            }
        }
        if let (Some(fy), Some(b)) = (t.feet_y, bx(a, "feet_box")) {
            let err = b[3] * c.h as f64 - fy as f64;
            feet.push(err.abs());
            if let Some(bb) = t.bbox {
                feet_rel.push(100.0 * err.abs() / (bb[3] - bb[1]) as f64);
            }
        }
        if let (Some(bb), Some(b), false) = (t.bbox, bx(a, "figure_box"), c.group == "two") {
            let tb = [
                bb[0] as f64 / c.w as f64,
                bb[1] as f64 / c.h as f64,
                bb[2] as f64 / c.w as f64,
                bb[3] as f64 / c.h as f64,
            ];
            ious.push(iou(tb, b));
        }
    }
    let mut lines = vec![];
    let mut j = json!({"answered": n, "cost": cost});
    lines.push(format!(
        "{n} answered; latency p50 {:.0} ms, p90 {:.0} ms; cost ${cost:.4}",
        pct(&mut lat, 0.5),
        pct(&mut lat, 0.9)
    ));
    j["latency_p50"] = json!(pct(&mut lat, 0.5));
    j["latency_p90"] = json!(pct(&mut lat, 0.9));
    let mut fj = serde_json::Map::new();
    for (k, (tot, ok)) in &facing {
        lines.push(format!(
            "facing {k}: {ok}/{tot} ({:.0}%)",
            100.0 * *ok as f64 / *tot as f64
        ));
        fj.insert(k.clone(), json!({"n": tot, "correct": ok}));
    }
    j["facing"] = Value::Object(fj);
    for &(q, _) in NOULS {
        let c = clean.get(q).cloned().unwrap_or_default();
        let b = bad.get(q).cloned().unwrap_or_default();
        lines.push(format!(
            "{q}: clean pictures flagged {} unsure {}; broken pictures caught {} unsure {}",
            c.rate(c.flag),
            c.rate(c.unsure),
            b.rate(b.flag),
            b.rate(b.unsure)
        ));
        j[q] = json!({"clean_n": c.n, "clean_flagged": c.flag, "clean_unsure": c.unsure, "bad_n": b.n, "bad_caught": b.flag, "bad_unsure": b.unsure});
    }
    let (f50, f90, r50, r90) = (
        pct(&mut feet, 0.5),
        pct(&mut feet, 0.9),
        pct(&mut feet_rel, 0.5),
        pct(&mut feet_rel, 0.9),
    );
    lines.push(format!(
        "feet box bottom vs lowest opaque row: |error| p50 {f50:.1} px, p90 {f90:.1} px (p50 {r50:.1}%, p90 {r90:.1}% of the figure's height; n {})",
        feet.len()
    ));
    j["feet_err_px"] =
        json!({"n": feet.len(), "p50": f50, "p90": f90, "p50_pct": r50, "p90_pct": r90});
    let (i50, i10) = (pct(&mut ious, 0.5), pct(&mut ious, 0.1));
    lines.push(format!(
        "figure box IoU with the opaque pixels' box: p50 {i50:.2}, p10 {i10:.2} (n {})",
        ious.len()
    ));
    j["figure_iou"] = json!({"n": ious.len(), "p50": i50, "p10": i10});
    Scores { lines, json: j }
}

fn draw_rect(img: &mut RgbaImage, r: [f64; 4], col: Rgba<u8>) {
    let (w, h) = (img.width() as f64, img.height() as f64);
    let x0 = (r[0] * w).clamp(0.0, w - 1.0) as u32;
    let x1 = (r[2] * w).clamp(0.0, w - 1.0) as u32;
    let y0 = (r[1] * h).clamp(0.0, h - 1.0) as u32;
    let y1 = (r[3] * h).clamp(0.0, h - 1.0) as u32;
    for d in 0..2u32 {
        for x in x0..=x1 {
            img.put_pixel(x, (y0 + d).min(img.height() - 1), col);
            img.put_pixel(x, y1.saturating_sub(d), col);
        }
        for y in y0..=y1 {
            img.put_pixel((x0 + d).min(img.width() - 1), y, col);
            img.put_pixel(x1.saturating_sub(d), y, col);
        }
    }
}

fn thumb(src: &RgbaImage, a: &Value, truth_feet: Option<f64>) -> RgbaImage {
    let target = 220.0;
    let k = target / src.height() as f64;
    let (w, h) = (((src.width() as f64 * k) as u32).max(1), target as u32);
    let filter = if k >= 1.0 {
        imageops::FilterType::Nearest
    } else {
        imageops::FilterType::Triangle
    };
    let s = imageops::resize(src, w, h, filter);
    let mut img = RgbaImage::from_pixel(w, h, Rgba([238, 238, 232, 255]));
    imageops::overlay(&mut img, &s, 0, 0);
    if let Some(fy) = truth_feet {
        let y = ((fy * h as f64) as u32).min(h - 1);
        for x in 0..w {
            if x % 6 < 4 {
                img.put_pixel(x, y, Rgba([220, 30, 30, 255]));
            }
        }
    }
    if let Some(b) = bx(a, "figure_box") {
        draw_rect(&mut img, b, Rgba([40, 90, 230, 255]));
    }
    if let Some(b) = bx(a, "feet_box") {
        draw_rect(&mut img, b, Rgba([20, 170, 60, 255]));
    }
    img
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn report(dir: &Path, review: &Path, specs: &[String]) {
    let cases: Vec<Case> =
        serde_json::from_str(&std::fs::read_to_string(dir.join("cases.json")).unwrap()).unwrap();
    let runs = load_runs(specs);
    std::fs::create_dir_all(review.join("img")).unwrap();
    let mut html = String::new();
    let mut summary = serde_json::Map::new();
    let mut sections: Vec<(String, Vec<String>)> = vec![];
    for (name, run) in &runs {
        let all = score(&cases, dir, run, &|_| true);
        let pixel = score(&cases, dir, run, &|c| c.art == "pixel");
        let smooth = score(&cases, dir, run, &|c| c.art == "smooth");
        let vfx = score(&cases, dir, run, &|c| c.art == "vfx");
        for (k, s) in [
            ("all", &all),
            ("pixel", &pixel),
            ("smooth", &smooth),
            ("vfx", &vfx),
        ] {
            if s.json["answered"].as_u64() == Some(0) {
                continue;
            }
            println!("== {name} / {k}");
            for l in &s.lines {
                println!("  {l}");
            }
            summary.insert(format!("{name}/{k}"), s.json.clone());
            sections.push((format!("{name}: {k} pictures"), s.lines.clone()));
        }
    }
    std::fs::write(
        review.join("summary.json"),
        serde_json::to_string_pretty(&summary).unwrap(),
    )
    .unwrap();

    // the cards: every case answered by the first run that answered it
    let mut cards = String::new();
    let mut wrong_n = 0;
    let mut shown = 0;
    let mut sheet_cache: BTreeMap<String, RgbaImage> = BTreeMap::new();
    let mut ordered: Vec<&Case> = cases.iter().collect();
    ordered.sort_by_key(|c| (c.art != "smooth", c.group == "clean", c.id.clone()));
    for c in ordered {
        let p = std::fs::canonicalize(dir.join(&c.file))
            .unwrap()
            .display()
            .to_string();
        let Some((rname, v)) = runs
            .iter()
            .find_map(|(n, r)| r.get(&(p.clone(), c.cell)).map(|v| (n, v)))
        else {
            continue;
        };
        let a = &v["answers"];
        if a.is_null() {
            continue;
        }
        let img = match c.cell {
            None => load(&dir.join(&c.file)),
            Some(i) => {
                let s = sheet_cache
                    .entry(c.file.clone())
                    .or_insert_with(|| load(&dir.join(&c.file)));
                imageops::crop_imm(s, (i % 4) * c.w, (i / 4) * c.h, c.w, c.h).to_image()
            }
        };
        let t = thumb(&img, a, c.truth.feet_y.map(|f| f as f64 / c.h as f64));
        let file = format!("img/{}.png", c.id);
        t.save(review.join(&file)).unwrap();
        let vs = verdicts(c, a);
        let wrong = vs.iter().any(|v| !v.2);
        for (q, s, _) in vs.iter().filter(|v| !v.2) {
            println!("WRONG {} [{rname}] {q}: {s}", c.id);
        }
        wrong_n += wrong as usize;
        shown += 1;
        let mut li = String::new();
        for (q, s, ok) in &vs {
            let _ = write!(
                li,
                "<li class=\"{}\"><b>{q}</b> {}</li>",
                if *ok { "ok" } else { "no" },
                esc(s)
            );
        }
        let _ = write!(
            cards,
            "<div class=\"card{}\"><img src=\"{file}\" loading=\"lazy\"><div class=\"id\">{} <span>{} / {} / {rname}</span></div><ul>{li}</ul></div>",
            if wrong { " wrong" } else { "" },
            esc(&c.id),
            c.art,
            c.group
        );
    }

    let _ = write!(
        html,
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Jev on fighter art</title><style>
body{{font:14px/1.45 system-ui,sans-serif;margin:24px;background:#16181d;color:#ddd;max-width:1500px}}
h1{{font-size:22px}} h2{{font-size:17px;margin-top:28px}} code{{background:#262a33;padding:1px 4px;border-radius:3px}}
.grid{{display:flex;flex-wrap:wrap;gap:10px}} .card{{background:#20232b;border:2px solid #2c303a;border-radius:6px;padding:6px;width:236px}}
.card.wrong{{border-color:#d33}} .card img{{display:block;margin:0 auto;max-width:224px;height:220px;object-fit:contain;background:#eee}}
.id{{font-weight:600;font-size:12px;margin:4px 0;word-break:break-all}} .id span{{font-weight:400;color:#999}}
ul{{margin:0;padding-left:16px;font-size:12px}} li.no{{color:#ff6b6b;font-weight:600}} li.ok{{color:#9c9}}
pre{{background:#20232b;padding:10px;border-radius:6px;white-space:pre-wrap}} .legend span{{padding:0 6px}}
</style></head><body>
<h1>Jev (TypeSafe decision model) as a checker of fighter pictures</h1>
<p>Can Jev (<code>typesafe/jev-1.14</code> through OpenRouter's decisions API) catch the bad pictures of art/README.md step 5
before the owner looks: wrong facing, a second figure, an effect drawn into the fighter, the figure cut by an edge, missing feet?
Tool: <code>cargo -Zscript art/tools/jev.rs</code>; this page: <code>art/tools/jev-eval.rs</code>. Every picture was sent once,
with all its questions in one request. A noul is read as yes at &ge; {YES}, no at &le; {NO}, unsure between.</p>
<h2>What was checked</h2>
<p>PixelLab rotations (facing from the file name) and animation frames (single figures facing east), the frames mirrored (west),
the one smooth image-model fighter we have (Aria from agent-sprite-forge, and its three takes), research composites
(several figures), drawn effect frames (no figure), and frames broken on purpose: a second figure beside the first (<i>two</i>),
a neighbour's body crossing in at the right edge (<i>intrude</i>), a drawn effect pasted on the fighter (<i>fx</i>), the right
edge and top cut through the figure (<i>cut</i>), the legs cropped above the feet (<i>nofeet</i>), and two 4x2 sheets asked
cell by cell (<i>sheet</i>: one cell mirrored, one with an effect, one pose drawn over into the next cell).
The feet truth is the row under the lowest opaque pixel.</p>
"#
    );
    for (title, lines) in &sections {
        let _ = write!(
            html,
            "<h2>{}</h2><pre>{}</pre>",
            esc(title),
            esc(&lines.join("\n"))
        );
    }
    html.push_str("<!--DOUBTS-->");
    let _ = write!(
        html,
        r#"<h2>Every picture ({shown}, {wrong_n} with a wrong answer, red)</h2>
<p class="legend">Blue: Jev's figure box. Green: Jev's feet box. Red dashes: the true lowest opaque row. A red line in a card is a wrong answer
(feet box wrong when off by more than 5% of the figure's height, figure box when IoU &lt; 0.6). Smooth pictures first, clean ones last.</p>
<div class="grid">{cards}</div></body></html>"#
    );
    let doubts = review.join("doubts.html");
    if doubts.exists() {
        html = html.replace("<!--DOUBTS-->", &std::fs::read_to_string(doubts).unwrap());
    }
    std::fs::write(review.join("index.html"), html).unwrap();
    println!(
        "{shown} cards, {wrong_n} with a wrong answer; {}",
        review.join("index.html").display()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("cases") if args.len() == 2 => cases(Path::new(&args[1])),
        Some("report") if args.len() >= 4 => {
            report(Path::new(&args[1]), Path::new(&args[2]), &args[3..])
        }
        _ => {
            for line in include_str!("jev-eval.rs")
                .lines()
                .filter(|l| l.starts_with("//!"))
            {
                println!(
                    "{}",
                    line.trim_start_matches("//!")
                        .strip_prefix(' ')
                        .unwrap_or("")
                );
            }
        }
    }
}
