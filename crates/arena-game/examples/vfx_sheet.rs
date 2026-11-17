//! A contact sheet of the spell effects: each effect a row, six moments of it across, painted as the game paints them
//! (behind and over the fighters, over a dark stage with a floor), scaled up. To look at them by eye.
//!
//! `cargo run -p arena-game --example vfx_sheet -- /tmp/vfx.png [id-filter ...] [--seed N] [--scale N] [--palette #hex,#hex,...]`

#![allow(clippy::disallowed_methods)]

use arena_game::raster::{Canvas, hex};
use arena_game::vfx::{Paint, Spawn, recipes};

const CW: i32 = 200;
const CH: i32 = 160;
const OX: f32 = 100.0;
const OY: f32 = 110.0;
const GROUND: f32 = 140.0;
const SHOTS: usize = 6;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut out = String::from("/tmp/vfx-sheet.png");
    let (mut filters, mut seed, mut scale, mut palette) = (Vec::new(), 7u32, 2u32, None);
    let mut it = args.into_iter();
    let mut first = true;
    while let Some(a) = it.next() {
        match a.as_str() {
            "--seed" => seed = it.next().and_then(|s| s.parse().ok()).unwrap_or(7),
            "--scale" => scale = it.next().and_then(|s| s.parse().ok()).unwrap_or(2),
            "--palette" => {
                palette = it
                    .next()
                    .map(|s| s.split(',').map(hex).collect::<Vec<_>>())
            }
            _ if first && a.ends_with(".png") => out = a,
            _ => filters.push(a),
        }
        first = false;
    }
    let list: Vec<_> = recipes::all()
        .filter(|r| filters.is_empty() || filters.iter().any(|f| r.id.contains(f.as_str())))
        .collect();
    let (w, h) = (CW * SHOTS as i32, CH * list.len().max(1) as i32);
    let mut sheet = Canvas::new(w, h);
    for (row, r) in list.iter().enumerate() {
        println!("row {row}: {} ({}, {:.2} s)", r.id, if r.looping { "loop" } else { "once" }, r.duration);
        let span = if r.looping { r.duration.clamp(1.0, 3.0) } else { r.duration };
        for shot in 0..SHOTS {
            let t = span * (0.04 + 0.92 * shot as f64 / (SHOTS - 1) as f64);
            let (mut back, mut front) = (stage(), Canvas::new(CW, CH));
            let weather = r.id.starts_with("weather/");
            let at = Spawn {
                x: OX,
                y: if weather || r.id == "fire/brazier" { GROUND } else { OY },
                ground: GROUND,
                palette: palette.clone(),
                width: weather.then_some(CW as f32),
                height: weather.then_some(GROUND + 10.0),
                seed,
                ..Spawn::default()
            };
            let mut i = r.spawn_instance(&at);
            let steps = (t * 60.0).round() as usize;
            for _ in 0..steps {
                i.update(1.0 / 60.0);
            }
            i.paint(&mut Paint {
                back: &mut back,
                front: &mut front,
            });
            let (x0, y0) = (shot as i32 * CW, row as i32 * CH);
            for y in 0..CH {
                for x in 0..CW {
                    let mut p = back.get(x, y);
                    let f = front.get(x, y);
                    let a = f[3] as u32;
                    for c in 0..3 {
                        p[c] = ((f[c] as u32 * a + p[c] as u32 * (255 - a)) / 255) as u8;
                    }
                    p[3] = 255;
                    // a frame line between cells
                    if x == 0 || y == 0 {
                        p = [60, 60, 80, 255];
                    }
                    sheet.put(x0 + x, y0 + y, p, arena_game::raster::Blend::Set);
                }
            }
        }
    }
    let s = scale.max(1);
    let mut img = image::RgbaImage::new(w as u32 * s, h as u32 * s);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = image::Rgba(sheet.get((x / s) as i32, (y / s) as i32));
    }
    match img.save(&out) {
        Ok(()) => println!("wrote {out}"),
        Err(e) => eprintln!("could not write {out}: {e}"),
    }
}

/// A dark stage with a floor, as the effects are seen over.
fn stage() -> Canvas {
    let mut c = Canvas::new(CW, CH);
    for y in 0..CH {
        for x in 0..CW {
            let v = 18 + (y * 14 / CH) as u8;
            let p = if (y as f32) >= GROUND {
                [34, 30, 40, 255]
            } else {
                [v, v + 2, v + 12, 255]
            };
            c.put(x, y, p, arena_game::raster::Blend::Set);
        }
    }
    // a figure's height for scale: a dim post as tall as a fighter (61 px)
    for y in (GROUND as i32 - 61)..GROUND as i32 {
        c.put(CW - 12, y, [50, 50, 64, 255], arena_game::raster::Blend::Set);
    }
    c
}
