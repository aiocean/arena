//! The hitbox overlay, off unless asked for (the page's "Hitbox" toggle or H, `?boxes=1`; natively H or
//! `ARENA_BOXES=1`): what the rules test hits against on the frame shown, from `arena_duel::debug::shapes` (the same
//! `frames` and `frames_data` the rules read, not a guess of its own), painted over the fighters on the `front`
//! picture. Each fighter's hurtboxes filled translucent in its look's colour with an outline, a blade's or a blast's
//! hitbox in red while the rules test it, shots' and fields' and units' shapes, and the hand's wrist, hilt and a
//! bolt's launch as dots. Off, it paints nothing and reads nothing.

use crate::layers::{Canvases, Uploaded};
use crate::layout::Layout;
use crate::raster::{Blend, Canvas, Rgba};
use crate::rules::Fight;
use arena_duel::data::{Look, UNIT};
use arena_duel::debug::{Drawn, Mark, Shape, shapes};
use arena_duel::fighter::State;
use bevy::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the overlay shows: set by the page (`web::boxes`) or natively by H.
static SHOWN: AtomicBool = AtomicBool::new(false);

/// Show the overlay or not.
pub fn set(on: bool) {
    SHOWN.store(on, Ordering::Relaxed);
}

/// Whether the overlay shows.
pub fn shown() -> bool {
    SHOWN.load(Ordering::Relaxed)
}

/// A shape in the world's pixels (y down from the top), as the overlay paints it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placed {
    Rect { x0: f32, y0: f32, x1: f32, y1: f32 },
    Circle { x: f32, y: f32, r: f32 },
}

/// One thing to paint: where, and in what colour.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub mark: Mark,
    pub placed: Placed,
    pub colour: Rgba,
}

/// The two looks' colours, gold and cyan (the review pages', the HUD's).
const GOLD: Rgba = [255, 200, 40, 255];
const CYAN: Rgba = [60, 220, 255, 255];
const STRIKE: Rgba = [255, 40, 40, 255];
const UNTOUCHABLE: Rgba = [170, 170, 190, 255];
const SHOT: Rgba = [255, 80, 220, 255];
const FIELD: Rgba = [255, 150, 40, 255];
const UNIT_COLOUR: Rgba = [120, 255, 120, 255];
const WRIST: Rgba = [80, 255, 120, 255];
const HILT: Rgba = [80, 200, 255, 255];
const LAUNCH: Rgba = [255, 255, 80, 255];

/// A shape of the rules (`debug::shapes`: x from the arena's left, y up from the ground, the rules' pixels) in the
/// world's pixels over a ground at row `ground`, as the fighters are drawn (`fighters::show`).
pub fn place(shape: Shape, ground: f32) -> Placed {
    let w = |v: f64| (v / UNIT) as f32;
    match shape {
        Shape::Rect { x0, y0, x1, y1 } => Placed::Rect {
            x0: w(x0),
            y0: ground - w(y1),
            x1: w(x1),
            y1: ground - w(y0),
        },
        Shape::Circle { x, y, r } => Placed::Circle {
            x: w(x),
            y: ground - w(y),
            r: w(r),
        },
    }
}

/// The colour of a shape: a body in its fighter's look's colour, the rest by what it is.
fn colour(d: &Drawn, state: &State) -> Rgba {
    match d.mark {
        Mark::Body => match d.owner.and_then(|o| state.fighters.get(o)).map(|f| f.look) {
            Some(Look::ThatThai) => CYAN,
            _ => GOLD,
        },
        Mark::Untouchable => UNTOUCHABLE,
        Mark::Strike => STRIKE,
        Mark::Shot => SHOT,
        Mark::Field => FIELD,
        Mark::Unit => UNIT_COLOUR,
        Mark::Wrist => WRIST,
        Mark::Hilt => HILT,
        Mark::Launch => LAUNCH,
    }
}

/// Everything the overlay paints for `state` over a ground at row `ground`: one mark for each shape the rules report,
/// in their order.
pub fn marks(state: &State, ground: f32) -> Vec<Stroke> {
    shapes(state)
        .iter()
        .map(|d| Stroke {
            mark: d.mark,
            placed: place(d.shape, ground),
            colour: colour(d, state),
        })
        .collect()
}

/// Paint `marks` on `canvas`: an area filled translucent with a solid outline, a point a small dot ringed in black.
pub fn paint(canvas: &mut Canvas, marks: &[Stroke]) {
    let fill = |c: Rgba| [c[0], c[1], c[2], 56];
    for m in marks {
        match m.placed {
            Placed::Rect { x0, y0, x1, y1 } => {
                let (x0, y0) = (x0.floor() as i32, y0.floor() as i32);
                let (x1, y1) = (x1.ceil() as i32, y1.ceil() as i32);
                let (w, h) = ((x1 - x0).max(1), (y1 - y0).max(1));
                canvas.rect(x0, y0, w, h, fill(m.colour), Blend::Over);
                canvas.rect(x0, y0, w, 1, m.colour, Blend::Set);
                canvas.rect(x0, y1 - 1, w, 1, m.colour, Blend::Set);
                canvas.rect(x0, y0, 1, h, m.colour, Blend::Set);
                canvas.rect(x1 - 1, y0, 1, h, m.colour, Blend::Set);
            }
            Placed::Circle { x, y, r } if r > 0.0 => {
                canvas.disc(x, y, r, fill(m.colour), Blend::Over);
                canvas.ring(x, y, r, 1.0, m.colour, Blend::Set);
            }
            Placed::Circle { x, y, .. } => {
                canvas.disc(x, y, 2.0, [10, 10, 10, 255], Blend::Set);
                canvas.disc(x, y, 1.2, m.colour, Blend::Set);
            }
        }
    }
}

