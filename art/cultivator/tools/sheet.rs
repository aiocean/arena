#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
serde_json = "1"
png = "0.17"
---
//! sheet: builds a fighter's sprite sheets for the duel from a PixelLab character export: every animation's
//! east frames with their pixels as drawn (the arena's pixels are the export's: `pixfx-duel`'s `UNIT`), trimmed,
//! packed one animation per row, with the feet found once on the first
//! idle frame (the export keeps every animation on one canvas, feet in one place). Writes, into <outdir>:
//!   <name>.json            the layout `arena-fighter` reads: each page's size, fps, each animation's cells (page,
//!                          box on it, top-left from the feet) and the portrait box (the head of the first idle frame)
//!   <name>-<school>.png    one sheet per school: the crimson of the robe's lining and aura turned to the
//!                          school's colour (hue and saturation; lightness kept). Rows that do not fit under
//!                          `PAGE` pixels go on further pages, `<name>-<school>-1.png` and on.
//! Sheets of at most 256 colours are written as indexed PNGs, pixel for pixel the same.
//! and, with `--preview <png>`, every sheet at 2x on a dark ground, to look at.
//!
//! The character is a plain sprite: effects are the engine's. `--strip <clip>=cyan|red` (repeatable) takes out an
//! effect the generator drew into an animation (a qi beam, a palm's burst): its colour is erased,
//! a hole it leaves inside the body is filled from the pixels round it, and the specks left floating are dropped.
//!
//! `--keep-colors` writes every school's sheet with the character's own colours: for a character that belongs to
//! one school and wears no crimson accent (the Seven-Colored Daoist's red and violet petals and hems pass the
//! crimson test and would be recoloured). `--head <rows>` sets how many rows from the top of the first idle frame
//! (at the export's size) make the portrait: 30 by default; an HD figure under a tall crown needs more.
//!
//! `--scale <n>` blows every pixel up to an n x n block, for a character drawn smaller than the arena's pixels (a
//! 128-pixel canvas where the arena's fighters stand about 240 pixels tall); an HD export (a 256-pixel canvas) needs
//! none.
//! Usage: cargo -Zscript sheet.rs <animationsdir> <outdir> <name> [--fps 10] [--scale 1] [--strip slash=cyan]...
//! [--preview p.png]
//! The same file serves every character (`art/<character>/tools/sheet.rs`; keep the copies alike, and move it into a
//! crate when a third character needs it). From each character's folder, on its cleaned clips (`src/`, see
//! `art/README.md` step 3):
//!   cultivator (Vương Lâm): `cargo -Zscript tools/sheet.rs src/Idle/animations ../../crates/arena-game/assets/fighters cultivator
//!     --fps 20 --head 64`
//!   Seven-Colored Daoist: `cargo -Zscript tools/sheet.rs src/Idle/animations ../../crates/arena-game/assets/fighters
//!     seven-colored-daoist --fps 20 --keep-colors --head 64 --strip shocked=spark`

use image::{Rgba, RgbaImage};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// The most pixels a page has on a side: what every browser takes in a canvas.
const PAGE: u32 = 4096;

/// The schools and the colour each turns the crimson to: the engine's looks' glow (`pixfx-fighter` look.rs).
const SCHOOLS: &[(&str, [u8; 3])] = &[("thunder", [0xff, 0xe0, 0x66]), ("frost", [0xa6, 0xec, 0xff])];

fn lum(c: &Rgba<u8>) -> u32 {
    299 * c[0] as u32 + 587 * c[1] as u32 + 114 * c[2] as u32
}

