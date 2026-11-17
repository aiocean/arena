//! Bake the spell effects into the sprite sheets the game plays: every way the rules play an effect (its colours and
//! size; the game mirrors a sheet for the facing) is found by fighting brain duels over many seeds, and each is
//! painted frame by frame (`fps_of`), behind and over the fighters, trimmed, the same picture kept once, and every
//! frame packed onto pages tallest first. An effect that reads the fighter (`SPRITE_AWARE`) or
//! the whole scene (`AMBIENT`) is painted live by the game and never baked. Writes `<out>/vfx-<n>.png` and
//! `<out>/index.txt` (`arena_vfx::sheet`).
//!
//! `cargo run --release -p arena-vfx --example bake -- crates/arena-game/assets/vfx [--seeds 60]`

#![allow(clippy::disallowed_methods)]

use arena_vfx::raster::Canvas;
use arena_vfx::sheet::{Cell, Index, Variant};
use arena_vfx::{Paint, Spawn, baked, recipes, variant_key};
use pixfx_duel::duel::{Duel, DuelConfig, TICK};
use pixfx_duel::out::{Out, PlayOpts};
use std::collections::BTreeMap;

/// Frames a second a sheet is baked at: a quick blow at the fighters' 20, a long one at 15, a loop (a slow cycle: a
/// circle turning, a cloud, a flame) at 12, as pixel-art effects are drawn.
fn fps_of(looping: bool, duration: f64) -> f32 {
    if looping {
        12.0
    } else if duration > 1.5 {
        15.0
    } else {
        20.0
    }
}
/// The canvas a frame is painted on, and where the effect's origin is on it: room above for what rises, room below
/// for what falls past where it was played.
const W: i32 = 640;
const H: i32 = 512;
const OX: f32 = 320.0;
const OY: f32 = 384.0;
/// A page's side: what WebGL2 takes everywhere.
const PAGE: u32 = 2048;

/// Every distinct way the rules play each effect, over `seeds` duels of two brains.
fn plays(seeds: u32) -> BTreeMap<(String, String), PlayOpts> {
    let mut found = BTreeMap::new();
    for seed in 1..=seeds {
        let mut duel = Duel::with_brains(DuelConfig {
            seed,
            left: 0.0,
            right: 1680.0,
            ground: 900.0,
            sky: 600.0,
            effects: pixfx_duel::effects::all(),
        });
        for _ in 0..(120.0 / TICK) as u32 {
            for out in duel.step(TICK) {
                if let Out::Play { fx, opts, .. } = out
                    && baked(fx)
                {
                    found
                        .entry((fx.to_string(), variant_key(&opts)))
                        .or_insert(opts);
                }
            }
        }
    }
    // every effect has at least its plain playing, for a variant no duel showed
    for r in recipes::all() {
        if baked(r.id) && !found.keys().any(|(fx, _)| fx == r.id) {
            let opts = PlayOpts::default();
            found.insert((r.id.to_string(), variant_key(&opts)), opts);
        }
    }
    found
}

/// The box of `c`'s opaque pixels: x0, y0, x1, y1 (exclusive); none for an empty picture.
fn opaque(c: &Canvas) -> Option<(i32, i32, i32, i32)> {
    let (mut x0, mut y0, mut x1, mut y1) = (c.w, c.h, 0, 0);
    for y in 0..c.h {
        for x in 0..c.w {
            if c.get(x, y)[3] > 0 {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x + 1);
                y1 = y1.max(y + 1);
            }
        }
    }
    (x1 > x0).then_some((x0, y0, x1, y1))
}

/// Every frame's picture, kept once: its opaque box cut out, and where its top-left sits from the origin.
struct Crops {
    pictures: Vec<(Canvas, i32, i32)>,
    seen: BTreeMap<u64, usize>,
}

impl Crops {
    /// `c`'s opaque part as a picture number (the same picture seen before is that one); none for an empty frame.
    fn put(&mut self, c: &Canvas) -> Option<usize> {
        let (x0, y0, x1, y1) = opaque(c)?;
        let mut cut = Canvas::new(x1 - x0, y1 - y0);
        for y in y0..y1 {
            for x in x0..x1 {
                cut.put(x - x0, y - y0, c.get(x, y), arena_vfx::raster::Blend::Set);
            }
        }
        let (dx, dy) = (x0 - OX as i32, y0 - OY as i32);
        // FNV-1a over the picture and its place
        let mut h: u64 = 0xcbf29ce484222325;
        for b in cut
            .bytes()
            .iter()
            .chain(&dx.to_le_bytes())
            .chain(&dy.to_le_bytes())
            .chain(&cut.w.to_le_bytes())
        {
            h = (h ^ u64::from(*b)).wrapping_mul(0x100000001b3);
        }
        if let Some(&i) = self.seen.get(&h) {
            return Some(i);
        }
        self.pictures.push((cut, dx, dy));
        self.seen.insert(h, self.pictures.len() - 1);
        Some(self.pictures.len() - 1)
    }

