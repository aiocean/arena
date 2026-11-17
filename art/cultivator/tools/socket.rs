#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
serde_json = "1"
---
//! socket: finds the sword drawn into each frame of an animation and turns it
//! into a weapon socket: where the hand grips it and which way the blade
//! points. Also writes, per frame, a mask of the sword (blade, guard and hilt,
//! grown a pixel) for an inpainting model to paint an empty fist over, and an
//! overlay to check the find by eye. `blade/` holds the blade alone (grown two
//! pixels, its outline with it): what erase.rs takes off, since the new weapon covers the hilt.
//!
//! The blade is the longest bright grey run of pixels (steel: low saturation,
//! high luminance); its hilt end is the end nearer the body's centre, and the
//! grip sits `hilt` pixels past it.
//! Usage: cargo -Zscript socket.rs framesdir outdir [hilt=6] [lum=150]

use image::Rgba;
use serde_json::json;
use std::path::Path;

fn lum(c: &Rgba<u8>) -> f64 {
    0.299 * c[0] as f64 + 0.587 * c[1] as f64 + 0.114 * c[2] as f64
}

fn steel(c: &Rgba<u8>, min_lum: f64) -> bool {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    c[3] > 0 && (mx - mn) < 28 && lum(c) > min_lum
}

/// Connected groups (8-way) of the cells `on` marks.
fn components(w: u32, h: u32, on: &[bool]) -> Vec<Vec<(u32, u32)>> {
    let mut seen = vec![false; on.len()];
    let mut out = Vec::new();
    for start in 0..on.len() {
        if !on[start] || seen[start] {
            continue;
        }
        let (mut stack, mut group) = (vec![start], Vec::new());
        seen[start] = true;
        while let Some(i) = stack.pop() {
            let (x, y) = ((i as u32) % w, (i as u32) / w);
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

/// Centre, unit axis and the extent along it (min, max) of a point set.
fn axis(pts: &[(u32, u32)]) -> ([f64; 2], [f64; 2], f64, f64) {
    let n = pts.len() as f64;
    let (cx, cy) = pts.iter().fold((0.0, 0.0), |(a, b), &(x, y)| (a + x as f64, b + y as f64));
    let (cx, cy) = (cx / n, cy / n);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for &(x, y) in pts {
        let (dx, dy) = (x as f64 - cx, y as f64 - cy);
        sxx += dx * dx;
        syy += dy * dy;
        sxy += dx * dy;
    }
    let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let (ux, uy) = (theta.cos(), theta.sin());
    let (mut lo, mut hi) = (f64::MAX, f64::MIN);
    for &(x, y) in pts {
        let t = (x as f64 - cx) * ux + (y as f64 - cy) * uy;
        (lo, hi) = (lo.min(t), hi.max(t));
    }
    ([cx, cy], [ux, uy], lo, hi)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: socket.rs framesdir outdir [hilt=6] [lum=150]");
        std::process::exit(2);
    }
    let (dir, out) = (Path::new(&args[0]), Path::new(&args[1]));
    let hilt: f64 = args.get(2).map_or(6.0, |s| s.parse().unwrap());
    let min_lum: f64 = args.get(3).map_or(150.0, |s| s.parse().unwrap());
    std::fs::create_dir_all(out.join("mask")).unwrap();
    std::fs::create_dir_all(out.join("check")).unwrap();
    std::fs::create_dir_all(out.join("blade")).unwrap();
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "png"))
        .collect();
    files.sort();

    let mut sockets = Vec::new();
    for f in &files {
        let img = image::open(f).unwrap().to_rgba8();
        let (w, h) = img.dimensions();
        let on: Vec<bool> = img.pixels().map(|c| steel(c, min_lum)).collect();
        // The blade: the group that reaches furthest end to end.
        let blade = components(w, h, &on)
            .into_iter()
            .filter(|g| g.len() >= 8)
            .max_by(|a, b| {
                let (_, _, l0, h0) = axis(a);
                let (_, _, l1, h1) = axis(b);
                (h0 - l0).total_cmp(&(h1 - l1))
            });
        let name = f.file_name().unwrap().to_str().unwrap().to_string();
        let Some(blade) = blade else {
            println!("{name}: no sword found");
            sockets.push(json!({ "frame": name, "found": false }));
            continue;
        };
        let (c, u, lo, hi) = axis(&blade);
        let ends = [[c[0] + u[0] * lo, c[1] + u[1] * lo], [c[0] + u[0] * hi, c[1] + u[1] * hi]];
        // The body's centre: every painted pixel that is not the blade.
        let (mut bx, mut by, mut bn) = (0.0, 0.0, 0.0);
        for (x, y, p) in img.enumerate_pixels() {
            if p[3] > 0 && !on[(y * w + x) as usize] {
                (bx, by, bn) = (bx + x as f64, by + y as f64, bn + 1.0);
            }
        }
        let body = [bx / bn, by / bn];
        let d = |p: [f64; 2]| (p[0] - body[0]).hypot(p[1] - body[1]);
        let (base, tip) = if d(ends[0]) < d(ends[1]) { (ends[0], ends[1]) } else { (ends[1], ends[0]) };
        let len = (tip[0] - base[0]).hypot(tip[1] - base[1]);
        let dir = [(tip[0] - base[0]) / len, (tip[1] - base[1]) / len];
        let grip = [base[0] - dir[0] * hilt, base[1] - dir[1] * hilt];
        let angle = dir[1].atan2(dir[0]).to_degrees();

        // Mask: the blade, and a capsule from the blade's base back past the
        // grip (guard, hilt, tassel), grown a pixel.
        let reach = hilt * 2.0;
        let mut mask = image::GrayImage::new(w, h);
        let mut blade_mask = image::GrayImage::new(w, h);
        let mut check = img.clone();
        for y in 0..h {
            for x in 0..w {
                let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                let t = ((px - base[0]) * -dir[0] + (py - base[1]) * -dir[1]).clamp(-1.0, reach);
                let q = [base[0] - dir[0] * t, base[1] - dir[1] * t];
                let near_hilt = (px - q[0]).hypot(py - q[1]) <= 3.0 && img.get_pixel(x, y)[3] > 0;
                let mut near_blade = false;
                for dy in -2i32..=2 {
                    for dx in -2i32..=2 {
                        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                        if nx >= 0 && ny >= 0 && nx < w as i32 && ny < h as i32 && on[(ny as u32 * w + nx as u32) as usize]
                        {
                            near_blade |= blade.contains(&(nx as u32, ny as u32));
                        }
                    }
                }
                if near_blade {
                    blade_mask.put_pixel(x, y, image::Luma([255]));
                }
                if near_blade || near_hilt {
                    mask.put_pixel(x, y, image::Luma([255]));
                    check.put_pixel(x, y, Rgba([255, 0, 200, 255]));
                }
            }
        }
        for (p, col) in [(grip, [0, 255, 0, 255]), (tip, [255, 255, 0, 255])] {
            let (x, y) = (p[0].round() as i32, p[1].round() as i32);
            if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                check.put_pixel(x as u32, y as u32, Rgba(col));
            }
        }
        mask.save(out.join("mask").join(&name)).unwrap();
        blade_mask.save(out.join("blade").join(&name)).unwrap();
        check.save(out.join("check").join(&name)).unwrap();
        println!("{name}: grip {:.1},{:.1} angle {angle:6.1} blade {len:.0}px", grip[0], grip[1]);
        sockets.push(json!({
            "frame": name, "found": true, "grip": [grip[0], grip[1]], "angle": angle,
            "blade_length": len, "z": "front",
        }));
    }
    let doc = json!({ "angle": "degrees, clockwise on screen, 0 = blade pointing +x", "frames": sockets });
    std::fs::write(out.join("sockets.json"), serde_json::to_string_pretty(&doc).unwrap()).unwrap();
}
