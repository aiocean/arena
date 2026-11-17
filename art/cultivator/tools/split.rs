#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
---
//! split: cuts a sprite into rig parts by polygons in a spec, writes each part
//! cropped to its box, a rig.json (box, pivot, parent, z) and a colour map.
//! Usage: cargo -Zscript split.rs sprite.png spec.json outdir
//!
//! spec.json: {"parts":[{"name","parent","z","pivot":[x,y],"poly":[[x,y],...],"source"}]}
//! Parts are tested in file order and the first polygon holding a pixel takes
//! it; a part with no poly takes whatever is left in its "rows" [y0,y1). A part
//! with a "source" (a path relative to the spec) is cut from that picture
//! instead, competing only with the parts cut from the same picture: that is
//! how a part painted over (an arm) and the part repainted under it (the
//! chest) both come out whole.

use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Deserialize)]
struct Part {
    name: String,
    #[serde(default)]
    parent: String,
    z: i32,
    pivot: [f64; 2],
    #[serde(default)]
    poly: Vec<[f64; 2]>,
    #[serde(default)]
    rows: [u32; 2],
    #[serde(default)]
    source: String,
}

#[derive(Deserialize)]
struct Spec {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Out {
    name: String,
    parent: String,
    z: i32,
    file: String,
    #[serde(rename = "box")]
    bbox: [u32; 4], // x, y, w, h on the source canvas
    pivot: [f64; 2], // on the source canvas
    local_pivot: [f64; 2],
    pixels: u32,
}

fn inside(poly: &[[f64; 2]], x: f64, y: f64) -> bool {
    let mut inn = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let ([xi, yi], [xj, yj]) = (poly[i], poly[j]);
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inn = !inn;
        }
        j = i;
    }
    inn
}

const TINTS: [[u8; 3]; 15] = [
    [0, 220, 255], [255, 150, 0], [150, 90, 40], [255, 40, 60], [255, 110, 170],
    [255, 200, 220], [160, 80, 255], [40, 90, 255], [120, 170, 255], [30, 160, 60],
    [130, 230, 100], [200, 255, 120], [255, 255, 0], [0, 255, 180], [220, 220, 220],
];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [sprite, spec_path, out_dir] = args.as_slice() else {
        eprintln!("usage: split.rs sprite.png spec.json outdir");
        std::process::exit(2);
    };
    let spec: Spec = serde_json::from_str(&std::fs::read_to_string(spec_path).unwrap()).unwrap();
    let out_dir = Path::new(out_dir);
    std::fs::create_dir_all(out_dir).unwrap();

    // One owner map per picture; parts compete only within their picture.
    let spec_dir = Path::new(spec_path).parent().unwrap();
    let mut sources: BTreeMap<String, RgbaImage> = BTreeMap::new();
    sources.insert(String::new(), image::open(sprite).unwrap().to_rgba8());
    for p in &spec.parts {
        if !sources.contains_key(&p.source) {
            sources.insert(p.source.clone(), image::open(spec_dir.join(&p.source)).unwrap().to_rgba8());
        }
    }
    let (w, h) = sources[""].dimensions();
    let mut owner: BTreeMap<String, Vec<Option<usize>>> = BTreeMap::new();
    for (key, src) in &sources {
        assert_eq!(src.dimensions(), (w, h), "{key} is not {w}x{h}");
        let mut own = vec![None; (w * h) as usize];
        let mut orphans = 0;
        for (x, y, px) in src.enumerate_pixels() {
            if px[3] == 0 {
                continue;
            }
            let (fx, fy) = (x as f64 + 0.5, y as f64 + 0.5);
            let k = (y * w + x) as usize;
            own[k] = spec.parts.iter().position(|p| {
                p.source == *key
                    && if p.poly.is_empty() { y >= p.rows[0] && y < p.rows[1] } else { inside(&p.poly, fx, fy) }
            });
            if own[k].is_none() && key.is_empty() {
                orphans += 1;
            }
        }
        if orphans > 0 {
            println!("WARNING: {orphans} painted pixels in no part");
        }
        owner.insert(key.clone(), own);
    }

    let mut outs: Vec<Out> = Vec::new();
    for (i, p) in spec.parts.iter().enumerate() {
        let (src, own) = (&sources[&p.source], &owner[&p.source]);
        let mine = |x: u32, y: u32| own[(y * w + x) as usize] == Some(i);
        let (mut x0, mut y0, mut x1, mut y1, mut n) = (u32::MAX, u32::MAX, 0, 0, 0);
        for y in 0..h {
            for x in 0..w {
                if mine(x, y) {
                    (x0, y0, x1, y1, n) = (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1), n + 1);
                }
            }
        }
        if n == 0 {
            println!("{:<16} empty", p.name);
            continue;
        }
        let img = RgbaImage::from_fn(x1 - x0, y1 - y0, |x, y| {
            if mine(x0 + x, y0 + y) { *src.get_pixel(x0 + x, y0 + y) } else { Rgba([0, 0, 0, 0]) }
        });
        let file = format!("{}.png", p.name);
        img.save(out_dir.join(&file)).unwrap();
        println!(
            "{:<16} {:<16} box {x0:3},{y0:3} {:3}x{:<3} pivot {:5.1},{:5.1}  {n:4} px",
            p.name, p.parent, x1 - x0, y1 - y0, p.pivot[0], p.pivot[1]
        );
        outs.push(Out {
            name: p.name.clone(),
            parent: p.parent.clone(),
            z: p.z,
            file,
            bbox: [x0, y0, x1 - x0, y1 - y0],
            pivot: p.pivot,
            local_pivot: [p.pivot[0] - x0 as f64, p.pivot[1] - y0 as f64],
            pixels: n,
        });
    }

    // The map shows every part where it sits, lowest z first.
    let mut map = RgbaImage::new(w, h);
    let mut order: Vec<(usize, &Out)> = outs.iter().enumerate().collect();
    order.sort_by_key(|(_, o)| o.z);
    for (i, o) in order {
        let part = image::open(out_dir.join(&o.file)).unwrap().to_rgba8();
        let t = TINTS[i % TINTS.len()];
        for (x, y, px) in part.enumerate_pixels() {
            if px[3] > 0 {
                map.put_pixel(o.bbox[0] + x, o.bbox[1] + y, Rgba([t[0], t[1], t[2], 255]));
            }
        }
    }
    for o in &outs {
        map.put_pixel(o.pivot[0] as u32, o.pivot[1] as u32, Rgba([255, 255, 255, 255]));
    }
    map.save(out_dir.join("_map.png")).unwrap();
    let rig = serde_json::json!({ "source": sprite, "canvas": [w, h], "parts": outs });
    std::fs::write(out_dir.join("rig.json"), serde_json::to_string_pretty(&rig).unwrap()).unwrap();
}
