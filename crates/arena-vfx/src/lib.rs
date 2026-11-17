//! The arena's spell effects painted on the CPU in world pixels: each effect id the rules play is an `Effect`
//! (`recipes`, on the `kit`) that starts an `Instance`; the rules move it and stop it, and one that does not loop ends
//! on its own. The game plays nearly all of them as sprite sheets baked from these (`examples/bake.rs`); only an
//! effect that needs the fighter's sprite (`SPRITE_AWARE`) is painted live. Rewritten after the pixfx engine's effects.

#![forbid(unsafe_code)]
// nothing here decides the fight (it only draws it): std's float rounding and transcendentals are fine
#![allow(clippy::disallowed_methods)]

pub mod drawn;
pub mod kit;
pub mod raster;
pub mod recipes;

use raster::{Canvas, Rgba, hex};

/// The effects that read the fighter they play on (its body, its outline): painted live by the game, never baked.
pub const SPRITE_AWARE: &[&str] = &["feedback/power-up"];

/// How an effect is played, in world pixels: where, its school's colours (from the rules), its size, which way it
/// faces, its seed.
#[derive(Clone, Debug, Default)]
pub struct Spawn {
    pub x: f32,
    pub y: f32,
    /// The ground's row (an effect that falls or splashes stops there).
    pub ground: f32,
    pub palette: Option<Vec<Rgba>>,
    pub accent: Option<Vec<Rgba>>,
    pub stars: Option<Vec<Rgba>>,
    pub fire: Option<Vec<Rgba>>,
    pub glow: Option<Rgba>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// Faces left.
    pub flip: bool,
    pub seed: u32,
}

impl Spawn {
    /// The effect's palette, or `default` when the rules gave none.
    pub fn palette_or(&self, default: &[&str]) -> Vec<Rgba> {
        self.palette
            .clone()
            .unwrap_or_else(|| default.iter().map(|c| hex(c)).collect())
    }

    /// Which way it faces: 1 right, -1 left.
    pub fn dir(&self) -> f32 {
        if self.flip { -1.0 } else { 1.0 }
    }
}

/// The two pictures an instance paints: behind the fighters (`back`) and over them (`front`). Light is added (`Blend::Add`)
/// on the same pictures.
pub struct Paint<'a> {
    pub back: &'a mut Canvas,
    pub front: &'a mut Canvas,
}

/// An effect: what starts when the rules play its id.
pub trait Effect: Send + Sync {
    /// Its id, as the rules name it (`"fire/brazier"`).
    fn id(&self) -> &'static str;
    /// Whether it plays until stopped (the rules stop it) rather than ending by itself.
    fn looping(&self) -> bool;
    /// One playing of it.
    fn spawn(&self, at: &Spawn) -> Box<dyn Instance>;
}

/// One playing effect.
pub trait Instance: Send + Sync {
    /// Moves on by `dt` seconds of the fight; false once it is over (a stopped effect fades out first).
    fn update(&mut self, dt: f32) -> bool;
    /// Where it is now (the rules move bolts and orbs each tick).
    fn set_position(&mut self, x: f32, y: f32);
    /// Asked to end: a looping effect fades out and then is over.
    fn stop(&mut self);
    /// Paints this frame.
    fn paint(&self, paint: &mut Paint);
}

/// An effect that belongs to the scene rather than to a spell (sized to the whole world): painted live by the game.
pub const AMBIENT: &[&str] = &["weather/fireflies"];

/// Whether effect `fx` is played from a baked sprite sheet.
pub fn baked(fx: &str) -> bool {
    !SPRITE_AWARE.contains(&fx) && !AMBIENT.contains(&fx)
}

fn colors(c: Option<&'static [&'static str]>) -> Option<Vec<Rgba>> {
    c.map(|cs| cs.iter().map(|s| hex(s)).collect())
}

impl Spawn {
    /// How the rules' play options read at world pixel (`x`, `y`) over the ground's row `ground` (lengths in the
    /// options are the rules' pixels, `UNIT` of them a world pixel).
    pub fn of(x: f32, y: f32, ground: f32, opts: &arena_duel::out::PlayOpts) -> Spawn {
        let world = |v: f64| (v / arena_duel::data::UNIT) as f32;
        Spawn {
            x,
            y,
            ground,
            palette: colors(opts.palette),
            accent: colors(opts.accent),
            stars: colors(opts.stars),
            fire: colors(opts.fire),
            glow: opts.glow.map(hex),
            width: opts.width.map(world),
            height: opts.height.map(world),
            flip: opts.flip.unwrap_or(false),
            seed: opts.seed.unwrap_or(1),
        }
    }
}

