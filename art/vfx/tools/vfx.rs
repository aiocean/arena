#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[profile.dev]
opt-level = 2

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
webp = "0.3"
serde_json = { version = "1", features = ["preserve_order"] }
---
//! vfx: builds a spell effect's sprite sheet for the duel (`crates/arena-vfx`) from its frames: one folder per
//! school the effect is drawn for (`thunder/`, `frost/`) or one for any (`any/`), each holding its frames as PNGs in
//! name order, every frame on one canvas, or in place of a folder that school's spritesheet (`thunder.json`, the PNG
//! beside it); or a single spritesheet's JSON, drawn for any school. A spritesheet is TexturePacker's, as PixelLab's
//! VFX export writes it, anchored at its pivot. Blank frames at the end are dropped. Frames
//! are trimmed and packed one animation per row, on pages under `PAGE`
//! pixels a side; each frame's top-left is measured from the anchor, the point the rules play the effect at: the
//! canvas's centre, or the middle of its foot (`--anchor feet`, an effect that stands on the ground). Writes, into
//! <outdir>:
//!   <file>.json        the layout: each page's size, fps, whether it loops, each school's frames
//!   <file>.webp        its first page, then <file>-1.webp and on (lossy; lossless for a sheet with no `px`, pixel
//!                      art drawn a pixel of the frame a pixel)
//! and adds the effect to <outdir>/index.json (`[{ "fx": <fx>, "file": <file> }, ...]`), which the page reads.
//! `<file>` is the effect's id with `/` turned to `-`.
//!
//! How it moves is code (`crates/arena-vfx`, `Motion`), written once beside the effect's frames: `motion.json` in
//! the folder that holds <framesdir> (or the spritesheet), copied into the layout as it is: `turns`, `fly`, `tail`,
//! `exit` (`art/vfx/swirl-blast/motion.json` is the pattern). A sheet drawn finer than `FINEST` world pixels a
//! pixel (its `px`) is shrunk to it: the finer pixels are not seen once the effect is drawn, only downloaded.
//!
//! Usage: cargo -Zscript vfx.rs <framesdir | sheet.json> <outdir> <fx> [--fps 12] [--loop] [--anchor centre|feet]
//! [--scale 1]
//! e.g. `cargo -Zscript art/vfx/tools/vfx.rs art/vfx/laser-bolt/frames site/assets/vfx projectiles/laser-bolt --loop`
//!
//! Or `vfx.rs --all <artdir> <outdir>`: every effect under <artdir> (`art/vfx`) that has a `motion.json`, built
//! from its `frames/` with what its `motion.json` says beside the motion: `fx`, `fps`, `loop` and `anchor` (the
//! anchor's pixel on the frames' canvas); then `drawn.json` in <outdir>, every effect's id, file and layout in one
//! file, which the game reads (`crates/arena-game`, `vfx::drawn`).
//! e.g. `cargo -Zscript art/vfx/tools/vfx.rs --all art/vfx crates/arena-game/assets/drawn`
//!
//! Or `vfx.rs --repack <outdir>`: every effect in <outdir>'s index still on PNG pages, cut back into its frames from
//! its own layout and written again as above (shrunk, WebP), its PNGs removed.

use image::RgbaImage;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The most pixels a page has on a side: what every browser takes in a canvas.
const PAGE: u32 = 4096;
/// The schools an effect can be drawn for, and "any".
const SCHOOLS: [&str; 3] = ["thunder", "frost", "any"];
/// The finest a sheet is kept, in world pixels a sheet pixel (`px`): a pixel of the screen at the game's scale (four
/// screen pixels a world pixel), so the drawing stays smooth where it is seen.
const FINEST: f64 = 0.25;
/// The lossy pages' quality, of their colour and of their alpha.
const QUALITY: (f32, i32) = (75.0, 80);

/// The box of the opaque pixels: x0, y0, x1, y1 (exclusive); none for an empty picture.
fn bbox(im: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut b: Option<(u32, u32, u32, u32)> = None;
    for (x, y, c) in im.enumerate_pixels() {
        if c[3] > 0 {
            b = Some(match b {
                None => (x, y, x + 1, y + 1),
                Some((a, bb, cc, d)) => (a.min(x), bb.min(y), cc.max(x + 1), d.max(y + 1)),
            });
        }
    }
    b
}

