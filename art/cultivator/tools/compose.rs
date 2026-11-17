#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
serde_json = "1"
---
//! compose: draws a weapon into each frame of an empty-handed animation at the
//! frame's socket (socket.rs's sockets.json), so one body animation carries any
//! weapon of its kind. The weapon picture lies with its blade pointing along
//! `rest` degrees from its grip; it is turned nearest-neighbour, pixels whole.
//! Usage: cargo -Zscript compose.rs framesdir sockets.json weapon.png gripX,gripY rest outdir

use image::{Rgba, RgbaImage};
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [frames, sockets, weapon, grip, rest, out] = a.as_slice() else {
        eprintln!("usage: compose.rs framesdir sockets.json weapon.png gripX,gripY rest outdir");
        std::process::exit(2);
    };
    let doc: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(sockets).unwrap()).unwrap();
    let wpn = image::open(weapon).unwrap().to_rgba8();
    let g: Vec<f64> = grip.split(',').map(|s| s.parse().unwrap()).collect();
    let rest: f64 = rest.parse().unwrap();
    std::fs::create_dir_all(out).unwrap();

    for s in doc["frames"].as_array().unwrap() {
        let name = s["frame"].as_str().unwrap();
        let mut img = image::open(Path::new(frames).join(name)).unwrap().to_rgba8();
        if s["found"].as_bool() == Some(true) {
            let (gx, gy) = (s["grip"][0].as_f64().unwrap(), s["grip"][1].as_f64().unwrap());
            let turn = (s["angle"].as_f64().unwrap() - rest).to_radians();
            let (sn, cs) = turn.sin_cos();
            let (w, h) = img.dimensions();
            // Each frame pixel looks back into the weapon picture.
            let layer = RgbaImage::from_fn(w, h, |x, y| {
                let (dx, dy) = (x as f64 + 0.5 - gx, y as f64 + 0.5 - gy);
                let (sx, sy) = (cs * dx + sn * dy + g[0], -sn * dx + cs * dy + g[1]);
                if sx < 0.0 || sy < 0.0 || sx >= wpn.width() as f64 || sy >= wpn.height() as f64 {
                    return Rgba([0, 0, 0, 0]);
                }
                *wpn.get_pixel(sx as u32, sy as u32)
            });
            for (x, y, p) in layer.enumerate_pixels() {
                if p[3] > 0 {
                    img.put_pixel(x, y, *p);
                }
            }
        }
        img.save(Path::new(out).join(name)).unwrap();
    }
    println!("wrote {out}");
}