pub struct BoxesPlugin;

impl Plugin for BoxesPlugin {
    fn build(&self, app: &mut App) {
        if std::env::var("ARENA_BOXES").is_ok_and(|v| v == "1") {
            set(true);
        }
        app.add_systems(
            Update,
            overlay.in_set(crate::Frame::Upload).before(Uploaded),
        );
        #[cfg(not(target_arch = "wasm32"))]
        app.add_systems(Update, key);
    }
}

/// Natively, H shows or hides the overlay (the page has its own toggle and key, `arena.js`).
#[cfg(not(target_arch = "wasm32"))]
fn key(keys: Res<ButtonInput<KeyCode>>) {
    if keys.just_pressed(KeyCode::KeyH) {
        set(!shown());
    }
}

fn overlay(layout: Res<Layout>, fight: NonSend<Fight>, mut canvases: ResMut<Canvases>) {
    if !shown() {
        return;
    }
    let Some(duel) = &fight.duel else {
        return;
    };
    let marks = marks(duel.state(), layout.ground);
    paint(&mut canvases.front, &marks);
}

#[cfg(test)]
mod tests {
    use super::*;
    use arena_duel::duel::{Duel, DuelConfig};
    use arena_duel::seat::Brain;

    fn duel(seed: u32) -> Duel {
        let layout = Layout::default();
        let config = DuelConfig {
            seed,
            ..crate::rules::config(&layout, seed)
        };
        Duel::new(config, [Brain::make(), Brain::make()])
    }

    /// For every tick of a fight, the overlay paints one mark for each shape the rules report, at that shape's place
    /// in the world (`place`), and a strike shows in red only while the rules report one.
    #[test]
    fn the_overlay_is_the_rules_shapes_tick_by_tick() -> Result<(), String> {
        let ground = Layout::default().ground;
        let (mut strikes, mut points, mut shots) = (0, 0, 0);
        for seed in 1..=4 {
            let mut d = duel(seed);
            for _ in 0..2400 {
                d.step(arena_duel::duel::TICK);
                let reported = shapes(d.state());
                let drawn = marks(d.state(), ground);
                assert_eq!(drawn.len(), reported.len());
                for (m, r) in drawn.iter().zip(&reported) {
                    assert_eq!(m.mark, r.mark);
                    assert_eq!(m.placed, place(r.shape, ground));
                    assert_eq!(m.colour == STRIKE, r.mark == Mark::Strike);
                }
                let count =
                    |f: &dyn Fn(Mark) -> bool| reported.iter().filter(|r| f(r.mark)).count();
                strikes += count(&|m| m == Mark::Strike);
                shots += count(&|m| matches!(m, Mark::Shot | Mark::Field | Mark::Unit));
                points += count(&|m| matches!(m, Mark::Wrist | Mark::Hilt | Mark::Launch));
            }
        }
        // four fights of 40 seconds each show every kind the overlay draws
        assert!(
            strikes > 0 && shots > 0 && points > 0,
            "strikes {strikes}, shots {shots}, points {points}"
        );
        Ok(())
    }

    /// A box lies where its fighter stands: inside the world, its bottom on the ground for a fighter on its feet.
    #[test]
    fn a_standing_fighters_boxes_sit_on_the_ground() {
        let layout = Layout::default();
        let d = duel(7);
        let bodies: Vec<_> = marks(d.state(), layout.ground)
            .into_iter()
            .filter(|m| m.mark == Mark::Body)
            .collect();
        assert!(!bodies.is_empty());
        let lowest = bodies
            .iter()
            .filter_map(|m| match m.placed {
                Placed::Rect { y1, .. } => Some(y1),
                Placed::Circle { .. } => None,
            })
            .fold(f32::MIN, f32::max);
        assert!(
            (lowest - layout.ground).abs() < 1.0,
            "{lowest} vs {}",
            layout.ground
        );
    }

    /// Painted, a box's outline is its colour and its inside a translucent wash of it.
    #[test]
    fn a_box_is_an_outline_over_a_wash() {
        let mut c = Canvas::new(40, 40);
        let m = Stroke {
            mark: Mark::Strike,
            placed: Placed::Rect {
                x0: 10.0,
                y0: 10.0,
                x1: 30.0,
                y1: 20.0,
            },
            colour: STRIKE,
        };
        paint(&mut c, &[m]);
        assert_eq!(c.get(10, 10), STRIKE);
        assert_eq!(c.get(29, 19), STRIKE);
        let inside = c.get(20, 15);
        assert!(inside[3] > 0 && inside[3] < 255, "{inside:?}");
        assert_eq!(c.get(5, 5)[3], 0, "outside is untouched");
    }
}