/// A spritesheet's frames, each on its full canvas, and its first frame's pivot (fractions of the canvas).
fn read_sheet(path: &Path, scale: u32) -> Res<(Vec<RgbaImage>, Option<(f64, f64)>)> {
    let meta: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
    let sheet = image::open(path.with_extension("png"))?.to_rgba8();
    let (mut frames, mut pivot) = (Vec::new(), None);
    for f in meta["frames"].as_array().ok_or(format!("{}: no frames", path.display()))? {
        let n = |v: &Value| v.as_u64().unwrap_or(0) as u32;
        let (r, s, size) = (&f["frame"], &f["spriteSourceSize"], &f["sourceSize"]);
        let piece = image::imageops::crop_imm(&sheet, n(&r["x"]), n(&r["y"]), n(&r["w"]), n(&r["h"])).to_image();
        let mut canvas = RgbaImage::new(n(&size["w"]), n(&size["h"]));
        image::imageops::replace(&mut canvas, &piece, n(&s["x"]) as i64, n(&s["y"]) as i64);
        pivot = pivot.or_else(|| Some((f["pivot"]["x"].as_f64()?, f["pivot"]["y"].as_f64()?)));
        if scale > 1 {
            canvas = image::imageops::resize(&canvas, canvas.width() * scale, canvas.height() * scale, image::imageops::FilterType::Nearest);
        }
        frames.push(canvas);
    }
    Ok((frames, pivot))
}

fn main() -> Res<()> {
    let usage = "usage: vfx.rs <framesdir> <outdir> <fx> [--fps 12] [--loop] [--anchor centre|feet] [--scale 1]";
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--repack") {
        return repack(args.get(2).ok_or(usage)?);
    }
    if args.get(1).map(String::as_str) == Some("--all") {
        return all(args.get(2).ok_or(usage)?, args.get(3).ok_or(usage)?);
    }
    let (Some(dir), Some(out), Some(fx)) = (args.get(1), args.get(2), args.get(3)) else {
        return Err(usage.into());
    };
    let (mut fps, mut looping, mut feet, mut scale) = (12.0f64, false, false, 1u32);
    let mut rest = args.iter().skip(4);
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--loop" => looping = true,
            "--fps" => fps = rest.next().ok_or(usage)?.parse()?,
            "--scale" => scale = rest.next().ok_or(usage)?.parse::<u32>()?.max(1),
            "--anchor" => match rest.next().map(String::as_str) {
                Some("centre" | "center") => feet = false,
                Some("feet") => feet = true,
                _ => return Err(usage.into()),
            },
            _ => return Err(usage.into()),
        }
    }

    // each school's frames, in name order or from its spritesheet (or one spritesheet's, for any school), and the
    // anchor if a spritesheet gives one
    let mut clips: BTreeMap<String, Vec<RgbaImage>> = BTreeMap::new();
    let mut pivot: Option<(f64, f64)> = None;
    let sheet_json = dir.ends_with(".json");
    if sheet_json {
        let (frames, p) = read_sheet(Path::new(dir), scale)?;
        pivot = p;
        clips.insert("any".to_owned(), frames);
    }
    for school in SCHOOLS.iter().filter(|_| !sheet_json) {
        let sheet = Path::new(dir).join(school).with_extension("json");
        if sheet.is_file() {
            let (frames, p) = read_sheet(&sheet, scale)?;
            pivot = pivot.or(p);
            clips.insert((*school).to_owned(), frames);
        }
    }
    if !sheet_json {
        for (school, frames) in folders(Path::new(dir), scale)? {
            clips.entry(school).or_insert(frames);
        }
    }
    let first = clips.values().next().and_then(|f| f.first()).ok_or(format!("{dir}: no thunder/, frost/ or any/ frames"))?;
    let (cw, ch) = first.dimensions();
    for (school, frames) in &clips {
        if frames.iter().any(|f| f.dimensions() != (cw, ch)) {
            return Err(format!("{school}: every frame must share one {cw}x{ch} canvas").into());
        }
    }
    let anchor = match pivot {
        Some((px, py)) if !feet => ((px * f64::from(cw)).round() as i32, (py * f64::from(ch)).round() as i32),
        _ => (cw as i32 / 2, if feet { ch as i32 } else { ch as i32 / 2 }),
    };

    // how it moves, from the effect's own folder
    let motion = Path::new(dir).parent().map(|p| p.join("motion.json")).filter(|p| p.is_file());
    let motion: Option<Value> = match &motion {
        Some(path) => Some(serde_json::from_str(&std::fs::read_to_string(path)?)?),
        None => None,
    };
    build(out, fx, clips, anchor, fps, looping, motion)
}

