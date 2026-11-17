#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
---
//! erase: takes the masked pixels (a drawn-in blade) off each frame without a
//! model. Filled from the edge of the hole inward: a pixel whose untouched
//! neighbours are mostly empty becomes empty (the blade stood against the
//! background), else it takes its neighbours' commonest colour (the blade lay
//! over the robe).
//! Usage: cargo -Zscript erase.rs framesdir maskdir outdir

use image::{Rgba, RgbaImage};
use std::{collections::HashMap, path::Path};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let [frames, masks, out] = a.as_slice() else {
        eprintln!("usage: erase.rs framesdir maskdir outdir");
        std::process::exit(2);
    };
    std::fs::create_dir_all(out).unwrap();
    let mut names: Vec<_> = std::fs::read_dir(masks).unwrap().map(|e| e.unwrap().file_name()).collect();
    names.sort();
    for name in names {
        let mut img: RgbaImage = image::open(Path::new(frames).join(&name)).unwrap().to_rgba8();
        let mask = image::open(Path::new(masks).join(&name)).unwrap().to_luma8();
        let (w, h) = img.dimensions();
        let mut hole: Vec<bool> = mask.pixels().map(|p| p[0] > 127).collect();
        let mut left = hole.iter().filter(|&&b| b).count();
        while left > 0 {
            let mut fills = Vec::new();
            for y in 0..h {
                for x in 0..w {
                    if !hole[(y * w + x) as usize] {
                        continue;
                    }
                    let (mut empty, mut seen) = (0, 0);
                    let mut colours: HashMap<[u8; 4], u32> = HashMap::new();
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                            if (dx, dy) == (0, 0) || nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                                continue;
                            }
                            if hole[(ny as u32 * w + nx as u32) as usize] {
                                continue;
                            }
                            seen += 1;
                            let c = img.get_pixel(nx as u32, ny as u32).0;
                            if c[3] == 0 {
                                empty += 1;
                            } else {
                                *colours.entry(c).or_default() += 1;
                            }
                        }
                    }
                    if seen == 0 {
                        continue;
                    }
                    let c = if empty * 2 >= seen {
                        [0, 0, 0, 0]
                    } else {
                        colours.into_iter().max_by_key(|&(c, n)| (n, c)).unwrap().0
                    };
                    fills.push((x, y, c));
                }
            }
            if fills.is_empty() {
                break;
            }
            for (x, y, c) in fills {
                img.put_pixel(x, y, Rgba(c));
                hole[(y * w + x) as usize] = false;
                left -= 1;
            }
        }
        img.save(Path::new(out).join(&name)).unwrap();
    }
    println!("wrote {out}");
}
