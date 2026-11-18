//! Pictures of drawn effects against the rules' boxes, to judge where each sits: for every effect named, its largest
//! frame where the game draws it from the point the rules play it at (one cast of its skill at a foe standing still,
//! `fx_anchors`), with the fighters' hurtboxes (blue caster, pink foe), the skill's hitboxes (red), the floor and the
//! point (gold); from each `drawn.json` given side by side (before and after a change), four times the size.
//!
//! `--frames` draws every frame of each side instead of its largest, six to a row.
//!
//! `cargo run -p arena-wiki --example fx_shots -- anchors.json <out dir> <drawn dir> [<drawn dir> ...] [--frames] -- <fx> ...`

// nothing here decides the fight (it only draws it): std's float rounding is fine
#![allow(clippy::disallowed_methods)]

use image::{Rgba, RgbaImage};
use serde_json::Value;
use std::path::Path;

const K: i32 = 4;

fn put(img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        let a = u32::from(c[3]);
        let p = img.get_pixel_mut(x as u32, y as u32);
        for i in 0..3 {
            p[i] = ((u32::from(c[i]) * a + u32::from(p[i]) * (255 - a)) / 255) as u8;
        }
    }
}

fn rect(img: &mut RgbaImage, b: [i32; 4], c: Rgba<u8>) {
    for x in b[0]..=b[2] {
        put(img, x, b[1], c);
        put(img, x, b[3], c);
    }
    for y in b[1]..=b[3] {
        put(img, b[0], y, c);
        put(img, b[2], y, c);
    }
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let split = args
        .iter()
        .position(|a| a == "--")
        .ok_or("usage: fx_shots anchors.json out drawn... -- fx...")?;
    let (head, names) = (&args[..split], &args[split + 1..]);
    let every = head.iter().any(|a| a == "--frames");
    let head: Vec<String> = head.iter().filter(|a| *a != "--frames").cloned().collect();
    let [anchors, out, dirs @ ..] = head.as_slice() else {
        return Err("usage: fx_shots anchors.json out drawn... -- fx...".into());
    };
    let read = |p: &str| std::fs::read_to_string(p).map_err(|e| format!("{p}: {e}"));
    let anchors: Value = serde_json::from_str(&read(anchors)?).map_err(|e| e.to_string())?;
    let sides: Vec<(String, Vec<Value>)> = dirs
        .iter()
        .map(|d| {
            let v: Value = serde_json::from_str(&read(&format!("{d}/drawn.json"))?)
                .map_err(|e| e.to_string())?;
            Ok((d.clone(), v.as_array().cloned().unwrap_or_default()))
        })
        .collect::<Result<_, String>>()?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    for fx in names {
        let play = &anchors[fx.as_str()][0];
        let num = |v: &Value| v.as_f64().unwrap_or(0.0);
        let boxes = |k: &str| -> Vec<[f64; 4]> {
            play[k]
                .as_array()
                .into_iter()
                .flatten()
                .map(|b| [num(&b[0]), num(&b[1]), num(&b[2]), num(&b[3])])
                .collect()
        };
        let hits: Vec<[f64; 4]> = play["hits"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|h| {
                [
                    num(&h["box"][0]),
                    num(&h["box"][1]),
                    num(&h["box"][2]),
                    num(&h["box"][3]),
                ]
            })
            .collect();
        let floor = num(&play["ground"]);
        // each side: its largest frame's cell, where it is drawn from the point, its page
        let mut shots = Vec::new();
        for (dir, list) in &sides {
            let e = list
                .iter()
                .find(|e| e["fx"] == fx.as_str())
                .ok_or(format!("{fx}: not in {dir}"))?;
            let l = &e["layout"];
            let clips = l["clips"].as_object().ok_or("no clips")?;
            let frames = clips
                .get("any")
                .or_else(|| clips.values().next())
                .and_then(Value::as_array)
                .ok_or("no frames")?;
            let lift = if l["motion"]["ground"].as_bool() == Some(true) {
                floor
            } else {
                0.0
            };
            // its largest frame, or every frame with `--frames`
            let area = |c: &&Value| c["w"].as_i64().unwrap_or(0) * c["h"].as_i64().unwrap_or(0);
            let picked: Vec<&Value> = if every {
                frames.iter().collect()
            } else {
                frames.iter().max_by_key(area).into_iter().collect()
            };
            let file = e["file"].as_str().unwrap_or_default();
            for c in picked {
                let page = c["page"].as_u64().unwrap_or(0);
                let name = if page == 0 {
                    format!("{file}.webp")
                } else {
                    format!("{file}-{page}.webp")
                };
                let img = arena_wiki::img::page(&Path::new(dir).join(name))?;
                shots.push((c.clone(), lift, img));
            }
        }
        // one frame of reference for every side
        let (mut x0, mut y0, mut x1, mut y1) = (-1.0f64, -1.0f64, 1.0f64, floor + 2.0);
        let mut grow = |b: [f64; 4]| {
            x0 = x0.min(b[0]);
            y0 = y0.min(b[1]);
            x1 = x1.max(b[2]);
            y1 = y1.max(b[3]);
        };
        for b in boxes("caster")
            .iter()
            .chain(boxes("foe").iter())
            .chain(hits.iter())
        {
            grow(*b);
        }
        for (c, lift, _) in &shots {
            let (dx, dy) = (num(&c["dx"]), num(&c["dy"]) + lift);
            grow([dx, dy, dx + num(&c["w"]), dy + num(&c["h"])]);
        }
        let (x0, y0) = (x0.floor() as i32 - 6, y0.floor() as i32 - 6);
        let (w, h) = (x1.ceil() as i32 + 6 - x0, y1.ceil() as i32 + 6 - y0);
        // panels in rows of six
        let cols = shots.len().clamp(1, 6) as i32;
        let rows = (shots.len() as i32 + cols - 1) / cols;
        let mut img = RgbaImage::from_pixel(
            ((w + 2) * K * cols) as u32,
            ((h + 2) * K * rows) as u32,
            Rgba([18, 16, 26, 255]),
        );
        for (i, (c, lift, page)) in shots.iter().enumerate() {
            let (ox, oy) = (
                (i as i32 % cols) * (w + 2) * K,
                (i as i32 / cols) * (h + 2) * K,
            );
            let at = |x: f64, y: f64| {
                (
                    ox + ((x - f64::from(x0)) * f64::from(K)).round() as i32,
                    oy + ((y - f64::from(y0)) * f64::from(K)).round() as i32,
                )
            };
            let px = |b: &[f64; 4]| {
                let (l, t) = at(b[0], b[1]);
                let (r, bt) = at(b[2], b[3]);
                [l, t, r, bt]
            };
            let (_, gy) = at(0.0, floor);
            for x in ox..ox + w * K {
                put(&mut img, x, gy, Rgba([140, 120, 170, 255]));
            }
            // the frame, nearest neighbour
            let (cx, cy, cw, ch) = (
                num(&c["x"]) as u32,
                num(&c["y"]) as u32,
                num(&c["w"]) as i32,
                num(&c["h"]) as i32,
            );
            let (fx0, fy0) = at(num(&c["dx"]), num(&c["dy"]) + lift);
            for v in 0..ch * K {
                for u in 0..cw * K {
                    let p = *page.get_pixel(cx + (u / K) as u32, cy + (v / K) as u32);
                    if p[3] > 0 {
                        put(&mut img, fx0 + u, fy0 + v, p);
                    }
                }
            }
            for b in boxes("caster") {
                rect(&mut img, px(&b), Rgba([90, 180, 255, 255]));
            }
            for b in boxes("foe") {
                rect(&mut img, px(&b), Rgba([255, 140, 200, 255]));
            }
            for b in &hits {
                rect(&mut img, px(b), Rgba([255, 70, 70, 255]));
            }
            let (gx, gy) = at(0.0, 0.0);
            for d in -4..=4 {
                put(&mut img, gx + d, gy, Rgba([243, 201, 105, 255]));
                put(&mut img, gx, gy + d, Rgba([243, 201, 105, 255]));
            }
        }
        let file = Path::new(out).join(format!("{}.png", fx.replace('/', "-")));
        img.save(&file)
            .map_err(|e| format!("{}: {e}", file.display()))?;
    }
    eprintln!("{} pictures in {out}", names.len());
    Ok(())
}
