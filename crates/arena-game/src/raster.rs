//! A picture in the world's own pixels, drawn on the CPU: the spells' particles, the light, the backdrop and the HUD
//! are painted here pixel by pixel (hard edges, no smoothing, as pixel art is drawn) and shown as a texture scaled up
//! by whole screen pixels.

/// A colour: red, green, blue, alpha, 0 to 255 (straight alpha, sRGB).
pub type Rgba = [u8; 4];

/// `#rrggbb` or `#rrggbbaa` (a missing or malformed colour is magenta, so it shows).
pub fn hex(s: &str) -> Rgba {
    let s = s.trim_start_matches('#');
    let at = |i: usize| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok());
    match (at(0), at(2), at(4)) {
        (Some(r), Some(g), Some(b)) => [
            r,
            g,
            b,
            if s.len() >= 8 {
                at(6).unwrap_or(255)
            } else {
                255
            },
        ],
        _ => [255, 0, 255, 255],
    }
}

/// `c` with its alpha multiplied by `a` (0 to 1).
pub fn fade(c: Rgba, a: f32) -> Rgba {
    [
        c[0],
        c[1],
        c[2],
        (c[3] as f32 * a.clamp(0.0, 1.0)).round() as u8,
    ]
}

/// The colour `t` (0 to 1) of the way from `a` to `b`.
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    [l(a[0], b[0]), l(a[1], b[1]), l(a[2], b[2]), l(a[3], b[3])]
}

/// The colour `t` (0 to 1) along a ramp of colours, evenly spaced.
pub fn ramp(colors: &[Rgba], t: f32) -> Rgba {
    match colors.len() {
        0 => [255, 0, 255, 255],
        1 => colors[0],
        n => {
            let at = t.clamp(0.0, 1.0) * (n - 1) as f32;
            let i = (at.floor() as usize).min(n - 2);
            mix(colors[i], colors[i + 1], at - i as f32)
        }
    }
}

/// How a pixel meets what is under it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    /// Replaces it.
    Set,
    /// Over it, by its alpha.
    Over,
    /// Added to it, by its alpha (light).
    Add,
}