/// Each school's frames in `dir` (`thunder/`, `frost/`, `any/`), PNGs in name order, blown up `scale` times; blank
/// frames at the end dropped.
fn folders(dir: &Path, scale: u32) -> Res<BTreeMap<String, Vec<RgbaImage>>> {
    let mut clips = BTreeMap::new();
    for school in SCHOOLS {
        let path = dir.join(school);
        if !path.is_dir() {
            continue;
        }
        let mut files: Vec<_> = std::fs::read_dir(&path)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.retain(|p| p.extension().is_some_and(|x| x == "png"));
        files.sort();
        let mut frames = Vec::new();
        for p in &files {
            let mut im = image::open(p)?.to_rgba8();
            if scale > 1 {
                im = image::imageops::resize(&im, im.width() * scale, im.height() * scale, image::imageops::FilterType::Nearest);
            }
            frames.push(im);
        }
        while frames.len() > 1 && frames.last().is_some_and(|f| bbox(f).is_none()) {
            frames.pop();
        }
        if !frames.is_empty() {
            clips.insert(school.to_owned(), frames);
        }
    }
    Ok(clips)
}

/// Every effect under `art` with a `motion.json`, built into `out`; then `out/drawn.json`.
fn all(art: &str, out: &str) -> Res<()> {
    let mut dirs: Vec<_> = std::fs::read_dir(art)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
    dirs.retain(|d| d.join("motion.json").is_file());
    dirs.sort();
    for dir in &dirs {
        let mut motion: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("motion.json"))?)?;
        let name = dir.display().to_string();
        let fx = motion["fx"].as_str().ok_or(format!("{name}: motion.json names no `fx`"))?.to_owned();
        let fps = motion["fps"].as_f64().unwrap_or(12.0);
        let looping = motion["loop"].as_bool().unwrap_or(false);
        let anchor = match motion["anchor"].as_array().map(Vec::as_slice) {
            Some([x, y]) => (x.as_i64().unwrap_or(0) as i32, y.as_i64().unwrap_or(0) as i32),
            _ => return Err(format!("{name}: motion.json gives no `anchor` [x, y]").into()),
        };
        // the rest is how it moves, which the layout carries
        if let Some(m) = motion.as_object_mut() {
            for key in ["fx", "fps", "loop", "anchor"] {
                m.remove(key);
            }
        }
        let clips = folders(&dir.join("frames"), 1)?;
        if clips.is_empty() {
            return Err(format!("{name}: no frames").into());
        }
        build(out, &fx, clips, anchor, fps, looping, Some(motion))?;
    }
    // every effect in one file, for the game
    let index: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(Path::new(out).join("index.json"))?)?;
    let mut drawn = Vec::new();
    for e in &index {
        let file = e["file"].as_str().unwrap_or_default();
        let layout: Value = serde_json::from_str(&std::fs::read_to_string(Path::new(out).join(format!("{file}.json")))?)?;
        drawn.push(json!({ "fx": e["fx"], "file": file, "layout": layout }));
    }
    std::fs::write(Path::new(out).join("drawn.json"), serde_json::to_string(&drawn)? + "\n")?;
    println!("{} effects into {out}/drawn.json", drawn.len());
    Ok(())
}