    /// Every picture on pages, tallest first, shelf by shelf: each picture's cell.
    fn pack(&self) -> (Vec<Canvas>, Vec<Cell>) {
        let mut order: Vec<usize> = (0..self.pictures.len()).collect();
        order.sort_by_key(|&i| {
            (
                std::cmp::Reverse(self.pictures[i].0.h),
                std::cmp::Reverse(self.pictures[i].0.w),
            )
        });
        let mut pages = vec![Canvas::new(PAGE as i32, PAGE as i32)];
        let mut cells = vec![
            Cell {
                page: 0,
                x: 0,
                y: 0,
                w: 0,
                h: 0,
                dx: 0,
                dy: 0
            };
            self.pictures.len()
        ];
        let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
        for i in order {
            let (pic, dx, dy) = &self.pictures[i];
            let (w, h) = (pic.w as u32, pic.h as u32);
            if x + w > PAGE {
                x = 0;
                y += shelf + 1;
                shelf = 0;
            }
            if y + h > PAGE {
                pages.push(Canvas::new(PAGE as i32, PAGE as i32));
                x = 0;
                y = 0;
                shelf = 0;
            }
            if let Some(p) = pages.last_mut() {
                p.blit(pic, 0, 0, pic.w, pic.h, x as i32, y as i32, [255; 4]);
            }
            cells[i] = Cell {
                page: (pages.len() - 1) as u32,
                x,
                y,
                w,
                h,
                dx: *dx,
                dy: *dy,
            };
            x += w + 1;
            shelf = shelf.max(h);
        }
        (pages, cells)
    }
}

/// One playing of `fx` with `opts`, painted frame by frame, facing right.
/// The frames of one playing, as picture numbers (`Crops`) behind and over the fighters.
type Frames = (Variant, Vec<Option<usize>>, Vec<Option<usize>>);

fn bake(fx: &str, key: &str, opts: &PlayOpts, crops: &mut Crops) -> Option<Frames> {
    let recipe = recipes::recipe(fx)?;
    let effect = recipes::find(fx)?;
    let at = Spawn {
        seed: 7,
        flip: false,
        ..Spawn::of(OX, OY, OY + 120.0, opts)
    };
    let fps = fps_of(recipe.looping, recipe.duration);
    let mut instance = effect.spawn(&at);
    let frames = if recipe.looping {
        (recipe.duration * f64::from(fps)).round().max(1.0) as usize
    } else {
        (recipe.duration * f64::from(fps)).round() as usize + 1
    };
    let restart = recipe.intro.map_or(0, |i| {
        ((i * f64::from(fps)).round() as usize).min(frames.saturating_sub(1))
    });
    let v = Variant {
        fx: fx.to_string(),
        key: key.to_string(),
        looping: recipe.looping,
        fps,
        restart,
        ..Variant::default()
    };
    let (mut backs, mut fronts) = (Vec::new(), Vec::new());
    let (mut back, mut front) = (Canvas::new(W, H), Canvas::new(W, H));
    for k in 0..frames {
        if k > 0 {
            instance.update(1.0 / fps);
        }
        back.clear();
        front.clear();
        instance.paint(&mut Paint {
            back: &mut back,
            front: &mut front,
        });
        backs.push(crops.put(&back));
        fronts.push(crops.put(&front));
    }
    Some((v, backs, fronts))
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let out = args.next().ok_or("usage: bake <outdir> [--seeds N]")?;
    let mut seeds = 60;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--seeds" => {
                seeds = args
                    .next()
                    .and_then(|s| s.parse().ok())
                    .ok_or("--seeds N")?
            }
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    let found = plays(seeds);
    println!("{} playings of {} effects", found.len(), {
        let mut ids: Vec<&String> = found.keys().map(|(fx, _)| fx).collect();
        ids.dedup();
        ids.len()
    });
    let mut crops = Crops {
        pictures: Vec::new(),
        seen: BTreeMap::new(),
    };
    let mut baked = Vec::new();
    for ((fx, key), opts) in &found {
        if let Some(f) = bake(fx, key, opts, &mut crops) {
            println!("  {fx} {key}: {} frames at {} fps", f.1.len(), f.0.fps);
            baked.push(f);
        }
    }
    let (pages, cells) = crops.pack();
    let mut index = Index::default();
    for (mut v, backs, fronts) in baked {
        v.back = backs.iter().map(|b| b.map(|i| cells[i])).collect();
        v.front = fronts.iter().map(|f| f.map(|i| cells[i])).collect();
        index.variants.push(v);
    }
    std::fs::create_dir_all(&out).map_err(|e| format!("{out}: {e}"))?;
    for old in std::fs::read_dir(&out)
        .map_err(|e| format!("{out}: {e}"))?
        .flatten()
    {
        if old.file_name().to_string_lossy().starts_with("vfx-") {
            let _ = std::fs::remove_file(old.path());
        }
    }
    for (i, page) in pages.iter().enumerate() {
        // a page cut to the rows that are used
        let used = cells
            .iter()
            .filter(|c| c.page == i as u32)
            .map(|c| c.y + c.h)
            .max()
            .unwrap_or(1);
        let name = format!("vfx-{i}.png");
        let img = image::RgbaImage::from_raw(
            PAGE,
            used,
            page.bytes()[..(PAGE * used * 4) as usize].to_vec(),
        )
        .ok_or("a page's bytes")?;
        let file =
            std::fs::File::create(format!("{out}/{name}")).map_err(|e| format!("{name}: {e}"))?;
        let png = image::codecs::png::PngEncoder::new_with_quality(
            std::io::BufWriter::new(file),
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        img.write_with_encoder(png)
            .map_err(|e| format!("{name}: {e}"))?;
        index.pages.push((name, [PAGE, used]));
    }
    println!(
        "{} pictures ({:.1} Mpx) on {} pages",
        crops.pictures.len(),
        cells.iter().map(|c| f64::from(c.w * c.h)).sum::<f64>() / 1e6,
        pages.len()
    );
    std::fs::write(format!("{out}/index.txt"), index.write()).map_err(|e| format!("index: {e}"))?;
    Ok(())
}
