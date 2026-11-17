//! The arena's duel for JavaScript. The page loads this beside the pixfx engine's wasm (`pixfx-web`): the engine
//! plays the effects, lights the scene and moves the bodies; this module holds the rules (`pixfx-duel`), the
//! bot modules a corner seats and the fighters and spell effects drawn from the arena's sprite sheets (`arena-fighter`, `arena-vfx`). It
//! translates; it decides nothing.

pub mod bot;
pub mod duel;
pub mod fighters;
mod js;
pub mod vfx;
