//! The arena's spell effects drawn from sprite sheets (`site/assets/vfx/`, built by `art/vfx/tools/vfx.rs`): a bolt,
//! a blast, a seal, drawn like the fighters, finer than the world. An effect's sheet holds one animation per school
//! (`thunder`, `frost`) or one for any (`any`), played once or round and round until the rules stop it. The engine's
//! pixfx effects stay for what is not a sprite: trails, sparks, the feel.
#![forbid(unsafe_code)]

use arena_fighter::{Cell, Sheet, stamp};
use pixfx_draw::Frame;
use pixfx_draw::math::js_round;

/// The animation of `sheet` that `school` plays: its own, else the one for any school.
fn clip<'a>(sheet: &'a Sheet, school: &str) -> Option<&'a Vec<Cell>> {
    let clips = &sheet.layout.clips;
    clips.get(school).or_else(|| clips.get("any"))
}

/// The frame an effect from `sheet` shows `t` seconds after it began, for a fighter of `school` (none for the arena's
/// own): none once a one-shot effect has played out.
pub fn shot(sheet: &Sheet, school: &str, t: f64) -> Option<Cell> {
    let cells = clip(sheet, school)?;
    if cells.is_empty() {
        return None;
    }
    let i = libm::floor(t.max(0.0) * sheet.layout.fps) as usize;
    if sheet.layout.looping {
        cells.get(i % cells.len()).copied()
    } else {
        cells.get(i).copied()
    }
}

/// Whether `school` has an effect on `sheet` (its own or one for any school).
pub fn plays(sheet: &Sheet, school: &str) -> bool {
    clip(sheet, school).is_some_and(|c| !c.is_empty())
}

/// The effect from `sheet` at `t` drawn with its anchor at (`x`, `y`) of the world, on a frame `unit` times finer,
/// mirrored when `flip`. Returns false once a one-shot effect has played out (nothing is drawn).
#[allow(clippy::too_many_arguments)]
pub fn draw(
    frame: &mut Frame,
    sheet: &Sheet,
    school: &str,
    t: f64,
    x: f64,
    y: f64,
    flip: bool,
    unit: u32,
) -> bool {
    let Some(cell) = shot(sheet, school, t) else {
        return false;
    };
    let k = f64::from(unit);
    let at = (js_round(x * k) as i32, js_round(y * k) as i32);
    stamp(
        frame,
        sheet,
        cell,
        at,
        if flip { -1.0 } else { 1.0 },
        false,
        1.0,
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A sheet of 1x1 frames: thunder's red then green, any school's blue; at 10 fps.
    type Res = Result<(), arena_fighter::LayoutError>;

    fn sheet(looping: bool) -> Result<Sheet, arena_fighter::LayoutError> {
        let cell = |x: u32| format!(r#"{{"x":{x},"y":0,"w":1,"h":1,"dx":0,"dy":-1}}"#);
        let json = format!(
            r#"{{"pages":[[3,1]],"fps":10,"looping":{looping},"clips":{{"thunder":[{},{}],"any":[{}]}}}}"#,
            cell(0),
            cell(1),
            cell(2)
        );
        let rgba = [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255];
        Sheet::any(&json, &rgba)
    }

    #[test]
    fn a_school_plays_its_own_animation_else_the_one_for_any() -> Res {
        let s = sheet(false)?;
        assert_eq!(shot(&s, "thunder", 0.15).map(|c| c.x), Some(1));
        assert_eq!(shot(&s, "frost", 0.0).map(|c| c.x), Some(2));
        assert!(plays(&s, "frost"));
        Ok(())
    }

    #[test]
    fn a_one_shot_ends_and_a_loop_goes_round() -> Res {
        assert_eq!(shot(&sheet(false)?, "thunder", 0.25), None);
        assert_eq!(shot(&sheet(true)?, "thunder", 0.25).map(|c| c.x), Some(0));
        Ok(())
    }

    #[test]
    fn it_draws_at_its_anchor_on_a_finer_frame_and_flips() -> Res {
        let s = sheet(false)?;
        let mut f = Frame::new(20, 20, 0, 0);
        assert!(draw(&mut f, &s, "thunder", 0.0, 5.0, 5.0, false, 2));
        // anchor (10, 10); the 1x1 frame sits one row above it
        assert_eq!(f.color[9 * 20 + 10].to_le_bytes(), [255, 0, 0, 255]);
        let mut g = Frame::new(20, 20, 0, 0);
        assert!(draw(&mut g, &s, "thunder", 0.0, 5.0, 5.0, true, 2));
        assert_eq!(
            g.color[9 * 20 + 9].to_le_bytes(),
            [255, 0, 0, 255],
            "mirrored about the anchor"
        );
        assert!(
            !draw(&mut g, &s, "thunder", 1.0, 5.0, 5.0, false, 2),
            "played out"
        );
        Ok(())
    }
}