/// The colours of an effect drawn into the frames. Cyan: a qi beam's glow, green and blue high over red (the
/// sword's steel is near white, red close to its green and blue). Red: a burst of fire, a bright pure red (the robe's
/// crimson is dark and its green and blue near each other). Spark: an electric spark's teal, green and blue both
/// well over red and near each other (the Daoist's green and blue hems are each far from the other); a spark has no
/// white core, so a white robe next to it stays.
fn effect(kind: &str, c: &Rgba<u8>) -> bool {
    let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
    c[3] > 0
        && match kind {
            "cyan" => g >= 100 && r + 40 <= g && b + 30 >= g,
            "red" => (r >= 180 && g <= 110 && b <= 110) || (r >= 120 && g <= 20 && b <= 20),
            "spark" => r <= 160 && g >= 110 && g >= r + 50 && (g - b).abs() <= 40,
            _ => false,
        }
}

const N8: [(i32, i32); 8] = [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)];

/// The near-white core of a glow: bright and nearly grey.
fn white(c: &Rgba<u8>) -> bool {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    c[3] > 0 && mn >= 170 && mx - mn < 45
}

/// `im` with the effect of colour `kind` taken out: its pixels erased together with the white core they wrap (white
/// pixels reached from them through white pixels with no dark outline next to them: a blade's steel is outlined, a
/// glow's core is not), then each erased pixel with at least five
/// opaque neighbours (inside the body) filled with the colour most of them share, over and over until none is
/// left to fill; last, every group of under 12 pixels not touching the largest group (specks of the effect that are
/// not its colour, a white core, a spark) dropped. Returns the picture and how many pixels it erased.
fn strip(im: &RgbaImage, kind: &str) -> (RgbaImage, usize) {
    let (w, h) = im.dimensions();
    let mut out = im.clone();
    let mut holes: Vec<(u32, u32)> = Vec::new();
    for (x, y, c) in im.enumerate_pixels() {
        if effect(kind, c) {
            out.put_pixel(x, y, Rgba([0; 4]));
            holes.push((x, y));
        }
    }
    let at = |im: &RgbaImage, x: i32, y: i32| {
        (x >= 0 && y >= 0 && x < w as i32 && y < h as i32).then(|| *im.get_pixel(x as u32, y as u32))
    };
    let outlined = |x: i32, y: i32| N8.iter().any(|(dx, dy)| at(im, x + dx, y + dy).is_some_and(|c| c[3] > 0 && lum(&c) < 70_000));
    let mut front = if kind == "spark" { Vec::new() } else { holes.clone() };
    while let Some((x, y)) = front.pop() {
        for (dx, dy) in N8 {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if at(&out, nx, ny).is_some_and(|c| white(&c)) && !outlined(nx, ny) {
                out.put_pixel(nx as u32, ny as u32, Rgba([0; 4]));
                holes.push((nx as u32, ny as u32));
                front.push((nx as u32, ny as u32));
            }
        }
    }
    let erased = holes.len();
    loop {
        let mut filled = Vec::new();
        for &(x, y) in &holes {
            let mut counts: HashMap<Rgba<u8>, u32> = HashMap::new();
            for (dx, dy) in N8 {
                if let Some(c) = at(&out, x as i32 + dx, y as i32 + dy).filter(|c| c[3] > 0) {
                    *counts.entry(c).or_default() += 1;
                }
            }
            if counts.values().sum::<u32>() >= 5 {
                if let Some((c, _)) = counts.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(lum(&b.0).cmp(&lum(&a.0)))) {
                    filled.push((x, y, c));
                }
            }
        }
        if filled.is_empty() {
            break;
        }
        for &(x, y, c) in &filled {
            out.put_pixel(x, y, c);
        }
        holes.retain(|&(x, y)| out.get_pixel(x, y)[3] == 0);
    }
    // groups of opaque pixels, 8-connected; keep the largest and every group of 12 or more
    let mut group = vec![usize::MAX; (w * h) as usize];
    let mut sizes = Vec::new();
    for start in 0..(w * h) as usize {
        if group[start] != usize::MAX || out.get_pixel(start as u32 % w, start as u32 / w)[3] == 0 {
            continue;
        }
        let id = sizes.len();
        let (mut stack, mut n) = (vec![start], 0usize);
        group[start] = id;
        while let Some(i) = stack.pop() {
            n += 1;
            let (x, y) = ((i as u32 % w) as i32, (i as u32 / w) as i32);
            for (dx, dy) in N8 {
                if at(&out, x + dx, y + dy).is_some_and(|c| c[3] > 0) {
                    let j = ((y + dy) as u32 * w + (x + dx) as u32) as usize;
                    if group[j] == usize::MAX {
                        group[j] = id;
                        stack.push(j);
                    }
                }
            }
        }
        sizes.push(n);
    }
    let main = sizes.iter().enumerate().max_by_key(|(_, n)| **n).map(|(i, _)| i);
    for (i, g) in group.iter().enumerate() {
        if *g != usize::MAX && Some(*g) != main && sizes[*g] < 12 {
            out.put_pixel(i as u32 % w, i as u32 / w, Rgba([0; 4]));
        }
    }
    (out, erased)
}

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