/// Which baked sheet of an effect a playing of it shows: its colours and size, never its seed (a sheet is one playing)
/// nor its facing (the game mirrors a sheet baked facing right). The bake and the game key a playing the same way.
pub fn variant_key(opts: &arena_duel::out::PlayOpts) -> String {
    let list = |c: Option<&[&str]>| c.map_or(String::new(), |cs| cs.join(","));
    let num = |v: Option<f64>| v.map_or(String::new(), |v| format!("{v:.0}"));
    format!(
        "p{}|a{}|s{}|f{}|g{}|w{}|h{}",
        list(opts.palette),
        list(opts.accent),
        list(opts.stars),
        list(opts.fire),
        opts.glow.unwrap_or(""),
        num(opts.width),
        num(opts.height),
    )
}

/// The sheets' layout as the bake writes it (`vfx/index.txt`): one line a frame cell, so the game reads it without a
/// JSON parser. Lines: `page <file> <w> <h>`, `variant <fx> <key> <loop 0|1> <restart> <fps>` (`restart`: the frame a loop
/// comes back to), then `back <page> <x> <y> <w> <h> <dx> <dy>` / `front ...` cells, one of each a frame (`- ` for an empty
/// one), in frame order.
pub mod sheet {
    /// One frame's picture on a page: its box and where its top-left sits from the effect's origin.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Cell {
        pub page: u32,
        pub x: u32,
        pub y: u32,
        pub w: u32,
        pub h: u32,
        pub dx: i32,
        pub dy: i32,
    }

    /// One baked playing of an effect: its frames behind and over the fighters.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Variant {
        pub fx: String,
        pub key: String,
        pub looping: bool,
        /// Frames a second it was baked at.
        pub fps: f32,
        /// The frame a loop comes back to (after its opening).
        pub restart: usize,
        pub back: Vec<Option<Cell>>,
        pub front: Vec<Option<Cell>>,
    }

    /// Every baked sheet.
    #[derive(Clone, Debug, Default, PartialEq)]
    pub struct Index {
        /// Each page's file and size.
        pub pages: Vec<(String, [u32; 2])>,
        pub variants: Vec<Variant>,
    }

    impl Index {
        /// The variant of `fx` keyed `key`, or the first of `fx` when that one was never baked.
        pub fn find(&self, fx: &str, key: &str) -> Option<&Variant> {
            self.variants
                .iter()
                .find(|v| v.fx == fx && v.key == key)
                .or_else(|| self.variants.iter().find(|v| v.fx == fx))
        }

        pub fn write(&self) -> String {
            let mut s = String::new();
            for (p, [w, h]) in &self.pages {
                s += &format!("page {p} {w} {h}\n");
            }
            let cell = |c: &Option<Cell>| match c {
                Some(c) => format!(
                    "{} {} {} {} {} {} {}",
                    c.page, c.x, c.y, c.w, c.h, c.dx, c.dy
                ),
                None => "-".to_string(),
            };
            for v in &self.variants {
                s += &format!(
                    "variant {} {} {} {} {}\n",
                    v.fx,
                    v.key,
                    u8::from(v.looping),
                    v.restart,
                    v.fps
                );
                for (b, f) in v.back.iter().zip(&v.front) {
                    s += &format!("back {}\nfront {}\n", cell(b), cell(f));
                }
            }
            s
        }

        pub fn read(text: &str) -> Result<Index, String> {
            let mut index = Index::default();
            for (n, line) in text.lines().enumerate() {
                let bad = || format!("vfx index line {}: {line}", n + 1);
                let mut w = line.split(' ');
                let num = |s: Option<&str>| -> Result<i64, String> {
                    s.and_then(|s| s.parse().ok()).ok_or_else(bad)
                };
                match w.next() {
                    Some("page") => {
                        let file = w.next().ok_or_else(bad)?.to_string();
                        let size = [num(w.next())? as u32, num(w.next())? as u32];
                        index.pages.push((file, size));
                    }
                    Some("variant") => index.variants.push(Variant {
                        fx: w.next().ok_or_else(bad)?.to_string(),
                        key: w.next().ok_or_else(bad)?.to_string(),
                        looping: num(w.next())? == 1,
                        restart: num(w.next())? as usize,
                        fps: w.next().and_then(|s| s.parse().ok()).ok_or_else(bad)?,
                        ..Variant::default()
                    }),
                    Some(side @ ("back" | "front")) => {
                        let v = index.variants.last_mut().ok_or_else(bad)?;
                        let first = w.next().ok_or_else(bad)?;
                        let c = if first == "-" {
                            None
                        } else {
                            let mut nums = vec![first.parse::<i64>().map_err(|_| bad())?];
                            for _ in 0..6 {
                                nums.push(num(w.next())?);
                            }
                            Some(Cell {
                                page: nums[0] as u32,
                                x: nums[1] as u32,
                                y: nums[2] as u32,
                                w: nums[3] as u32,
                                h: nums[4] as u32,
                                dx: nums[5] as i32,
                                dy: nums[6] as i32,
                            })
                        };
                        if side == "back" {
                            v.back.push(c)
                        } else {
                            v.front.push(c)
                        }
                    }
                    Some("") | None => {}
                    _ => return Err(bad()),
                }
            }
            Ok(index)
        }
    }
}
