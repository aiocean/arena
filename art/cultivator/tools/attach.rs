#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
serde_json = "1"
---
//! attach: measures a held item's attach point on every frame by subtracting
//! the animation drawn empty-handed from the same animation drawn holding the
//! item (the game-factory `attach-point` skill, "Two drawings of one
//! animation"). Writes the skill's `<character>.attach.json` shape.
//!
//! crop <framesdir> <outdir> <w> <h>
//!     Crops every frame to w x h around the union of their opaque pixels
//!     (one offset for all, so they still line up), to fit the frame size a
//!     tool takes. Writes `crop.json` (offset, source size) beside the frames:
//!     `diff --crop` uses it to put the points back on the source frame.
//!
//! diff <emptydir> <armeddir> <outdir> [--crop crop.json] [--origin 0.5,1]
//!      [--anim name] [--point hand_r] [--tol 48]
//!     Per frame, the item is the largest connected group of pixels the armed
//!     frame has and the empty one lacks or colours differently (sum of RGB
//!     distances over `tol`). Its axis is its longest span. The grip is where
//!     it meets the hand: the empty frame's skin-coloured group nearest the
//!     axis's near end, projected onto the axis; with no skin within reach it
//!     falls back to the near end (reported as "end"). The angle points from
//!     the grip to the far end: degrees, 0 = right, clockwise positive (y
//!     down). The item is behind the body when it covers no body pixel.
//!     Writes `attach.json` (points relative to the origin, on the source
//!     frame when --crop is given), `report.json` (per-frame diagnostics: how
//!     many body pixels changed outside the item, which says whether the edit
//!     kept the body), `layer/` (the item alone per frame) and `check.png`
//!     (empty | armed | item | overlay at 4x: body grey, item green, other
//!     changed pixels magenta, grip red).
//!
//! Usage: cargo -Zscript attach.rs crop|diff ...

use image::{Rgba, RgbaImage};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type Res<T> = Result<T, Box<dyn std::error::Error>>;
type Px = (u32, u32);

fn frames(dir: &str) -> Res<Vec<PathBuf>> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "png"))
        .collect();
    v.sort();
    Ok(v)
}

fn load(p: &Path) -> Res<RgbaImage> {
    Ok(image::open(p)?.to_rgba8())
}

fn crop(dir: &str, out: &str, w: u32, h: u32) -> Res<()> {
    let files = frames(dir)?;
    let imgs: Vec<RgbaImage> = files.iter().map(|p| load(p)).collect::<Res<_>>()?;
    let first = imgs.first().ok_or("no frames")?;
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0, 0);
    for im in &imgs {
        for (x, y, c) in im.enumerate_pixels() {
            if c[3] > 0 {
                (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
            }
        }
    }
    let (bw, bh) = (x1 + 1 - x0, y1 + 1 - y0);
    if bw > w || bh > h {
        return Err(format!("art spans {bw}x{bh}, more than {w}x{h}").into());
    }
    let (sw, sh) = first.dimensions();
    let ox = (x0 + bw / 2).saturating_sub(w / 2).min(sw - w);
    let oy = (y0 + bh / 2).saturating_sub(h / 2).min(sh - h);
    std::fs::create_dir_all(out)?;
    for (p, im) in files.iter().zip(&imgs) {
        let c = image::imageops::crop_imm(im, ox, oy, w, h).to_image();
        c.save(Path::new(out).join(p.file_name().ok_or("no name")?))?;
    }
    let meta = json!({"offset": [ox, oy], "source": [sw, sh], "frame": [w, h]});
    std::fs::write(Path::new(out).join("crop.json"), serde_json::to_string_pretty(&meta)?)?;
    println!("art box {x0},{y0} {bw}x{bh}; crop offset {ox},{oy} -> {w}x{h} (crop.json)");
    Ok(())
}

fn dist(a: &Rgba<u8>, b: &Rgba<u8>) -> u32 {
    (0..3).map(|i| (a[i] as i32 - b[i] as i32).unsigned_abs()).sum()
}

fn skin(c: &Rgba<u8>) -> bool {
    let lum = 0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64;
    c[3] > 0 && c[0] > c[1] && c[0] as i32 - c[2] as i32 > 25 && lum > 95.0
}

