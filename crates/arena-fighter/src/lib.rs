//! The arena's fighters, drawn from sprite sheets. The engine (`pixfx-fighter`) says what a fighter is doing
//! (its `Appearance`) and draws the marks that hang on it; how it looks is the arena's: a sheet of frames per
//! animation ([`sheet`]), the frame each pose shows ([`pick`]) and that frame drawn on an engine `Frame`
//! ([`paint`]).
#![forbid(unsafe_code)]

pub mod paint;
pub mod pick;
pub mod sheet;

pub use paint::{Marks, coarse, draw, draw_marks, draw_portrait, stamp};
pub use pick::{Shot, pick};
pub use sheet::{Cell, Layout, LayoutError, Sheet};