/// A picture `w` by `h` pixels, row by row from the top left.
#[derive(Clone, Debug)]
pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub px: Vec<Rgba>,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Canvas {
        let (w, h) = (w.max(1), h.max(1));
        Canvas {
            w,
            h,
            px: vec![[0; 4]; (w * h) as usize],
        }
    }

    /// Every pixel clear.
    pub fn clear(&mut self) {
        self.px.fill([0; 4]);
    }

    /// The canvas as RGBA bytes.
    pub fn bytes(&self) -> &[u8] {
        self.px.as_flattened()
    }

    pub fn get(&self, x: i32, y: i32) -> Rgba {
        if x < 0 || y < 0 || x >= self.w || y >= self.h {
            return [0; 4];
        }
        self.px[(y * self.w + x) as usize]
    }

    /// Pixel (`x`, `y`) meets colour `c` by `blend`; off the canvas, nothing.
    pub fn put(&mut self, x: i32, y: i32, c: Rgba, blend: Blend) {
        if x < 0 || y < 0 || x >= self.w || y >= self.h || (c[3] == 0 && blend != Blend::Set) {
            return;
        }
        let p = &mut self.px[(y * self.w + x) as usize];
        match blend {
            Blend::Set => *p = c,
            Blend::Over => {
                let a = c[3] as u32;
                let under = p[3] as u32 * (255 - a) / 255;
                let out = a + under;
                if out == 0 {
                    *p = [0; 4];
                    return;
                }
                for i in 0..3 {
                    p[i] = ((c[i] as u32 * a + p[i] as u32 * under) / out) as u8;
                }
                p[3] = out.min(255) as u8;
            }
            Blend::Add => {
                let a = c[3] as u32;
                for i in 0..3 {
                    p[i] = (p[i] as u32 + c[i] as u32 * a / 255).min(255) as u8;
                }
                p[3] = 255;
            }
        }
    }

    /// The rectangle from (`x`, `y`), `w` by `h`.
    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Rgba, blend: Blend) {
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(self.w), (y + h).min(self.h));
        for yy in y0..y1 {
            for xx in x0..x1 {
                self.put(xx, yy, c, blend);
            }
        }
    }

    /// A disc of radius `r` around (`cx`, `cy`).
    pub fn disc(&mut self, cx: f32, cy: f32, r: f32, c: Rgba, blend: Blend) {
        if r <= 0.0 {
            return;
        }
        let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                if dx * dx + dy * dy <= r * r {
                    self.put(x, y, c, blend);
                }
            }
        }
    }

    /// A ring of radius `r` and thickness `t` around (`cx`, `cy`).
    pub fn ring(&mut self, cx: f32, cy: f32, r: f32, t: f32, c: Rgba, blend: Blend) {
        if r <= 0.0 {
            return;
        }
        let outer = r + t / 2.0;
        let inner = (r - t / 2.0).max(0.0);
        let (x0, x1) = ((cx - outer).floor() as i32, (cx + outer).ceil() as i32);
        let (y0, y1) = ((cy - outer).floor() as i32, (cy + outer).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = dx * dx + dy * dy;
                if d <= outer * outer && d >= inner * inner {
                    self.put(x, y, c, blend);
                }
            }
        }
    }

    /// A one-pixel line from (`x0`, `y0`) to (`x1`, `y1`).
    pub fn line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, c: Rgba, blend: Blend) {
        let (mut x, mut y) = (x0.floor() as i32, y0.floor() as i32);
        let (xe, ye) = (x1.floor() as i32, y1.floor() as i32);
        let (dx, dy) = ((xe - x).abs(), -(ye - y).abs());
        let (sx, sy) = (if x < xe { 1 } else { -1 }, if y < ye { 1 } else { -1 });
        let mut err = dx + dy;
        // a line never draws one pixel twice, so a translucent one stays even
        for _ in 0..4096 {
            self.put(x, y, c, blend);
            if x == xe && y == ye {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// A soft glow: a disc of radius `r` whose alpha falls from `c`'s at the middle to none at the edge, in `steps`
    /// hard bands (pixel art has no smooth gradients).
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgba, steps: u32, blend: Blend) {
        if r <= 0.0 {
            return;
        }
        let steps = steps.max(1) as f32;
        let (x0, x1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32);
        let (y0, y1) = ((cy - r).floor() as i32, (cy + r).ceil() as i32);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt() / r;
                if d < 1.0 {
                    let band = ((1.0 - d) * steps).ceil() / steps;
                    self.put(x, y, fade(c, band), blend);
                }
            }
        }
    }

    /// Part of `src` (`sx`, `sy`, `w` by `h`) drawn at (`x`, `y`), over what is here, tinted by `tint` (multiplied).
    #[allow(clippy::too_many_arguments)]
    pub fn blit(
        &mut self,
        src: &Canvas,
        sx: i32,
        sy: i32,
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        tint: Rgba,
    ) {
        for j in 0..h {
            for i in 0..w {
                let p = src.get(sx + i, sy + j);
                if p[3] == 0 {
                    continue;
                }
                let m = |a: u8, b: u8| (a as u32 * b as u32 / 255) as u8;
                let c = [
                    m(p[0], tint[0]),
                    m(p[1], tint[1]),
                    m(p[2], tint[2]),
                    m(p[3], tint[3]),
                ];
                self.put(x + i, y + j, c, Blend::Over);
            }
        }
    }

    /// A PNG's pixels as a canvas.
    pub fn from_png(bytes: &[u8]) -> Option<Canvas> {
        let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
        let rgba = img.to_rgba8();
        let (w, h) = (rgba.width() as i32, rgba.height() as i32);
        let px = rgba
            .into_raw()
            .chunks_exact(4)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();
        Some(Canvas { w, h, px })
    }
}

/// Small deterministic dice for what is only drawn (never the rules'): mulberry32.
#[derive(Clone, Debug)]
pub struct Dice(u32);

impl Dice {
    pub fn new(seed: u32) -> Dice {
        Dice(seed)
    }

    /// A number in [0, 1).
    pub fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x6d2b79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        ((t ^ (t >> 14)) as f64 / 4294967296.0) as f32
    }

    /// A number in [`a`, `b`).
    pub fn range(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.next()
    }

    /// -1 or 1.
    pub fn sign(&mut self) -> f32 {
        if self.next() < 0.5 { -1.0 } else { 1.0 }
    }
}
