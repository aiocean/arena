#!/usr/bin/env -S cargo -Zscript
---
[package]
edition = "2024"

[dependencies]
image = { version = "0.24", default-features = false, features = ["png"] }
gif = "0.14"
---
//! gif: each animation folder (frame_000.png, frame_001.png, ...) as a looping
//! GIF on a grey ground, scaled up by whole pixels so the art stays sharp.
//! Usage: cargo -Zscript gif.rs outdir scale ms dir...

use image::RgbaImage;
use std::{fs::File, path::Path};

const GROUND: [u8; 3] = [96, 96, 108];

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, scale, ms, dirs @ ..] = args.as_slice() else {
        eprintln!("usage: gif.rs outdir scale ms dir...");
        std::process::exit(2);
    };
    let (scale, delay): (u32, u16) = (scale.parse().unwrap(), ms.parse::<u16>().unwrap() / 10);
    std::fs::create_dir_all(out).unwrap();
    for dir in dirs {
        let mut files: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "png"))
            .collect();
        files.sort();
        let frames: Vec<RgbaImage> = files.iter().map(|f| image::open(f).unwrap().to_rgba8()).collect();
        let (w, h) = frames[0].dimensions();
        let (gw, gh) = (w * scale, h * scale);
        // The animation's name is the folder above the direction (…/slash/east).
        let path = Path::new(dir);
        let name = match path.file_name().unwrap().to_str().unwrap() {
            "east" | "west" | "north" | "south" => path.parent().unwrap().file_name().unwrap(),
            _ => path.file_name().unwrap(),
        };
        let file = Path::new(out).join(format!("{}.gif", name.to_str().unwrap()));
        let mut enc = gif::Encoder::new(File::create(&file).unwrap(), gw as u16, gh as u16, &[]).unwrap();
        enc.set_repeat(gif::Repeat::Infinite).unwrap();
        for img in &frames {
            let mut px = Vec::with_capacity((gw * gh * 4) as usize);
            for y in 0..gh {
                for x in 0..gw {
                    let c = img.get_pixel(x / scale, y / scale);
                    let a = c[3] as u32;
                    for k in 0..3 {
                        px.push(((c[k] as u32 * a + GROUND[k] as u32 * (255 - a)) / 255) as u8);
                    }
                    px.push(255);
                }
            }
            let mut frame = gif::Frame::from_rgba_speed(gw as u16, gh as u16, &mut px, 10);
            frame.delay = delay;
            enc.write_frame(&frame).unwrap();
        }
        println!("{} ({} frames)", file.display(), frames.len());
    }
}
