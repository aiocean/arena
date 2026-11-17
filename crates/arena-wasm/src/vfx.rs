//! `arena-vfx` for JS: the spell effects drawn from the arena's sprite sheets, finer than the world, handed to the
//! page as a picture (`fighters::Picture`). It translates only: which frame shows is `arena-vfx`'s.

use crate::fighters::Picture;
use arena_fighter::Sheet;
use pixfx_draw::Frame;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

/// The effects' painter: one sheet per effect (`add`), then, per picture, `begin()`, each effect `draw`, `picture()`.
#[wasm_bindgen]
pub struct Vfx {
    sheets: BTreeMap<String, Sheet>,
    unit: u32,
    fine: Frame,
}

#[wasm_bindgen]
impl Vfx {
    /// A painter for a world `w` x `h` game pixels, drawing the effects `unit` times finer.
    #[wasm_bindgen(constructor)]
    pub fn new(w: u32, h: u32, unit: u32) -> Result<Vfx, JsError> {
        if w == 0 || h == 0 || unit == 0 {
            return Err(JsError::new(&format!("Vfx: the world is {w}x{h} at unit {unit}; it needs all three")));
        }
        Ok(Vfx { sheets: BTreeMap::new(), unit, fine: Frame::new(w * unit, h * unit, 0, 0) })
    }

    /// Effect `fx`'s sheet (its id as the rules name it, `"projectiles/laser-bolt"`): its layout's JSON and every
    /// page's RGBA bytes one after another.
    pub fn add(&mut self, fx: &str, layout: &str, rgba: &[u8]) -> Result<(), JsError> {
        let sheet = Sheet::any(layout, rgba).map_err(|e| JsError::new(&format!("Vfx.add({fx:?}): {e}")))?;
        self.sheets.insert(fx.to_owned(), sheet);
        Ok(())
    }

    /// Whether effect `fx` is drawn from a sheet for a fighter of `school` (`""` for the arena's own).
    pub fn has(&self, fx: &str, school: &str) -> bool {
        self.sheets.get(fx).is_some_and(|s| arena_vfx::plays(s, school))
    }

    /// Start a new picture.
    pub fn begin(&mut self) {
        self.fine.clear();
    }

    /// Effect `fx` `t` seconds after it began, for `school`, anchored at (`x`, `y`) world pixels, mirrored when
    /// `flip`. False once a one-shot effect has played out, or for an effect with no sheet.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, fx: &str, school: &str, t: f64, x: f64, y: f64, flip: bool) -> bool {
        let Some(sheet) = self.sheets.get(fx) else { return false };
        arena_vfx::draw(&mut self.fine, sheet, school, t, x, y, flip, self.unit)
    }

    /// What was drawn since `begin()`: its lit part, `unit` times finer than the world, or none.
    pub fn picture(&self) -> Option<Picture> {
        Picture::of(&self.fine, self.unit)
    }
}
