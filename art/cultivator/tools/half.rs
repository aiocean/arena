#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
---
//! half: shrinks a pixel sprite to half size without blending colours. Each
//! 2x2 block becomes the colour most of its opaque pixels share (the darker
//! one on a tie, so outlines survive); a block with fewer than two opaque
//! pixels becomes transparent.
//! Usage: cargo -Zscript half.rs in.png out.png

use image::{Rgba, RgbaImage};
use std::collections::HashMap;

fn lum(c: &Rgba<u8>) -> u32 {
    299 * c[0] as u32 + 587 * c[1] as u32 + 114 * c[2] as u32
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let [_, input, output] = args.as_slice() else {
        return Err("usage: half.rs in.png out.png".into());
    };
    let src = image::open(input)?.to_rgba8();
    let (w, h) = (src.width() / 2, src.height() / 2);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut counts: HashMap<Rgba<u8>, u32> = HashMap::new();
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let c = *src.get_pixel(2 * x + dx, 2 * y + dy);
                if c[3] > 0 {
                    *counts.entry(c).or_default() += 1;
                }
            }
            if counts.values().sum::<u32>() < 2 {
                continue;
            }
            let best = counts
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then(lum(b.0).cmp(&lum(a.0))))
                .map(|(c, _)| *c);
            if let Some(c) = best {
                out.put_pixel(x, y, c);
            }
        }
    }
    out.save(output)?;
    Ok(())
}