/// The effect's frames packed onto pages and written into `out`, with its layout, and added to the index: shrunk
/// first to `FINEST` when drawn finer.
fn build(
    out: &str,
    fx: &str,
    mut clips: BTreeMap<String, Vec<RgbaImage>>,
    mut anchor: (i32, i32),
    fps: f64,
    looping: bool,
    mut motion: Option<Value>,
) -> Res<()> {
    let px = motion.as_ref().and_then(|m| m["px"].as_f64());
    if let Some(p) = px.filter(|&p| p < FINEST) {
        let f = p / FINEST;
        for frames in clips.values_mut() {
            for im in frames.iter_mut() {
                *im = shrink(im, f);
            }
        }
        anchor = ((f64::from(anchor.0) * f).round() as i32, (f64::from(anchor.1) * f).round() as i32);
        if let Some(m) = motion.as_mut() {
            m["px"] = json!(FINEST);
        }
    }
    // a sheet with no `px` is pixel art, a pixel of the frame a pixel: kept exact
    let lossless = px.is_none();
    let (cw, ch) = clips.values().next().and_then(|f| f.first()).map(RgbaImage::dimensions).unwrap_or((0, 0));

    // pack: shelf by shelf per school, wrapping to a new row (or page) when a frame would pass PAGE
    let mut cells: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut placed: Vec<(RgbaImage, usize, u32, u32)> = Vec::new();
    let mut sizes: Vec<(u32, u32)> = vec![(0, 0)];
    for (school, frames) in &clips {
        let pieces: Vec<_> = frames.iter().map(|im| (im, bbox(im).unwrap_or((0, 0, 1, 1)))).collect();
        let mut x = 0u32;
        let mut shelf = 0u32;
        for (im, (a, b, c, d)) in pieces {
            let (pw, ph) = (c - a, d - b);
            if pw > PAGE || ph > PAGE {
                return Err(format!("{school}: a frame of {pw}x{ph} does not fit on a {PAGE}-pixel page").into());
            }
            if x > 0 && x + pw > PAGE {
                let page = sizes.len() - 1;
                sizes[page] = (sizes[page].0.max(x), sizes[page].1 + shelf + 1);
                x = 0;
                shelf = 0;
            }
            if sizes.last().is_some_and(|&(_, h)| h > 0 && h + ph > PAGE) {
                sizes.push((0, 0));
                x = 0;
                shelf = 0;
            }
            let page = sizes.len() - 1;
            let y = sizes[page].1;
            let piece = image::imageops::crop_imm(im, a, b, pw, ph).to_image();
            cells.entry(school.clone()).or_default().push(json!({
                "page": page, "x": x, "y": y, "w": pw, "h": ph,
                "dx": a as i32 - anchor.0, "dy": b as i32 - anchor.1,
            }));
            placed.push((piece, page, x, y));
            x += pw + 1;
            shelf = shelf.max(ph);
        }
        if x > 0 || shelf > 0 {
            let page = sizes.len() - 1;
            sizes[page] = (sizes[page].0.max(x), sizes[page].1 + shelf + 1);
        }
    }
    let mut pages: Vec<RgbaImage> = sizes.iter().map(|&(w, h)| RgbaImage::new(w.max(1), h.max(1))).collect();
    for (piece, page, x, y) in &placed {
        image::imageops::replace(&mut pages[*page], piece, *x as i64, *y as i64);
    }

    std::fs::create_dir_all(out)?;
    let file = fx.replace('/', "-");
    let page_sizes: Vec<[u32; 2]> = pages.iter().map(|p| [p.width(), p.height()]).collect();
    let mut layout = json!({ "pages": page_sizes, "fps": fps, "looping": looping, "clips": cells });
    if let Some(motion) = motion {
        layout["motion"] = motion;
    }
    std::fs::write(Path::new(out).join(format!("{file}.json")), serde_json::to_string(&layout)?)?;
    let page_name = |i: usize, ext: &str| if i == 0 { format!("{file}.{ext}") } else { format!("{file}-{i}.{ext}") };
    for (i, page) in pages.iter().enumerate() {
        std::fs::write(Path::new(out).join(page_name(i, "webp")), encode(page, lossless)?)?;
    }
    // the pages an older build left: PNGs, and WebPs past the last
    for i in 0.. {
        let (png, webp) = (Path::new(out).join(page_name(i, "png")), Path::new(out).join(page_name(i, "webp")));
        let stale_webp = i >= pages.len() && webp.is_file();
        if !png.is_file() && !stale_webp {
            break;
        }
        let _ = std::fs::remove_file(&png);
        if stale_webp {
            std::fs::remove_file(&webp)?;
        }
    }
    // the index: this effect added, or its entry kept where it was; locked, as several effects may be built at once
    let lock = std::fs::File::create(Path::new(out).join(".index.lock"))?;
    lock.lock()?;
    let index_path = Path::new(out).join("index.json");
    let mut index: Vec<Value> = match std::fs::read_to_string(&index_path) {
        Ok(s) => serde_json::from_str(&s)?,
        Err(_) => Vec::new(),
    };
    index.retain(|e| e["fx"] != json!(fx));
    index.push(json!({ "fx": fx, "file": file }));
    index.sort_by(|a, b| a["fx"].as_str().cmp(&b["fx"].as_str()));
    std::fs::write(&index_path, serde_json::to_string_pretty(&index)? + "\n")?;
    println!(
        "{fx}: {} school(s), {} frames on a {cw}x{ch} canvas; {} page(s); anchor {:?}; {} fps{}",
        clips.len(),
        placed.len(),
        pages.len(),
        anchor,
        fps,
        if looping { ", looping" } else { "" }
    );
    Ok(())
}

