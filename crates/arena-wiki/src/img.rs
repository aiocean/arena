//! The wiki's pictures, drawn offline from the game's own sheets: a clip's frames in a row on their feet with the
//! rules' boxes over them and its phases marked under them, an effect's frames in a row. Each picture is a WebP named
//! by its content (`img/<hash>.webp`), so a page caches it for good.

use image::{Rgba, RgbaImage, imageops};
use std::path::Path;

/// One frame's picture on a page: its box, and where its top-left sits from its origin (a fighter's feet, an
/// effect's anchor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pic {
    pub page: u32,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub dx: i32,
    pub dy: i32,
}

/// What a frame of a strip shows over its picture, in the picture's pixels from its origin, y up (the rules' boxes).
#[derive(Clone, Debug, Default)]
pub struct Marks {
    pub hurt: Vec<[f64; 4]>,
    pub hit: Option<[f64; 4]>,
    pub hand: Option<[f64; 2]>,
    /// The bar under the frame.
    pub phase: Option<Phase>,
}

/// Which part of a clip a frame is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Windup,
    Active,
    Follow,
}

const HURT: Rgba<u8> = Rgba([90, 180, 255, 255]);
const HIT: Rgba<u8> = Rgba([255, 70, 70, 255]);
const HAND: Rgba<u8> = Rgba([255, 220, 60, 255]);
const BAR: u32 = 6;
const GAP: u32 = 6;

fn phase_colour(p: Phase) -> Rgba<u8> {
    match p {
        Phase::Windup => Rgba([150, 150, 165, 255]),
        Phase::Active => Rgba([255, 70, 70, 255]),
        Phase::Follow => Rgba([240, 170, 60, 255]),
    }
}

/// A sheet's pages, read from PNG or WebP.
pub fn page(path: &Path) -> Result<RgbaImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if path.extension().is_some_and(|e| e == "webp") {
        let decoded = webp::Decoder::new(&bytes)
            .decode()
            .ok_or_else(|| format!("{}: not a WebP", path.display()))?;
        let (w, h) = (decoded.width(), decoded.height());
        let raw: &[u8] = &decoded;
        let rgba: Vec<u8> = if decoded.is_alpha() {
            raw.to_vec()
        } else {
            raw.chunks(3)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect()
        };
        return RgbaImage::from_raw(w, h, rgba)
            .ok_or_else(|| format!("{}: bad size", path.display()));
    }
    image::load_from_memory(&bytes)
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// The frames `pics` (from `pages`) in a row, each on its origin, `marks` over them, scaled by `scale` (`smooth`: a
/// linear filter for drawn art; else nearest, for pixel art).
pub fn strip(
    pages: &[RgbaImage],
    pics: &[Pic],
    marks: &[Marks],
    scale: f64,
    smooth: bool,
) -> Option<RgbaImage> {
    if pics.is_empty() {
        return None;
    }
    // the box round every frame and every mark, from the origin, y down
    let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for p in pics {
        x0 = x0.min(p.dx);
        y0 = y0.min(p.dy);
        x1 = x1.max(p.dx + p.w as i32);
        y1 = y1.max(p.dy + p.h as i32);
    }
    for m in marks {
        for b in m.hurt.iter().chain(m.hit.iter()) {
            x0 = x0.min(b[0].floor() as i32);
            x1 = x1.max(b[2].ceil() as i32);
            y0 = y0.min(-(b[3].ceil() as i32));
            y1 = y1.max(-(b[1].floor() as i32));
        }
    }
    let (fw, fh) = ((x1 - x0).max(1) as u32 + 2, (y1 - y0).max(1) as u32 + 2);
    let n = pics.len() as u32;
    let mut out = RgbaImage::new(n * fw + (n - 1) * GAP, fh);
    for (i, p) in pics.iter().enumerate() {
        let left = i as u32 * (fw + GAP);
        // the origin in this frame's tile
        let (ox, oy) = (left as i32 + 1 - x0, 1 - y0);
        if let Some(src) = pages.get(p.page as usize) {
            let cell = imageops::crop_imm(src, p.x, p.y, p.w, p.h).to_image();
            imageops::overlay(&mut out, &cell, i64::from(ox + p.dx), i64::from(oy + p.dy));
        }
        if let Some(m) = marks.get(i) {
            for b in &m.hurt {
                outline(&mut out, ox, oy, b, HURT);
            }
            if let Some(b) = m.hit {
                outline(&mut out, ox, oy, &b, HIT);
            }
            if let Some([hx, hy]) = m.hand {
                let (cx, cy) = (ox + hx.round() as i32, oy - hy.round() as i32);
                for d in -3..=3 {
                    put(&mut out, cx + d, cy, HAND);
                    put(&mut out, cx, cy + d, HAND);
                }
            }
        }
    }
    let mut out = if (scale - 1.0).abs() > 1e-9 {
        let (w, h) = (
            ((f64::from(out.width()) * scale).round() as u32).max(1),
            ((f64::from(out.height()) * scale).round() as u32).max(1),
        );
        let filter = if smooth {
            imageops::FilterType::Triangle
        } else {
            imageops::FilterType::Nearest
        };
        imageops::resize(&out, w, h, filter)
    } else {
        out
    };
    // the phase bars, after the scale so they stay crisp
    if marks.iter().any(|m| m.phase.is_some()) {
        let (w, h) = (out.width(), out.height());
        let mut barred = RgbaImage::new(w, h + BAR + 2);
        imageops::overlay(&mut barred, &out, 0, 0);
        let tile = f64::from(fw + GAP) * scale;
        let inner = f64::from(fw) * scale;
        for (i, m) in marks.iter().enumerate() {
            let Some(p) = m.phase else { continue };
            let from = (i as f64 * tile).round() as u32;
            let to = (i as f64 * tile + inner).round() as u32;
            for x in from..to.min(w) {
                for y in h + 2..h + 2 + BAR {
                    barred.put_pixel(x, y, phase_colour(p));
                }
            }
        }
        out = barred;
    }
    Some(out)
}

fn put(img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, c);
    }
}

/// Box `b` (from the origin, y up) outlined on `img`, its origin at (`ox`, `oy`).
fn outline(img: &mut RgbaImage, ox: i32, oy: i32, b: &[f64; 4], c: Rgba<u8>) {
    let (l, r) = (ox + b[0].round() as i32, ox + b[2].round() as i32);
    let (t, bt) = (oy - b[3].round() as i32, oy - b[1].round() as i32);
    for x in l..=r {
        put(img, x, t, c);
        put(img, x, bt, c);
    }
    for y in t..=bt {
        put(img, l, y, c);
        put(img, r, y, c);
    }
}

/// `img` as WebP: lossless for pixel art, else lossy at a quality that keeps drawn art clean.
pub fn webp(img: &RgbaImage, lossless: bool) -> Vec<u8> {
    let enc = webp::Encoder::from_rgba(img.as_raw(), img.width(), img.height());
    let mem = if lossless {
        enc.encode_lossless()
    } else {
        enc.encode(82.0)
    };
    mem.to_vec()
}

/// A picture's name by its content: 16 hex digits of FNV-1a.
pub fn name_of(bytes: &[u8]) -> String {
    format!("{:016x}", arena_duel::fingerprint::fnv1a(bytes))
}