/// 8-connected groups of the cells `on` marks.
fn groups(w: u32, h: u32, on: &[bool]) -> Vec<Vec<Px>> {
    let mut seen = vec![false; on.len()];
    let mut out = Vec::new();
    for s in 0..on.len() {
        if !on[s] || seen[s] {
            continue;
        }
        seen[s] = true;
        let mut stack = vec![s];
        let mut group = Vec::new();
        while let Some(i) = stack.pop() {
            let (x, y) = (i as u32 % w, i as u32 / w);
            group.push((x, y));
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = (ny as u32 * w + nx as u32) as usize;
                    if on[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        out.push(group);
    }
    out
}

fn centroid(px: &[Px]) -> (f64, f64) {
    let n = px.len().max(1) as f64;
    let (sx, sy) = px.iter().fold((0.0, 0.0), |(a, b), p| (a + p.0 as f64, b + p.1 as f64));
    (sx / n, sy / n)
}

fn d(a: (f64, f64), b: (f64, f64)) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

struct Socket {
    grip: (f64, f64),
    tip: Px,
    angle: f64,
    length: f64,
    by: &'static str,
}

/// The item's axis, which end is held, and where on it the hand is.
fn socket(item: &[Px], body: &RgbaImage) -> Option<Socket> {
    let mut far = 0i64;
    let mut axis = None;
    for (k, p) in item.iter().enumerate() {
        for q in &item[k + 1..] {
            let dd = (p.0 as i64 - q.0 as i64).pow(2) + (p.1 as i64 - q.1 as i64).pow(2);
            if dd > far {
                far = dd;
                axis = Some((*p, *q));
            }
        }
    }
    let (p, q) = axis?;
    let opaque: Vec<Px> = body.enumerate_pixels().filter(|(_, _, c)| c[3] > 0).map(|(x, y, _)| (x, y)).collect();
    let mid = centroid(&opaque);
    let f = |a: Px| (a.0 as f64, a.1 as f64);
    let (near, tip) = if d(f(p), mid) <= d(f(q), mid) { (p, q) } else { (q, p) };
    let (w, h) = body.dimensions();
    let on: Vec<bool> = body.pixels().map(skin).collect();
    let length = (far as f64).sqrt();
    // The hand: the skin group nearest the near end, if it is within reach
    // (a third of the item's length), projected onto the axis.
    let hand = groups(w, h, &on)
        .iter()
        .map(|g| centroid(g))
        .map(|c| (d(c, f(near)), c))
        .filter(|(dd, _)| *dd <= length / 3.0)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| c);
    let (ux, uy) = ((tip.0 as f64 - near.0 as f64) / length, (tip.1 as f64 - near.1 as f64) / length);
    let (grip, by) = match hand {
        Some(c) => {
            let t = ((c.0 - near.0 as f64) * ux + (c.1 - near.1 as f64) * uy).clamp(0.0, length);
            ((near.0 as f64 + t * ux, near.1 as f64 + t * uy), "hand")
        }
        None => (f(near), "end"),
    };
    let angle = uy.atan2(ux).to_degrees();
    Some(Socket { grip, tip, angle, length, by })
}

fn paste(sheet: &mut RgbaImage, im: &RgbaImage, col: u32, scale: u32) {
    let off = col * im.width() * scale;
    for (x, y, c) in im.enumerate_pixels() {
        for sy in 0..scale {
            for sx in 0..scale {
                sheet.put_pixel(off + x * scale + sx, y * scale + sy, *c);
            }
        }
    }
}

struct Opts {
    crop: Option<String>,
    origin: (f64, f64),
    anim: String,
    point: String,
    tol: u32,
}

fn diff(empty: &str, armed: &str, out: &str, o: &Opts) -> Res<()> {
    let ef = frames(empty)?;
    let af = frames(armed)?;
    if ef.len() != af.len() {
        return Err(format!("{} empty frames, {} armed", ef.len(), af.len()).into());
    }
    let first = load(ef.first().ok_or("no frames")?)?;
    // Points go back onto the source frame the crop was cut from.
    let (offset, source) = match &o.crop {
        Some(p) => {
            let m: Value = serde_json::from_str(&std::fs::read_to_string(p)?)?;
            let n = |v: &Value, i: usize| v[i].as_f64().ok_or("bad crop.json");
            ((n(&m["offset"], 0)?, n(&m["offset"], 1)?), (n(&m["source"], 0)?, n(&m["source"], 1)?))
        }
        None => ((0.0, 0.0), (first.width() as f64, first.height() as f64)),
    };
    let origin = (o.origin.0 * source.0, o.origin.1 * source.1);
    std::fs::create_dir_all(Path::new(out).join("layer"))?;
    let scale = 4;
    let (mut points, mut behind, mut report, mut rows) = (vec![], vec![], vec![], vec![]);
    for (i, (ep, ap)) in ef.iter().zip(&af).enumerate() {
        let (e, a) = (load(ep)?, load(ap)?);
        let (w, h) = e.dimensions();
        if a.dimensions() != (w, h) {
            return Err(format!("frame {i}: sizes differ").into());
        }
        let changed: Vec<bool> = e
            .pixels()
            .zip(a.pixels())
            .map(|(ec, ac)| ac[3] > 0 && (ec[3] == 0 || dist(ec, ac) > o.tol))
            .collect();
        let item = groups(w, h, &changed).into_iter().max_by_key(Vec::len).unwrap_or_default();
        let mut keep = vec![false; changed.len()];
        for &(x, y) in &item {
            keep[(y * w + x) as usize] = true;
        }
        let covered = item.iter().filter(|&&(x, y)| e.get_pixel(x, y)[3] > 0).count();
        let noise = changed.iter().zip(&keep).filter(|(c, k)| **c && !**k).count();
        let lost = e.pixels().zip(a.pixels()).filter(|(ec, ac)| ec[3] > 0 && ac[3] == 0).count();
        let body = e.pixels().filter(|c| c[3] > 0).count();

        let mut layer = RgbaImage::new(w, h);
        for &(x, y) in &item {
            layer.put_pixel(x, y, *a.get_pixel(x, y));
        }
        layer.save(Path::new(out).join("layer").join(ap.file_name().ok_or("no name")?))?;

        let s = socket(&item, &e).ok_or(format!("frame {i}: no item found"))?;
        let gx = s.grip.0 + offset.0 - origin.0;
        let gy = s.grip.1 + offset.1 - origin.1;
        points.push(json!([gx.round(), gy.round(), s.angle.round()]));
        behind.push(json!(covered == 0));
        report.push(json!({
            "frame": i, "item_pixels": item.len(), "length": s.length.round(),
            "grip_on_frame": [s.grip.0.round(), s.grip.1.round()], "grip_by": s.by,
            "tip_on_frame": [s.tip.0, s.tip.1], "covers_body_pixels": covered,
            "changed_outside_item": noise, "body_pixels_lost": lost, "body_pixels": body,
        }));
        println!(
            "f{i}: item {:>3}px len {:>2.0}  grip ({:>2.0},{:>2.0}) by {:<4} angle {:>4.0}  covers {:>3}  changed outside {:>3} / {body}",
            item.len(), s.length, s.grip.0, s.grip.1, s.by, s.angle, covered, noise
        );

        let mut over = RgbaImage::new(w, h);
        for (x, y, c) in e.enumerate_pixels() {
            if c[3] > 0 {
                over.put_pixel(x, y, Rgba([c[0] / 2 + 60, c[1] / 2 + 60, c[2] / 2 + 60, 255]));
            }
        }
        for (j, (c, k)) in changed.iter().zip(&keep).enumerate() {
            let (x, y) = (j as u32 % w, j as u32 / w);
            if *k {
                over.put_pixel(x, y, Rgba([40, 220, 90, 255]));
            } else if *c {
                over.put_pixel(x, y, Rgba([230, 40, 220, 255]));
            }
        }
        let (rx, ry) = (s.grip.0.round() as u32, s.grip.1.round() as u32);
        if rx < w && ry < h {
            over.put_pixel(rx, ry, Rgba([255, 30, 30, 255]));
        }
        let mut row = RgbaImage::from_pixel(w * scale * 4, h * scale, Rgba([28, 30, 38, 255]));
        for (col, im) in [&e, &a, &layer, &over].into_iter().enumerate() {
            let mut bg = RgbaImage::from_pixel(w, h, Rgba([28, 30, 38, 255]));
            image::imageops::overlay(&mut bg, im, 0, 0);
            paste(&mut row, &bg, col as u32, scale);
        }
        rows.push(row);
    }
    let (rw, rh) = (rows[0].width(), rows[0].height());
    let mut sheet = RgbaImage::new(rw, rh * rows.len() as u32);
    for (k, r) in rows.iter().enumerate() {
        image::imageops::replace(&mut sheet, r, 0, (k as u32 * rh) as i64);
    }
    sheet.save(Path::new(out).join("check.png"))?;
    let attach = json!({
        "frame": [source.0, source.1],
        "origin": [o.origin.0, o.origin.1],
        "angle": "deg-clockwise",
        "points": { o.anim.as_str(): {
            o.point.as_str(): points,
            format!("{}_behind", o.point): behind,
        }},
    });
    std::fs::write(Path::new(out).join("attach.json"), serde_json::to_string_pretty(&attach)?)?;
    std::fs::write(
        Path::new(out).join("report.json"),
        serde_json::to_string_pretty(&json!({"tolerance": o.tol, "frames": report}))?,
    )?;
    Ok(())
}

fn main() -> Res<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let usage = "usage: attach.rs crop <dir> <out> <w> <h> | diff <empty> <armed> <out> \
                 [--crop crop.json] [--origin 0.5,1] [--anim name] [--point hand_r] [--tol 48]";
    match args.first().map(String::as_str) {
        Some("crop") if args.len() == 5 => crop(&args[1], &args[2], args[3].parse()?, args[4].parse()?),
        Some("diff") if args.len() >= 4 => {
            let mut o = Opts { crop: None, origin: (0.5, 1.0), anim: "anim".into(), point: "hand_r".into(), tol: 48 };
            let mut rest = args[4..].iter();
            while let Some(flag) = rest.next() {
                let v = rest.next().ok_or(usage)?;
                match flag.as_str() {
                    "--crop" => o.crop = Some(v.clone()),
                    "--origin" => {
                        let (x, y) = v.split_once(',').ok_or("--origin x,y")?;
                        o.origin = (x.parse()?, y.parse()?);
                    }
                    "--anim" => o.anim = v.clone(),
                    "--point" => o.point = v.clone(),
                    "--tol" => o.tol = v.parse()?,
                    _ => return Err(usage.into()),
                }
            }
            diff(&args[1], &args[2], &args[3], &o)
        }
        _ => Err(usage.into()),
    }
}