/// `im` shrunk by `f`, resampled with its colour premultiplied so the clear pixels around it do not darken its edge.
fn shrink(im: &RgbaImage, f: f64) -> RgbaImage {
    let mut pre = im.clone();
    for p in pre.pixels_mut() {
        let a = u16::from(p[3]);
        for c in 0..3 {
            p[c] = ((u16::from(p[c]) * a + 127) / 255) as u8;
        }
    }
    let (w, h) = ((f64::from(im.width()) * f).round().max(1.0) as u32, (f64::from(im.height()) * f).round().max(1.0) as u32);
    let mut out = image::imageops::resize(&pre, w, h, image::imageops::FilterType::CatmullRom);
    for p in out.pixels_mut() {
        let a = u16::from(p[3]);
        for c in 0..3 {
            p[c] = if a == 0 { 0 } else { ((u16::from(p[c]) * 255 + a / 2) / a).min(255) as u8 };
        }
    }
    out
}

/// A page as WebP: lossless, or lossy at `QUALITY`.
fn encode(page: &RgbaImage, lossless: bool) -> Res<Vec<u8>> {
    let mut config = webp::WebPConfig::new().map_err(|_| "webp: no config")?;
    config.lossless = i32::from(lossless);
    config.method = 6;
    if lossless {
        config.quality = 100.0;
    } else {
        (config.quality, config.alpha_quality) = QUALITY;
        config.use_sharp_yuv = 1;
    }
    let encoder = webp::Encoder::from_rgba(page.as_raw(), page.width(), page.height());
    Ok(encoder.encode_advanced(&config).map_err(|e| format!("webp: {e:?}"))?.to_vec())
}

/// Every effect in `out`'s index still on PNG pages, cut back into its frames from its layout and built again.
fn repack(out: &str) -> Res<()> {
    let index: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(Path::new(out).join("index.json"))?)?;
    for entry in &index {
        let (Some(fx), Some(file)) = (entry["fx"].as_str(), entry["file"].as_str()) else { continue };
        if !Path::new(out).join(format!("{file}.png")).is_file() {
            continue;
        }
        let layout: Value = serde_json::from_str(&std::fs::read_to_string(Path::new(out).join(format!("{file}.json")))?)?;
        let count = layout["pages"].as_array().map_or(1, Vec::len);
        let pages = (0..count)
            .map(|i| {
                let name = if i == 0 { format!("{file}.png") } else { format!("{file}-{i}.png") };
                Ok(image::open(Path::new(out).join(name))?.to_rgba8())
            })
            .collect::<Res<Vec<_>>>()?;
        let n = |v: &Value| v.as_i64().unwrap_or(0) as i32;
        let cells: Vec<&Value> = layout["clips"].as_object().into_iter().flat_map(|c| c.values()).flat_map(|v| v.as_array().into_iter().flatten()).collect();
        let x0 = cells.iter().map(|c| n(&c["dx"])).min().unwrap_or(0);
        let y0 = cells.iter().map(|c| n(&c["dy"])).min().unwrap_or(0);
        let x1 = cells.iter().map(|c| n(&c["dx"]) + n(&c["w"])).max().unwrap_or(1);
        let y1 = cells.iter().map(|c| n(&c["dy"]) + n(&c["h"])).max().unwrap_or(1);
        let mut clips: BTreeMap<String, Vec<RgbaImage>> = BTreeMap::new();
        for (school, frames) in layout["clips"].as_object().ok_or(format!("{file}: no clips"))? {
            for c in frames.as_array().into_iter().flatten() {
                let page = pages.get(c["page"].as_u64().unwrap_or(0) as usize).ok_or(format!("{file}: no page"))?;
                let piece = image::imageops::crop_imm(page, n(&c["x"]) as u32, n(&c["y"]) as u32, n(&c["w"]) as u32, n(&c["h"]) as u32).to_image();
                let mut canvas = RgbaImage::new((x1 - x0) as u32, (y1 - y0) as u32);
                image::imageops::replace(&mut canvas, &piece, i64::from(n(&c["dx"]) - x0), i64::from(n(&c["dy"]) - y0));
                clips.entry(school.clone()).or_default().push(canvas);
            }
        }
        let fps = layout["fps"].as_f64().unwrap_or(12.0);
        let looping = layout["looping"].as_bool().unwrap_or(false);
        build(out, fx, clips, (-x0, -y0), fps, looping, layout.get("motion").cloned())?;
    }
    Ok(())
}
