//! `arena-fighter` for JS: the duel's fighters drawn from the arena's sprite sheets, handed to the page as pictures
//! in the shape the engine's `drawPicture` takes (`x`, `y`, `w`, `h`, `color`, `light`), and how many of their
//! pixels make one of the page's (`unit`): the fighters and their faces are drawn finer than the page's world, the
//! marks on them in its own pixels. It translates only: which frame shows and how it is drawn are `arena-fighter`'s.

use crate::js::parse;
use arena_fighter::{Marks, Sheet};
use js_sys::Uint8ClampedArray;
use pixfx_core::{PixelBox, color_rgba_in, light_rgba_in};
use pixfx_draw::Frame;
use pixfx_fighter::look::Look;
use pixfx_fighter::paint::Appearance;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

/// The fighters' painter: one sheet per school (`add`), then, per picture, `begin()`, what to draw, `picture()`.
#[wasm_bindgen]
pub struct Fighters {
    sheets: BTreeMap<String, Sheet>,
    unit: u32,
    /// The fighters and faces, `unit` times finer than the world.
    fine: Frame,
    /// The marks on them, in the world's pixels.
    marks: Frame,
    /// The fighters in the world's pixels, for the effects that wrap them.
    body: Frame,
}

#[wasm_bindgen]
impl Fighters {
    /// A painter for a world `w` x `h` game pixels, drawing the fighters `unit` times finer.
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32, unit: u32) -> Result<Fighters, JsError> {
        if w == 0 || h == 0 {
            return Err(JsError::new(&format!(
                "Fighters: the world is {w}x{h}; it needs both sides"
            )));
        }
        if unit == 0 {
            return Err(JsError::new(
                "Fighters: unit is 0; a fighter's pixels are a whole part of the world's",
            ));
        }
        Ok(Fighters {
            sheets: BTreeMap::new(),
            unit,
            fine: Frame::new(w * unit, h * unit, 0, 0),
            marks: Frame::new(w, h, 0, 0),
            body: Frame::new(w, h, 0, 0),
        })
    }

    /// School `school`'s sheet: its layout's JSON and every page's RGBA bytes one after another, in the layout's
    /// order (each page the size the layout gives it).
    pub fn add(&mut self, school: &str, layout: &str, rgba: &[u8]) -> Result<(), JsError> {
        let sheet = Sheet::paged(layout, rgba)
            .map_err(|e| JsError::new(&format!("Fighters.add({school:?}): {e}")))?;
        self.sheets.insert(school.to_owned(), sheet);
        Ok(())
    }

    /// Start a new picture.
    pub fn begin(&mut self) {
        self.fine.clear();
        self.marks.clear();
        self.body.clear();
    }

    /// A fighter of `school`, as `a` (the engine's appearance: `{x, air, pose, poseT, face, ...}`) says, its feet
    /// on row `ground`, and the marks on it (the engine's marks, or null).
    pub fn fighter(
        &mut self,
        school: &str,
        a: &JsValue,
        ground: f64,
        marks: &JsValue,
    ) -> Result<(), JsError> {
        let a: Appearance = parse("appearance", a)?;
        let marks: Option<Marks> = parse("marks", marks)?;
        let sheet = self.sheets.get(school).ok_or_else(|| unknown(school))?;
        arena_fighter::draw(&mut self.fine, sheet, &a, ground, self.unit);
        if let Some(m) = marks {
            let glow = Look::preset(school)
                .map(|l| l.glow)
                .ok_or_else(|| unknown(school))?;
            arena_fighter::draw_marks(&mut self.marks, &m, &a, ground, glow);
        }
        Ok(())
    }

    /// `school`'s face, scaled by whole pixels to fit a `size` square at (`x`, `y`) (world pixels), at `alpha`,
    /// looking right (`face` 1) or left (-1).
    pub fn portrait(
        &mut self,
        school: &str,
        x: i32,
        y: i32,
        size: u32,
        alpha: f64,
        face: f64,
    ) -> Result<(), JsError> {
        let sheet = self.sheets.get(school).ok_or_else(|| unknown(school))?;
        let u = self.unit as i32;
        arena_fighter::draw_portrait(
            &mut self.fine,
            sheet,
            x * u,
            y * u,
            size * self.unit,
            alpha,
            face,
        );
        Ok(())
    }

    /// The fighters and faces drawn since `begin()`: their lit part, `unit` times finer than the world, or none
    /// when nothing shows.
    pub fn picture(&self) -> Option<Picture> {
        Picture::of(&self.fine, self.unit)
    }

    /// The marks drawn since `begin()`, in the world's pixels.
    pub fn marks(&self) -> Option<Picture> {
        Picture::of(&self.marks, 1)
    }

    /// The fighters drawn since `begin()` in the world's pixels, for an effect that wraps them (`effect.wrap`).
    pub fn body(&mut self) -> Option<Picture> {
        arena_fighter::coarse(&self.fine, self.unit, &mut self.body);
        Picture::of(&self.body, 1)
    }
}

fn unknown(school: &str) -> JsError {
    JsError::new(&format!(
        "Fighters: no sheet for school {school:?}; add it first"
    ))
}

/// A picture of fighters: drawn at (`x`, `y`), `w` x `h`, its colour normally, then its light added, in pixels
/// `unit` of which make one of the world's.
#[wasm_bindgen]
pub struct Picture {
    b: PixelBox,
    unit: u32,
    color: Vec<u8>,
    light: Vec<u8>,
}

impl Picture {
    /// The lit part of `frame`, or none when nothing shows.
    fn of(frame: &Frame, unit: u32) -> Option<Picture> {
        let b = PixelBox::lit(frame)?;
        let len = b.w as usize * b.h as usize * 4;
        let (mut color, mut light) = (vec![0; len], vec![0; len]);
        color_rgba_in(frame, b, &mut color);
        light_rgba_in(frame, b, &mut light);
        Some(Picture {
            b,
            unit,
            color,
            light,
        })
    }
}

#[wasm_bindgen]
impl Picture {
    /// How many of its pixels make one of the world's.
    #[wasm_bindgen(getter)]
    pub fn unit(&self) -> u32 {
        self.unit
    }

    #[wasm_bindgen(getter)]
    pub fn x(&self) -> u32 {
        self.b.x
    }

    #[wasm_bindgen(getter)]
    pub fn y(&self) -> u32 {
        self.b.y
    }

    #[wasm_bindgen(getter)]
    pub fn w(&self) -> u32 {
        self.b.w
    }

    #[wasm_bindgen(getter)]
    pub fn h(&self) -> u32 {
        self.b.h
    }

    /// `w * h * 4` bytes, straight alpha.
    #[wasm_bindgen(getter)]
    pub fn color(&self) -> Uint8ClampedArray {
        Uint8ClampedArray::from(self.color.as_slice())
    }

    /// The light over transparency, to draw added.
    #[wasm_bindgen(getter)]
    pub fn light(&self) -> Uint8ClampedArray {
        Uint8ClampedArray::from(self.light.as_slice())
    }
}
