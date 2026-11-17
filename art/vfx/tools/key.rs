#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
---
//! key: turns a generated effect picture (`art/README.md`, "Effects": light on black, a grid of frames read left to
//! right then top to bottom) into the frames `vfx.rs` packs, one folder per school, in that school's colours. Each
//! cell is taken `INSET` pixels inside its edges (past the lines a model draws between cells) onto one canvas of the
//! cell's size; the black is keyed out by brightness (the brightest channel); the picture is drawn in greys, and its
//! grey picks the colour along the school's ramp (dark grey its deep edge, white its white), so one picture serves
//! every school. `--scale` shrinks the frames to the size the duel plays them at (world pixels; smooth, the pixel
//! layer makes the pixels). Writes <outdir>/<school>/NN.png.
//!
//! `--keep <school>` keeps the picture's own colours instead (one drawn in that school's colours): each pixel's
//! colour is divided back out of its cover, into <outdir>/<school>/ alone.
//!
//! `--frames` keeps only the first cells (the rest of the picture drawn for the model's sake: what comes after is
//! pixfx's, the breaking up).
//!
//! Usage: cargo -Zscript key.rs <picture.png> <outdir> [--grid 4x2] [--scale 0.3] [--frames 8] [--keep thunder]
//! e.g. `cargo -Zscript art/vfx/tools/key.rs art/vfx/swirl-blast/gen-1/image-01.png art/vfx/swirl-blast/frames
//! --scale 0.32`, then `cargo -Zscript art/vfx/tools/vfx.rs art/vfx/swirl-blast/frames site/assets/vfx
//! explosions/swirl-blast --fps 10`

use image::{Rgba, imageops};
use std::path::Path;

type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// Pixels taken off each side of a cell: the grid lines a model draws and their blur.
const INSET: u32 = 6;
/// Brightness under which a pixel is the background, and over which it is solid (0 to 1).
const KEY: (f32, f32) = (0.07, 0.25);
/// Each school's ramp, dark to light: the duel's circle palettes (`crates/pixfx-duel/src/data.rs`, `THUNDER_CIRCLE`,
/// `FROST_CIRCLE`) from the third colour on, the deep ones being the engine's outline and shadow.
const SCHOOLS: [(&str, [&str; 5]); 2] = [
    ("thunder", ["#b07a1a", "#e8b030", "#ffd84a", "#fff6c4", "#ffffff"]),
    ("frost", ["#2c58aa", "#3a8bd8", "#62c8f4", "#dcfbff", "#ffffff"]),
];
/// The grey the picture's darkest edge tone sits at, and its white: the ramp is spread between them.
const GREYS: (f32, f32) = (0.35, 1.0);

fn hex(s: &str) -> [f32; 3] {
    let v = u32::from_str_radix(s.trim_start_matches('#'), 16).unwrap_or(0);
    [(v >> 16) as f32, ((v >> 8) & 255) as f32, (v & 255) as f32]
}

/// The colour `t` (0 to 1) of the way along `ramp`.
fn along(ramp: &[[f32; 3]], t: f32) -> [f32; 3] {
    let f = t.clamp(0.0, 1.0) * (ramp.len() - 1) as f32;
    let i = (f as usize).min(ramp.len() - 2);
    let k = f - i as f32;
    std::array::from_fn(|c| ramp[i][c] + (ramp[i + 1][c] - ramp[i][c]) * k)
}

fn main() -> Res<()> {
    let usage = "usage: key.rs <picture.png> <outdir> [--grid 4x2] [--scale 0.3] [--frames 8] [--keep thunder]";
    let args: Vec<String> = std::env::args().collect();
    let (Some(input), Some(out)) = (args.get(1), args.get(2)) else {
        return Err(usage.into());
    };
    let (mut cols, mut rows, mut scale, mut frames) = (4u32, 2u32, 1.0f32, u32::MAX);
    let mut keep: Option<String> = None;
    let mut rest = args.iter().skip(3);
    while let Some(flag) = rest.next() {
        match flag.as_str() {
            "--grid" => {
                let g = rest.next().ok_or(usage)?;
                let (c, r) = g.split_once('x').ok_or(usage)?;
                (cols, rows) = (c.parse()?, r.parse()?);
            }
            "--scale" => scale = rest.next().ok_or(usage)?.parse()?,
            "--frames" => frames = rest.next().ok_or(usage)?.parse()?,
            "--keep" => keep = Some(rest.next().ok_or(usage)?.clone()),
            _ => return Err(usage.into()),
        }
    }

    let pic = image::open(input)?.to_rgba8();
    let (cw, ch) = (pic.width() / cols, pic.height() / rows);
    let (w, h) = (((cw - 2 * INSET) as f32 * scale).round() as u32, ((ch - 2 * INSET) as f32 * scale).round() as u32);
    let schools: Vec<(&str, Option<Vec<[f32; 3]>>)> = match &keep {
        Some(k) => vec![(k.as_str(), None)],
        None => SCHOOLS.iter().map(|(s, r)| (*s, Some(r.iter().map(|c| hex(c)).collect()))).collect(),
    };
    for (school, ramp) in &schools {
        let dir = Path::new(out).join(school);
        // a fresh folder: frames left from an earlier, longer run would play on
        if dir.is_dir() {
            std::fs::remove_dir_all(&dir)?;
        }
        std::fs::create_dir_all(&dir)?;
        for i in 0..(cols * rows).min(frames) {
            let (x, y) = ((i % cols) * cw + INSET, (i / cols) * ch + INSET);
            let mut cell = imageops::crop_imm(&pic, x, y, cw - 2 * INSET, ch - 2 * INSET).to_image();
            for p in cell.pixels_mut() {
                let v = p.0[..3].iter().copied().max().unwrap_or(0) as f32 / 255.0;
                let a = ((v - KEY.0) / (KEY.1 - KEY.0)).clamp(0.0, 1.0);
                // an edge pixel blended with the black is its tone darkened by its cover: divide the cover back out
                let c = match ramp {
                    Some(ramp) => {
                        let grey = if a > 0.0 { (v / a.max(v / GREYS.1)).min(1.0) } else { 0.0 };
                        along(ramp, (grey - GREYS.0) / (GREYS.1 - GREYS.0))
                    }
                    // its own colour, the black it was blended with divided back out
                    None => {
                        let k = if a > 0.0 { 1.0 / a.max(v) } else { 0.0 };
                        std::array::from_fn(|i| (p.0[i] as f32 * k).min(255.0))
                    }
                };
                *p = Rgba([c[0] as u8, c[1] as u8, c[2] as u8, (a * 255.0).round() as u8]);
            }
            if scale != 1.0 {
                cell = imageops::resize(&cell, w, h, imageops::FilterType::Lanczos3);
            }
            cell.save(dir.join(format!("{:02}.png", i + 1)))?;
        }
    }
    println!("{input}: {} frames of {w}x{h} for {} school(s) into {out}", (cols * rows).min(frames), schools.len());
    Ok(())
}