/// The robe's crimson: red over green, blue at least green (a purplish red); skin, whose blue is under its
/// green, stays.
fn crimson(c: &Rgba<u8>) -> bool {
    let (r, g, b) = (c[0] as i32, c[1] as i32, c[2] as i32);
    c[3] > 0 && r > g + 25 && b + 4 >= g && r >= b
}

fn to_hsl(c: [u8; 3]) -> (f64, f64, f64) {
    let [r, g, b] = c.map(|v| v as f64 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let l = (mx + mn) / 2.0;
    if mx == mn {
        return (0.0, 0.0, l);
    }
    let d = mx - mn;
    let s = if l > 0.5 { d / (2.0 - mx - mn) } else { d / (mx + mn) };
    let h = if mx == r { (g - b) / d + if g < b { 6.0 } else { 0.0 } } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (h / 6.0, s, l)
}

fn from_hsl(h: f64, s: f64, l: f64) -> [u8; 3] {
    if s == 0.0 {
        let v = (l * 255.0).round() as u8;
        return [v; 3];
    }
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let f = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round() as u8
    };
    [f(h + 1.0 / 3.0), f(h), f(h - 1.0 / 3.0)]
}

fn recolor(im: &RgbaImage, to: [u8; 3]) -> RgbaImage {
    let (th, ts, _) = to_hsl(to);
    let mut out = im.clone();
    for c in out.pixels_mut() {
        if crimson(c) {
            let (_, s, l) = to_hsl([c[0], c[1], c[2]]);
            let [r, g, b] = from_hsl(th, (ts * 0.6 + s * 0.4).min(1.0), l);
            *c = Rgba([r, g, b, c[3]]);
        }
    }
    out
}

/// Writes `im` as an indexed PNG when it has at most 256 colours (every pixel kept exactly, transparency in the
/// palette's alpha), else as RGBA: a fighter's few dozen colours make a sheet a fraction of the RGBA size.
fn save_png(im: &RgbaImage, path: &Path) -> Res<()> {
    let mut index: HashMap<[u8; 4], u8> = HashMap::new();
    let mut idx = Vec::with_capacity((im.width() * im.height()) as usize);
    for c in im.pixels() {
        let key = if c[3] == 0 { [0; 4] } else { c.0 };
        let next = index.len();
        if next == 256 && !index.contains_key(&key) {
            return Ok(im.save(path)?);
        }
        idx.push(*index.entry(key).or_insert(next as u8));
    }
    let mut colours = vec![[0u8; 4]; index.len()];
    for (c, i) in &index {
        colours[*i as usize] = *c;
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(path)?), im.width(), im.height());
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::Best);
    enc.set_palette(colours.iter().flat_map(|c| [c[0], c[1], c[2]]).collect::<Vec<u8>>());
    enc.set_trns(colours.iter().map(|c| c[3]).collect::<Vec<u8>>());
    enc.write_header()?.write_image_data(&idx)?;
    Ok(())
}

fn main() -> Res<()> {
    let usage = "usage: sheet.rs <animationsdir> <outdir> <name> [--fps 10] [--scale 1] [--head 30] [--keep-colors] [--strip clip=cyan|red|spark]... [--preview p.png]";
    let args: Vec<String> = std::env::args().collect();
    let (Some(dir), Some(out), Some(name)) = (args.get(1), args.get(2), args.get(3)) else {
        return Err(usage.into());
    };
    let (mut fps, mut strips, mut preview) = (10.0f64, HashMap::<String, String>::new(), None::<String>);
    let mut scale = 1u32;
    let (mut head_rows, mut keep_colors) = (30u32, false);
    let mut rest = args.iter().skip(4);
    while let Some(flag) = rest.next() {
        if flag == "--keep-colors" {
            keep_colors = true;
            continue;
        }
        let value = rest.next().ok_or(usage)?;
        match flag.as_str() {
            "--head" => head_rows = value.parse::<u32>()?.max(1),
            "--fps" => fps = value.parse()?,
            "--scale" => scale = value.parse::<u32>()?.max(1),
            "--strip" => {
                let (clip, kind) = value.split_once('=').ok_or(usage)?;
                if !matches!(kind, "cyan" | "red" | "spark") {
                    return Err(format!("--strip {value}: the colour is cyan, red or spark").into());
                }
                strips.insert(clip.to_owned(), kind.to_owned());
            }
            "--preview" => preview = Some(value.clone()),
            _ => return Err(usage.into()),
        }
    }

    // every animation's east frames, in name order
    let mut clips: BTreeMap<String, Vec<RgbaImage>> = BTreeMap::new();
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path().join("east");
        let Some(clip) = path.parent().and_then(|p| p.file_name()).and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        if !path.is_dir() {
            continue;
        }
        let mut files: Vec<_> = std::fs::read_dir(&path)?.filter_map(|e| e.ok().map(|e| e.path())).collect();
        files.retain(|p| p.extension().is_some_and(|x| x == "png"));
        files.sort();
        let mut frames = Vec::new();
        for p in &files {
            let mut im = image::open(p)?.to_rgba8();
            if let Some(kind) = strips.get(&clip) {
                let (clean, erased) = strip(&im, kind);
                if erased > 0 {
                    println!("{clip}/{}: {kind} effect, {erased} pixels taken out", p.file_name().and_then(|n| n.to_str()).unwrap_or("?"));
                }
                im = clean;
            }
            if scale > 1 {
                im = image::imageops::resize(&im, im.width() * scale, im.height() * scale, image::imageops::FilterType::Nearest);
            }
            frames.push(im);
        }
        clips.insert(clip, frames);
    }
    let idle = clips.get("stance-idle").and_then(|f| f.first()).ok_or("no stance-idle frames")?;
    let (x0, y0, x1, y1) = bbox(idle).ok_or("the first idle frame is empty")?;
    let feet = ((x0 + x1) as i32 / 2, y1 as i32);

    // pack: one row per animation, a new page when a row would pass the page's foot
    let mut cells: BTreeMap<String, Vec<serde_json::Value>> = BTreeMap::new();
    let mut placed: Vec<(RgbaImage, usize, u32, u32)> = Vec::new();
    // each page's width and height
    let mut sizes: Vec<(u32, u32)> = vec![(0, 0)];
    for (clip, frames) in &clips {
        let pieces: Vec<_> = frames
            .iter()
            .filter_map(|im| bbox(im).map(|(a, b, c, d)| (im, a, b, c, d)))
            .collect();
        let row_w: u32 = pieces.iter().map(|(_, a, _, c, _)| c - a + 1).sum();
        let row_h = pieces.iter().map(|(_, _, b, _, d)| d - b).max().unwrap_or(0);
        if row_w > PAGE || row_h > PAGE {
            return Err(format!("{clip}: a row of {row_w}x{row_h} does not fit on a {PAGE}-pixel page").into());
        }
        if sizes.last().is_some_and(|&(_, h)| h > 0 && h + row_h > PAGE) {
            sizes.push((0, 0));
        }
        let page = sizes.len() - 1;
        let (width, y) = sizes[page];
        let mut x = 0u32;
        for (im, a, b, c, d) in pieces {
            let piece = image::imageops::crop_imm(im, a, b, c - a, d - b).to_image();
            cells.entry(clip.clone()).or_default().push(json!({
                "page": page, "x": x, "y": y, "w": c - a, "h": d - b, "dx": a as i32 - feet.0, "dy": b as i32 - feet.1,
            }));
            placed.push((piece, page, x, y));
            x += c - a + 1;
        }
        sizes[page] = (width.max(x), y + row_h + 1);
    }
    let mut pages: Vec<RgbaImage> = sizes.iter().map(|&(w, h)| RgbaImage::new(w, h)).collect();
    for (piece, page, x, y) in &placed {
        image::imageops::replace(&mut pages[*page], piece, *x as i64, *y as i64);
    }

    // the portrait: the head, the first 30 rows (at the export's size) of the first idle frame, where it sits on the sheet
    let first = cells.get("stance-idle").and_then(|c| c.first()).ok_or("no idle cell")?;
    let (sx, sy) = (first["x"].as_u64().unwrap_or(0) as u32, first["y"].as_u64().unwrap_or(0) as u32);
    let sp = first["page"].as_u64().unwrap_or(0);
    let head = image::imageops::crop_imm(idle, x0, y0, x1 - x0, (head_rows * scale).min(y1 - y0)).to_image();
    let (hx0, hy0, hx1, hy1) = bbox(&head).ok_or("no head")?;
    let portrait = json!({"page": sp, "x": sx + hx0, "y": sy + hy0, "w": hx1 - hx0, "h": hy1 - hy0, "dx": 0, "dy": 0});

    std::fs::create_dir_all(out)?;
    let page_sizes: Vec<[u32; 2]> = sizes.iter().map(|&(w, h)| [w, h]).collect();
    let layout = json!({"pages": page_sizes, "fps": fps, "clips": cells, "portrait": portrait});
    std::fs::write(Path::new(out).join(format!("{name}.json")), serde_json::to_string(&layout)?)?;
    // each school's pages, the first page of each kept for the preview
    let mut sheets = Vec::new();
    for (school, to) in SCHOOLS {
        for (i, page) in pages.iter().enumerate() {
            let s = if keep_colors { page.clone() } else { recolor(page, *to) };
            let file = if i == 0 { format!("{name}-{school}.png") } else { format!("{name}-{school}-{i}.png") };
            save_png(&s, &Path::new(out).join(file))?;
            if i == 0 {
                sheets.push(s);
            }
        }
    }
    let (width, y) = sizes[0];
    for clip in strips.keys() {
        if !clips.contains_key(clip) {
            return Err(format!("--strip {clip}: no such animation").into());
        }
    }
    // preview: the first page of each sheet side by side at 2x
    let k = 2;
    let mut prev = RgbaImage::from_pixel(width * k * 2 + k * 4, y * k, Rgba([28, 30, 38, 255]));
    for (i, s) in sheets.iter().enumerate() {
        let off = i as u32 * (width * k + k * 4);
        for (px, py, c) in s.enumerate_pixels() {
            if c[3] == 0 {
                continue;
            }
            for v in 0..k {
                for u in 0..k {
                    prev.put_pixel(off + px * k + u, py * k + v, *c);
                }
            }
        }
    }
    if let Some(p) = preview {
        prev.save(p)?;
    }
    println!(
        "{} animations, {} frames; {} page(s), the first {width}x{y}; feet at {:?} on a {}x{} frame; figure {}px tall",
        clips.len(),
        placed.len(),
        pages.len(),
        feet,
        idle.width(),
        idle.height(),
        y1 - y0
    );
    Ok(())
}
